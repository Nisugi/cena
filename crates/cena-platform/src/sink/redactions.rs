//! [`Redactions`]: the closed set of strings a session log must never contain.
//!
//! Split from `writer.rs` under `plan/05` Rule 4.1 -- move code down, do not
//! raise the cap -- when naming the registered kinds in the log header (review
//! finding 9) took that file past 800 lines. The seam was already there:
//! nothing in this file knows a file exists, and the sink only asks it two
//! questions -- what to replace, and what to call what it replaces.

/// The exact strings to remove before anything is written.
///
/// **A closed set, known in advance.** That is what makes this exact rather
/// than a guess: the account is typed at a prompt, and the key
/// arrives in the `L` response. Nothing here has to decide whether a word
/// *looks like* a credential.
///
/// The live run of 2026-09-18 printed
/// `A\t<ACCOUNT>\tKEY\t<KEY-REDACTED>\t<NAME>` -- the key was already
/// redacted for the terminal; the account name and the author's real name were
/// not, and would have gone to disk verbatim.
#[derive(Default, Clone)]
pub struct Redactions {
    secrets: Vec<(String, &'static str)>,
}

/// Hand-written, because the derive **printed every secret**.
///
/// `Redactions` exists to keep launch keys out of files, and deriving `Debug`
/// meant any `{:?}` -- on it, on a `SessionSink`, or on a `SessionEnd` that
/// contains one -- dumped the whole `Vec<(String, _)>` in the clear. Found by
/// review. The same mistake `Credentials` and `LaunchPayload` were already
/// written by hand to avoid.
///
/// The COUNT is shown, which is what a reader legitimately wants ("was anything
/// registered?") and reveals nothing.
impl std::fmt::Debug for Redactions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactions")
            .field(
                "secrets",
                &format_args!("{} registered", self.secrets.len()),
            )
            .finish()
    }
}

impl Redactions {
    /// Nothing redacted yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Redact one exact string wherever it appears.
    ///
    /// Ignores empties and very short strings: a one- or two-character secret
    /// would match everywhere and turn the log into noise, which is a worse
    /// outcome than not redacting a string that short. Three is the floor.
    pub fn add(&mut self, secret: &str, replacement: &'static str) {
        let secret = secret.trim();
        if secret.len() >= 3 {
            self.secrets.push((secret.to_owned(), replacement));
        }
    }

    /// The account name, its `A`-response echo, and the real name beside it.
    pub fn account(&mut self, account: &str) {
        self.add(account, "<ACCOUNT>");
        // The server echoes the account uppercased in `A\t<ACCOUNT>\tKEY...`,
        // which a case-sensitive match would miss.
        self.add(&account.to_uppercase(), "<ACCOUNT>");
    }

    /// The session key from `L`. One-shot and short-lived, but it is a
    /// credential for as long as it is valid.
    pub fn key(&mut self, key: &str) {
        self.add(key, "<KEY>");
    }

    /// The account holder's real name, as the `A` response carries it.
    pub fn real_name(&mut self, name: &str) {
        self.add(name, "<NAME>");
        self.add(&name.to_uppercase(), "<NAME>");
    }

    /// Apply every redaction to a string.
    #[must_use]
    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (secret, replacement) in &self.secrets {
            if out.contains(secret.as_str()) {
                out = out.replace(secret.as_str(), replacement);
            }
        }
        out
    }

    /// Apply every redaction to raw wire bytes, **byte for byte**.
    ///
    /// # Why this does not go through `str`
    ///
    /// It used to: on a match the whole chunk round-tripped through
    /// `String::from_utf8_lossy`, which replaces every invalid byte with
    /// U+FFFD. So redacting a secret silently corrupted **unrelated bytes in
    /// the same chunk** -- and the `.bytes` file is meant to be the wire, so a
    /// fixture cut from such a chunk would differ from what the server sent,
    /// with nothing to indicate it.
    ///
    /// The no-match path was always byte-exact, and
    /// `bytes_without_a_secret_are_returned_unchanged` only covered that path,
    /// so the corruption had no test. Review finding PL-5.
    ///
    /// Scanning bytes also removes the question of whether a secret is valid
    /// UTF-8: the old comment reasoned that every secret is ASCII "so that case
    /// does not arise", which was true but load-bearing. Now it is irrelevant.
    #[must_use]
    pub fn apply_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        if self.secrets.is_empty() {
            return bytes.to_vec();
        }
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        'outer: while at < bytes.len() {
            for (secret, replacement) in &self.secrets {
                let needle = secret.as_bytes();
                if bytes[at..].starts_with(needle) {
                    out.extend_from_slice(replacement.as_bytes());
                    at += needle.len();
                    continue 'outer;
                }
            }
            out.push(bytes[at]);
            at += 1;
        }
        out
    }

    /// The registered secrets, for the sink's boundary arithmetic.
    ///
    /// Crate-visible, not public: the values are credentials, and the only
    /// legitimate caller is the sink deciding where a chunk may be cut.
    pub(crate) fn secrets(&self) -> &[(String, &'static str)] {
        &self.secrets
    }

    /// The length of the longest registered secret, in bytes.
    ///
    /// The sink uses this to size the tail it carries between chunks: a secret
    /// can straddle a read boundary, and holding back `longest - 1` bytes
    /// guarantees any secret is whole in some chunk. Zero when nothing is
    /// registered.
    #[must_use]
    pub fn longest_secret(&self) -> usize {
        self.secrets
            .iter()
            .map(|(secret, _)| secret.len())
            .max()
            .unwrap_or(0)
    }

    /// Whether anything is being redacted. For a startup line that says so,
    /// because a log that *claims* to be scrubbed and is not is worse than one
    /// that admits it is raw.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }

    /// What is registered, named by KIND, for the log header.
    ///
    /// Read off the replacement labels rather than written as a fixed phrase,
    /// because the fixed phrase was wrong: the header said "account and
    /// character" whenever anything was registered, and the character is
    /// never registered -- `main` registers the account alone (review finding
    /// 9). A header that overstates what was scrubbed is the dangerous
    /// direction, the same one the 2026-09-19 fix in `writer.rs` corrected
    /// for keys.
    pub(super) fn kinds(&self) -> String {
        const KNOWN: [(&str, &str); 3] = [
            ("<ACCOUNT>", "the account name"),
            ("<NAME>", "the account holder's real name"),
            ("<KEY>", "a session key"),
        ];
        let mut kinds: Vec<&str> = KNOWN
            .into_iter()
            .filter(|(label, _)| self.secrets.iter().any(|(_, r)| r == label))
            .map(|(_, kind)| kind)
            .collect();
        // `add` is public and takes any label, so a set can hold kinds this
        // table does not name. Say so rather than print an empty list.
        if self
            .secrets
            .iter()
            .any(|(_, r)| !KNOWN.iter().any(|(label, _)| label == r))
        {
            kinds.push("other registered strings");
        }
        kinds.join(", ")
    }
}
