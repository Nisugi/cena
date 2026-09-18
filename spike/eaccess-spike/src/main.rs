//! EAccess login spike — `plan/10-eaccess-spec.md` §11.
//!
//! Throwaway. Proves we can authenticate against Simutronics' SGE service and
//! reach game text. No config, no session model, no reconnect, no disk writes.
//!
//! PASS CRITERION (§11.2): prints recognizable game text — the login banner or a
//! room description — proving XML mode was enabled. A TCP accept proves nothing.
//!
//! Everything here is built on measurements taken 2026-09-18 (`plan/10` §12.1),
//! not on inference:
//!   S1  Lich sends NO SNI. We suppress it too, because eaccess.play.net sits
//!       behind an AWS load balancer that may route on it.
//!   S2  TLS 1.2, TLS_RSA_WITH_AES_128_GCM_SHA256 (static RSA). rustls refuses
//!       to implement static-RSA key exchange, hence native-tls.
//!   S3  The hash can leave 0..=255. Ruby RAISES there, so Lich has never sent
//!       such a byte and the server's behavior is unobserved. We replicate the
//!       failure rather than wrapping — wrapping would be an untested protocol
//!       change disguised as a port.
//!   S4  NO response carries a terminator. Not \n, not \r\n. Framing dispatches
//!       on the leading command letter.

use native_tls::TlsConnector;
use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const EACCESS_HOST: &str = "eaccess.play.net";
const EACCESS_PORT: u16 = 7910;
const READ_BUF: usize = 8192;
const STAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// Named stage, so a failure says WHICH step broke rather than "login failed".
/// Lich does this (`eaccess.rb` `stage()`) because three of seven historical
/// incidents were silent hangs; §11.2 requires the same.
#[derive(Debug)]
struct SpikeError {
    stage: &'static str,
    detail: String,
}

impl std::fmt::Display for SpikeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[stage: {}] {}", self.stage, self.detail)
    }
}

impl std::error::Error for SpikeError {}

fn err<E: std::fmt::Display>(stage: &'static str, e: E) -> SpikeError {
    SpikeError { stage, detail: e.to_string() }
}

type R<T> = Result<T, SpikeError>;

/// The SGE password hash (`eaccess.rb:216-219`):
///
/// ```text
/// password[i] = ((password[i] - 32) ^ hashkey[i]) + 32
/// ```
///
/// S3: Ruby raises on both overflow and underflow — 14,336 of 65,536 byte pairs.
/// We return an error in exactly those cases instead of wrapping, because
/// wrapping emits bytes the server has never been observed to accept.
///
/// Also guards the short-key case Ruby leaves open: Ruby indexes `key[i]` for
/// `i` over the *password* length and raises on nil; Rust's `zip` would silently
/// truncate, producing a wrong password rather than an error.
fn hash_password(password: &[u8], key: &[u8]) -> R<Vec<u8>> {
    if key.len() < password.len() {
        return Err(err(
            "hash",
            format!(
                "key ({} bytes) shorter than password ({} bytes) — Ruby raises here; refusing to truncate",
                key.len(),
                password.len()
            ),
        ));
    }

    let mut out = Vec::with_capacity(password.len());
    for (i, (&p, &k)) in password.iter().zip(key.iter()).enumerate() {
        let intermediate = (p as i32 - 32) ^ (k as i32);
        let result = intermediate + 32;
        if !(0..=255).contains(&result) {
            return Err(err(
                "hash",
                format!(
                    "byte {i} out of range: ((0x{p:02x} - 32) ^ 0x{k:02x}) + 32 = {result}. \
                     Ruby raises here and Lich has never sent such a byte, so the server's \
                     behavior is UNOBSERVED (plan/10 §12.1 S3). Refusing to guess."
                ),
            ));
        }
        out.push(result as u8);
    }
    Ok(out)
}

/// One read. S4: responses carry no terminator, so we cannot scan for one —
/// we take what the socket gives us and dispatch on the leading command letter.
fn read_response<S: Read>(stream: &mut S, stage: &'static str) -> R<String> {
    let mut buf = vec![0u8; READ_BUF];
    let n = stream.read(&mut buf).map_err(|e| err(stage, e))?;
    if n == 0 {
        return Err(err(stage, "connection closed by peer (0 bytes)"));
    }
    Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
}

fn send<S: Write>(stream: &mut S, line: &str, stage: &'static str) -> R<()> {
    // Build the whole message, then ONE write. VellumFE does this deliberately
    // (`network.rs:920-931`): "Match Ruby's puts - sends string with newline in a
    // SINGLE write ... to ensure it goes out as a single TLS record."
    //
    // Two `write_all` calls can emit two TLS records, and this server does not
    // tolerate a command split across records. Ruby's IO#puts is inherently one
    // write, so Lich never had to think about it — this is a genuine
    // Rust-vs-Ruby porting hazard.
    let mut message = Vec::with_capacity(line.len() + 1);
    message.extend_from_slice(line.as_bytes());
    message.push(b'\n');
    stream.write_all(&message).map_err(|e| err(stage, e))?;
    stream.flush().map_err(|e| err(stage, e))
}

/// Redact anything that looks like a credential before printing.
fn redact(s: &str) -> String {
    s.split('\t')
        .map(|field| {
            if field.len() == 32 && field.chars().all(|c| c.is_ascii_hexdigit()) {
                "<KEY-REDACTED>".to_string()
            } else if let Some(v) = field.strip_prefix("KEY=") {
                let _ = v;
                "KEY=<REDACTED>".to_string()
            } else {
                field.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\t")
}

fn prompt(label: &str) -> String {
    print!("{label}: ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    std::io::stdin().lock().read_line(&mut s).expect("stdin");
    s.trim_end_matches(['\r', '\n']).to_string()
}

struct LaunchPayload {
    gamehost: String,
    gameport: u16,
    key: String,
}

fn authenticate(account: &str, password: &str, character: &str, game_code: &str) -> R<LaunchPayload> {
    // --- TLS ---------------------------------------------------------------
    // S1: no SNI. `use_sni(false)` matches Lich's ClientHello, which carries no
    // server_name extension (verified: extension type 0 absent).
    //
    // The cert is self-signed with no chain of trust, so normal verification
    // cannot apply. Lich pins it instead (eaccess.rb:90-130). The spike accepts
    // it unverified and PRINTS THE FINGERPRINT so production can pin by DER
    // fingerprint — not by PEM string equality, which is line-ending sensitive
    // (plan/10 §12.3).
    let connector = TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .use_sni(false)
        .build()
        .map_err(|e| err("tls_build", e))?;

    let addr = format!("{EACCESS_HOST}:{EACCESS_PORT}");
    eprintln!("[stage: tcp_connect] {addr}");
    let tcp = TcpStream::connect(&addr).map_err(|e| err("tcp_connect", e))?;
    // VellumFE sets nodelay here (`network.rs`): without it Nagle can coalesce or
    // delay the small command writes this protocol is built from.
    tcp.set_nodelay(true).map_err(|e| err("tcp_connect", e))?;
    tcp.set_read_timeout(Some(STAGE_TIMEOUT)).map_err(|e| err("tcp_connect", e))?;
    tcp.set_write_timeout(Some(STAGE_TIMEOUT)).map_err(|e| err("tcp_connect", e))?;

    eprintln!("[stage: tls_handshake] no SNI (matching Lich)");
    let mut conn = connector.connect(EACCESS_HOST, tcp).map_err(|e| err("tls_handshake", e))?;

    // --- K: get the hash key ----------------------------------------------
    eprintln!("[stage: k_request] sending K");
    send(&mut conn, "K", "k_request")?;
    let key_raw = {
        let mut buf = vec![0u8; READ_BUF];
        let n = conn.read(&mut buf).map_err(|e| err("k_response", e))?;
        if n == 0 {
            return Err(err("k_response", "connection closed"));
        }
        buf.truncate(n);
        buf
    };
    if key_raw.is_empty() {
        return Err(err("k_response", "MALFORMED_K_RESPONSE (empty)"));
    }
    // VellumFE trims the key before hashing (`network.rs:760`,
    // `obfuscate_password(password, hash_key.trim())`) — and it is a working
    // implementation against this server. Our capture showed a 32-byte key whose
    // bytes are not all printable, so a raw read can carry framing whitespace
    // that must not enter the XOR. Trim ASCII whitespace from both ends.
    let key_start = key_raw.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(0);
    let key_end = key_raw
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(key_raw.len());
    let key_raw = key_raw[key_start..key_end].to_vec();

    eprintln!(
        "[stage: k_response] {} bytes after trim (not printed — key material)",
        key_raw.len()
    );
    // MEASURED 2026-09-18: this key came back 32 bytes, valid UTF-8, all ASCII,
    // no high bits. So raw-bytes and UTF-8-round-trip hashing are equivalent for
    // it, and VellumFE's `String::from_utf8` + `.bytes()` path (network.rs:952,
    // :975) agrees with this one byte for byte. The earlier capture's `\x7F` is
    // still ASCII (DEL), so it does not contradict this.

    // --- A: authenticate ---------------------------------------------------
    // The hashed password is ARBITRARY BYTES, not UTF-8. Routing it through a
    // String mangles every byte above 0x7F into U+FFFD, which silently corrupts
    // the credential — Ruby has no such problem because its Strings are byte
    // arrays. Build the request as raw bytes instead.
    let hashed = hash_password(password.as_bytes(), &key_raw)?;

    eprintln!("[stage: a_request] sending A ({} hashed bytes)", hashed.len());
    let mut a_req = Vec::with_capacity(account.len() + hashed.len() + 8);
    a_req.extend_from_slice(b"A\t");
    a_req.extend_from_slice(account.as_bytes());
    a_req.push(b'\t');
    a_req.extend_from_slice(&hashed);
    a_req.push(b'\n');
    conn.write_all(&a_req).map_err(|e| err("a_request", e))?;
    conn.flush().map_err(|e| err("a_request", e))?;
    let a = read_response(&mut conn, "a_response")?;
    if !a.contains("\tKEY\t") {
        // Failure path only: no KEY is present, so nothing in this response is a
        // session key and the whole thing is safe to show. plan/10 §12.3's
        // warning is about the SUCCESS path, where the last field IS the key.
        return Err(err(
            "a_response",
            format!("authentication rejected. server said: {:?}", a.trim()),
        ));
    }
    eprintln!("[stage: a_response] OK: {}", redact(a.trim()));

    // M (game list). VellumFE skips it (`network.rs:773` goes straight A -> F);
    // Lich sends it (`eaccess.rb`). Sending it is harmless and matches the
    // captured working login, so we follow Lich here.
    send(&mut conn, "M", "m_request")?;
    let m = read_response(&mut conn, "m_response")?;
    // Print it in full: M lists every instance code the server will accept, so a
    // game code missing here is a client-side typo, not a server refusal.
    eprintln!("[stage: m_response] {:?}", m.trim());
    // REFUSE, don't warn. A code missing from M is unrecognised by the server,
    // and the server does NOT say so at the point of use: `F\t{bad}` still
    // answers — about the account's default instance — so the session silently
    // proceeds pointed at the wrong game and fails four commands later at `L`
    // with PROBLEM 3. Checking here turns a four-command-deep mystery into a
    // one-line refusal naming the actual mistake.
    //
    // MEASURED 2026-09-18: `gs3` (lowercase) produced exactly that chain —
    // F=PREMIUM (not GST's FREE), C=16 slots (not GST's 100), L=PROBLEM 3.
    let listed = m
        .trim()
        .split('\t')
        .skip(1)
        .step_by(2)
        .any(|code| code == game_code);
    if !listed {
        let codes: Vec<&str> = m.trim().split('\t').skip(1).step_by(2).collect();
        return Err(err(
            "m_response",
            format!(
                "game code {game_code:?} is not offered by the server. Codes are \
                 CASE-SENSITIVE; the server offers: {}. An unrecognised code does not \
                 fail here — F answers about the account's default instance instead, \
                 and the login fails later at L with an unrelated-looking PROBLEM.",
                codes.join(", ")
            ),
        ));
    }

    // --- F: entitlement ----------------------------------------------------
    send(&mut conn, &format!("F\t{game_code}"), "f_request")?;
    let f = read_response(&mut conn, "f_response")?;
    // Check the response ANSWERS THE COMMAND WE SENT before reading its content.
    // Every response here is `<letter>\t...`, echoing the command letter, so a
    // mismatch means the read stream has slipped out of step with the write
    // stream — and every subsequent field read is then meaningless. This is the
    // check whose absence made a lowercase game code look like an entitlement
    // problem, then a pricing problem, then a session-state problem.
    if !f.starts_with("F\t") {
        return Err(err(
            "f_response",
            format!("expected an F response, got {:?} — read/write streams are out of step", f.trim()),
        ));
    }
    if !["NORMAL", "PREMIUM", "TRIAL", "INTERNAL", "FREE"].iter().any(|t| f.contains(t)) {
        return Err(err("f_response", format!("no entitlement for {game_code}: {}", f.trim())));
    }
    // Print the RAW response, not a parsed field. An earlier version printed
    // `g.split('\t').nth(2)`, which turned the server's bare "?" (its
    // bad-command reply, docs/eaccess-protocol-analysis.md:274) into the
    // innocuous-looking `tier="?"` — a rejected command rendered as a missing
    // field. Do not parse a response before you have seen it fail.
    eprintln!("[stage: f_response] {:?}", f.trim());

    // --- G: SELECT GAME -----------------------------------------------------
    // G is the select-game command (analysis:106), not merely an info query.
    // Everything after it — C's character list, and therefore L — depends on it
    // having succeeded.
    send(&mut conn, &format!("G\t{game_code}"), "g_request")?;
    let g = read_response(&mut conn, "g_response")?;
    eprintln!("[stage: g_response] {:?}", g.trim());
    if !g.starts_with("G\t") {
        return Err(err(
            "g_response",
            format!(
                "select-game failed for {game_code}: {:?}. Nothing after this point is \
                 meaningful — C would list another instance's characters and L would \
                 refuse. A reply that does not start with \"G\\t\" may also mean the \
                 read stream has DESYNCHRONISED from the write stream (a response to an \
                 earlier command arriving here); check the game code against M first.",
                g.trim()
            ),
        ));
    }

    // P (pricing). Lich sends it (`eaccess.rb:259`) and VellumFE sends it
    // (`network.rs:777`); both discard the response, and so do we.
    //
    // TESTED AND DISPROVEN 2026-09-18: a run with `P` removed entirely still
    // produced 16 slots at C and PROBLEM 3 at L. `P` is not what moves the
    // session. (The GSX-echo in the capture that suggested otherwise remains
    // unexplained, but it is not the cause of this failure.) Restored, because
    // both working implementations send it.
    send(&mut conn, &format!("P\t{game_code}"), "p_request")?;
    let p = read_response(&mut conn, "p_response")?;
    eprintln!("[stage: p_response] {:?}", p.trim());

    // --- C: character list -------------------------------------------------
    send(&mut conn, "C", "c_request")?;
    let c = read_response(&mut conn, "c_response")?;
    eprintln!("[stage: c_response] {}", c.trim());
    if !c.starts_with("C\t") {
        return Err(err(
            "c_response",
            format!("expected a C response, got {:?} — read/write streams are out of step", c.trim()),
        ));
    }

    // The C header's second field is the account's MAX CHARACTER SLOTS on the
    // currently selected instance, and it identifies that instance:
    // GST (free) = 100 slots, premium = 16
    // (docs/eaccess-protocol-analysis.md:166, :293, :301).
    //
    // This is the diagnostic that cracked PROBLEM 3: a spike run showed 16 while
    // the working Lich login showed 100, for the SAME account and character —
    // proving the session had drifted off GST before `C`. Print it so a future
    // instance drift is visible immediately instead of surfacing as an
    // unexplained refusal one step later.
    if let Some(slots) = c.trim().split('\t').nth(2) {
        eprintln!(
            "[stage: c_response] max slots={slots} (GST free=100, premium=16 — \
             identifies the SELECTED instance, not the requested one)"
        );
    }

    // Format: C \t n \t n \t n \t n \t <code> \t <Name> [\t <code> \t <Name>]...
    let fields: Vec<&str> = c.trim().split('\t').collect();
    let mut char_code = None;
    let mut i = 5;
    while i + 1 < fields.len() {
        if fields[i + 1].eq_ignore_ascii_case(character) {
            char_code = Some(fields[i]);
            break;
        }
        i += 2;
    }
    let char_code = char_code.ok_or_else(|| {
        err("resolve_char", format!("character {character:?} not on this account"))
    })?;
    // {:?} so any stray whitespace or control byte in the parsed code is visible
    // rather than invisibly breaking the L request.
    eprintln!("[stage: resolve_char] code={:?}", char_code);

    // --- L: launch ---------------------------------------------------------
    let l_req = format!("L\t{char_code}\tSTORM");
    eprintln!("[stage: l_request] sending {:?}", l_req);
    send(&mut conn, &l_req, "l_request")?;
    let l = read_response(&mut conn, "l_response")?;
    // The success guard MUST be `L\tOK\t`, not `^L\t`. A refusal is
    // `L\tPROBLEM\t<n>`, which also starts with `L\t` — Lich's own analysis
    // (`docs/eaccess-protocol-analysis.md:196`) records that a loose guard
    // accepts it and then parses a garbage launch payload.
    if !l.starts_with("L\tOK") {
        let detail = if l.contains("PROBLEM") {
            // Lich documents PROBLEM 1 only ("no entitlement to create on this
            // instance", docs/eaccess-protocol-analysis.md:196). PROBLEM 3 is
            // NOT in Lich's docs; what follows is this project's own finding,
            // labelled INFERRED because it rests on one observation.
            //
            // INFERRED 2026-09-18: PROBLEM 3 means the character code is not
            // valid on the CURRENTLY SELECTED instance. Evidence: a spike run
            // and a working Lich login used the same account, character and
            // character code (W_ACCOUNT_000) and the same L bytes; the spike's
            // C header reported 16 slots and Lich's reported 100, so the spike
            // was on a premium instance while Lich was on GST. Byte-diffing the
            // L request finds nothing — the bytes are identical and only the
            // session state differs.
            format!(
                "launch refused ({}). PROBLEM 1 (no creation entitlement) is the \
                 only code Lich documents. PROBLEM 3 is INFERRED to mean the \
                 character code is not valid on the selected instance — check the \
                 max-slot count printed at c_response: 100 means GST, 16 means a \
                 premium instance. If it disagrees with the game code you asked \
                 for, the session drifted before C.",
                l.trim()
            )
        } else {
            format!("launch refused: {}", l.trim())
        };
        return Err(err("l_response", detail));
    }
    eprintln!("[stage: l_response] {}", redact(l.trim()));

    // plan/10 §12.3: splitn(2, '=') — a KEY value could itself contain '='.
    let mut gamehost = None;
    let mut gameport = None;
    let mut key = None;
    for field in l.trim().split('\t') {
        let mut kv = field.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("GAMEHOST"), Some(v)) => gamehost = Some(v.to_string()),
            (Some("GAMEPORT"), Some(v)) => gameport = v.parse::<u16>().ok(),
            (Some("KEY"), Some(v)) => key = Some(v.to_string()),
            _ => {}
        }
    }

    Ok(LaunchPayload {
        gamehost: gamehost.ok_or_else(|| err("l_response", "no GAMEHOST"))?,
        gameport: gameport.ok_or_else(|| err("l_response", "no GAMEPORT"))?,
        key: key.ok_or_else(|| err("l_response", "no KEY"))?,
    })
}

/// Connect to the game and send the three-part handshake, then print what comes
/// back. THIS is the pass criterion — recognizable game text, not a TCP accept.
fn connect_game(p: &LaunchPayload) -> R<()> {
    let addr = format!("{}:{}", p.gamehost, p.gameport);
    eprintln!("\n[stage: game_connect] {addr}");
    let mut sock = TcpStream::connect(&addr).map_err(|e| err("game_connect", e))?;
    sock.set_read_timeout(Some(STAGE_TIMEOUT)).map_err(|e| err("game_connect", e))?;

    // key\n, then the client descriptor (`plan/10` §7.4).
    //
    // THE BANNER IS NOT COSMETIC. `/FE:WRAYTH /VERSION:1.0.1.28` is what makes
    // the server serve the EXTENDED FEED — `<pulse>`, `<exposeContainer>`, and
    // `<inventoryManager>` in reply to `_inventory manager`. There is no other
    // negotiation: the server keys on this string alone.
    //
    // CONFIRMED by the author 2026-09-18; live-verified 2026-08-12 against the
    // real server, and independently corroborated by the login capture in
    // `crates/cena-protocol/tests/fixtures/login_setup.xml`, where the server
    // echoes `<settingsInfo client='1.0.1.28' .../>`.
    //
    // This spike previously sent `/FE:STORMFRONT /VERSION:1.0.1.26`, which is a
    // Lich-era banner and gets the REDUCED feed. `plan/10` §7.4 already
    // specified WRAYTH 1.0.1.28 in four places; only this file was stale.
    sock.write_all(format!("{}\n", p.key).as_bytes()).map_err(|e| err("game_handshake", e))?;
    sock.write_all(b"/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML\n")
        .map_err(|e| err("game_handshake", e))?;
    sock.flush().map_err(|e| err("game_handshake", e))?;

    // Two `<c>` ready signals, ~300 ms apart.
    //
    // > **RESTORED 2026-09-18.** These were removed earlier the same day on
    // > Saga-research reasoning -- "GemStone sends no `<c>`; that is a
    // > DragonRealms thing" -- and that was WRONG. VellumFE, a working
    // > GemStone client against these same servers, sends them
    // > **unconditionally with no DR branch**
    // > (`reference/VellumFE/src/network.rs:689-694`: "Send ready signals -
    // > game server expects two `<c>` signals with delay").
    // >
    // > The removal was never run against the live server: the spike's VERIFIED
    // > marks predate it. `CLAUDE.md` says to read VellumFE FIRST, before Lich,
    // > before the spec, before theorising -- and records that this rule was
    // > already broken three times during this very spike, each time with the
    // > answer sitting in `network.rs`. This was the fourth.
    // > **AND NOW TESTED, not taken on anyone's word.** Vellum is evidence, not
    // > proof -- believing it uncritically is the same error as believing the
    // > Saga inference, pointed the other way. `CENA_SKIP_C=1` omits them, so
    // > one run each settles it against the live server:
    // >
    // >   * both runs reach game text  -> the signals are HARMLESS but not
    // >     required; Vellum sends them defensively and so should we.
    // >   * skip-run hangs or returns no markup -> they are REQUIRED. Vellum
    // >     is right and the removal would have broken login.
    // >   * skip-run works and the send-run does NOT -> they are HARMFUL, and
    // >     the Saga reading was right after all.
    let skip_c = std::env::var("CENA_SKIP_C").is_ok_and(|v| v == "1");
    if skip_c {
        eprintln!("[stage: game_handshake] CENA_SKIP_C=1 -- sending NO <c> signals");
    } else {
        eprintln!("[stage: game_handshake] sending two <c> ready signals, 300ms apart");
        for _ in 0..2 {
            sock.write_all(b"<c>
").map_err(|e| err("game_handshake", e))?;
            sock.flush().map_err(|e| err("game_handshake", e))?;
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    eprintln!("[stage: game_read] reading up to 2 KB\n");
    println!("{}", "=".repeat(70));

    let mut total = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut buf = vec![0u8; 2048];
    while total.len() < 2048 && Instant::now() < deadline {
        match sock.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => total.extend_from_slice(&buf[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(err("game_read", e)),
        }
    }

    let text = String::from_utf8_lossy(&total);
    println!("{text}");
    println!("{}", "=".repeat(70));

    // §11.2: XML mode proven, not merely connected.
    let looks_like_game = text.contains("<mode")
        || text.contains("GemStone")
        || text.contains("DragonRealms")
        || text.contains("<settingsInfo")
        || text.contains("<app");

    if looks_like_game {
        eprintln!("\nPASS — game text received, XML mode enabled ({} bytes).", total.len());
        Ok(())
    } else {
        Err(err(
            "pass_criterion",
            format!("connected but no recognizable game text in {} bytes", total.len()),
        ))
    }
}

fn main() {
    eprintln!("EAccess login spike — plan/10 §11. Throwaway; writes nothing to disk.\n");

    let account = prompt("account");
    let password = prompt("password");
    let character = prompt("character");
    // Game codes are CASE-SENSITIVE on the wire. `M` lists them uppercase
    // (GS3, GST, GSX, DR, ...) and a lowercase code is not recognised — but the
    // failure is silent and misleading: F still answers (about the account's
    // DEFAULT instance, not the one asked for), so the session proceeds pointed
    // at the wrong game and only fails four commands later at L.
    //
    // Uppercasing here is a convenience for the human at the prompt. The
    // protocol layer must NOT do this silently — see plan/10 §4.4a.
    let game_code = {
        let g = prompt("game code [GST]");
        let g = if g.is_empty() { "GST".to_string() } else { g };
        let upper = g.to_ascii_uppercase();
        if upper != g {
            eprintln!("[input] game code {g:?} -> {upper:?} (codes are case-sensitive)");
        }
        upper
    };
    eprintln!();

    let started = Instant::now();
    match authenticate(&account, &password, &character, &game_code) {
        Ok(payload) => {
            eprintln!(
                "\n[auth complete in {:?}] GAMEHOST={} GAMEPORT={} KEY=<REDACTED>",
                started.elapsed(),
                payload.gamehost,
                payload.gameport
            );
            if let Err(e) = connect_game(&payload) {
                eprintln!("\nFAIL {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            // §11.2: a wrong password must fail cleanly in under 2s, naming the
            // stage — not hang.
            eprintln!("\nFAIL {e}");
            eprintln!("(elapsed {:?})", started.elapsed());
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hash is symmetric under XOR, so hashing twice returns the original.
    /// Uses the REAL key shape observed 2026-09-18: 32 bytes, NOT printable
    /// ASCII (the captured key contained 0x7F).
    #[test]
    fn hash_round_trips() {
        let key: Vec<u8> = (0u8..32).map(|i| i.wrapping_mul(7).wrapping_add(0x30)).collect();
        let pw = b"hunter2";
        let once = hash_password(pw, &key).expect("in range");
        let twice = hash_password(&once, &key).expect("in range");
        assert_eq!(&twice, pw, "XOR must be its own inverse");
    }

    /// S3: we must REFUSE out-of-range rather than wrap. Ruby raises here, so
    /// the server has never seen such a byte.
    #[test]
    fn refuses_out_of_range_instead_of_wrapping() {
        // (0x20 - 32) ^ 0xFF = 255, + 32 = 287 -> out of range.
        let e = hash_password(&[0x20], &[0xFF]).expect_err("must refuse");
        assert_eq!(e.stage, "hash");
        assert!(e.detail.contains("out of range"), "got: {}", e.detail);
    }

    /// Ruby raises on a short key; Rust's zip would silently truncate.
    #[test]
    fn refuses_short_key() {
        let e = hash_password(b"longpassword", b"key").expect_err("must refuse");
        assert!(e.detail.contains("shorter"), "got: {}", e.detail);
    }

    /// Session keys and KEY= fields must never reach stdout.
    #[test]
    fn redacts_credentials() {
        let line = "L\tOK\tGAMEHOST=storm.gs4.game.play.net\tKEY=9ac77c189205275c1b604953d7e2b6aa";
        let out = redact(line);
        assert!(!out.contains("9ac77c189205275c1b604953d7e2b6aa"), "leaked: {out}");
        assert!(out.contains("GAMEHOST=storm.gs4.game.play.net"));
    }
}
