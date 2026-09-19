//! The handshake itself: the `K A M F G P C L` sequence over TLS.
//!
//! Everything here touches the **eaccess** socket. The pure half -- the types,
//! the hash and the field parsers -- is in [`super::wire`], and that split is
//! what makes the parsers testable at all (`plan/05` Rule 4.4: a `mod.rs`
//! re-exports and wires, it does not implement; the architecture test caught
//! this file sitting in `mod.rs` and named the rule). The *game* socket that
//! this sequence's [`LaunchPayload`] points at is in [`super::game`], split
//! out when this file passed the 400-line cap.
//!
//! Read [`super`] for why `EAccess` lives in `cena-platform` and for the BUILT,
//! NOT RUN rule that governs every function below.

use super::refusal::{describe_launch_refusal, launch_refusal_is_fatal};
use super::wire::hash_password;
use super::wire::{
    Credentials, EACCESS_HOST, EACCESS_PORT, EaccessError, LaunchPayload, READ_BUF, err,
    expect_echo, is_launch_ok, offered_game_codes, parse_launch, redact, redact_char_code,
    resolve_char_code,
};
use crate::bytes::ByteSource;
use crate::live::LiveSource;

/// How long one stage may wait for its answer.
///
/// **`plan/10` §2.1 requires this in bold:** *"There are no read timeouts
/// anywhere ... **Cena must add an explicit per-read deadline that Lich
/// lacks.**"* Lich has only a 30s whole-exchange watchdog, and the TLS
/// handshake plus every protocol read sit unbounded underneath it.
///
/// 10s, matching the spike's `STAGE_TIMEOUT`
/// (`spike/eaccess-spike/src/main.rs:30`), which is the value a working login
/// was measured against.
///
/// # This is a deadline, and a deadline is not `set_read_timeout`
///
/// This module's header explains why the spike's `set_read_timeout` did not
/// port: `plan/12` §5.5 wants every wait cancellable and a socket option is
/// not. **That argument was then over-applied** -- the port removed the
/// mechanism and kept only the objection to it, leaving every read unbounded,
/// which §5.5's other half ("every wait has a deadline; no unbounded
/// `await`") forbids just as plainly. Found by adversarial review.
///
/// `tokio::time::timeout` satisfies both: it is a deadline *and* it is
/// cancellable, because dropping the future cancels the read with it. The two
/// goals were never in conflict.
const STAGE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// One read, bounded by [`STAGE_DEADLINE`].
///
/// S4 (`plan/10` §12.1): **no response carries a terminator** -- not `\n`, not
/// `\r\n` -- so there is nothing to scan for. We take what the socket gives
/// and dispatch on the leading command letter.
///
/// The deadline names the stage it expired in. Without it a server that
/// accepts a command and never answers leaves the program sitting with the
/// *previous* stage's banner on screen, and the author cannot tell a hang from
/// a slow link -- which also breaks the spike's own pass criterion, that a bad
/// login "must fail cleanly in under 2s, naming the stage -- not hang"
/// (`spike/eaccess-spike/src/main.rs:599-600`).
async fn read_response(conn: &mut LiveSource, stage: &'static str) -> Result<String, EaccessError> {
    let mut buf = vec![0u8; READ_BUF];
    let read = tokio::time::timeout(STAGE_DEADLINE, conn.read(&mut buf))
        .await
        .map_err(|_elapsed| {
            err(
                stage,
                format!(
                    "no response within {STAGE_DEADLINE:?}. The command was \
                     accepted but never answered, or the link stalled. Lich \
                     bounds only the whole exchange (30s) and leaves each read \
                     unbounded; plan/10 §2.1 requires this per-read deadline."
                ),
            )
        })?;
    let n = read.map_err(|e| err(stage, e))?;
    if n == 0 {
        return Err(err(stage, "connection closed by peer (0 bytes)"));
    }
    Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
}

/// Send one line, as **one** write.
///
/// [`ByteSource::write_all`] carries the single-write contract; this just adds
/// the newline to the same buffer rather than writing it separately.
async fn send(conn: &mut LiveSource, line: &str, stage: &'static str) -> Result<(), EaccessError> {
    let mut message = Vec::with_capacity(line.len() + 1);
    message.extend_from_slice(line.as_bytes());
    message.push(b'\n');
    conn.write_all(&message).await.map_err(|e| err(stage, e))
}

/// Run the full `K A M F G P C L` handshake and return where the game is.
///
/// Each step's diagnostics go to `progress`, which receives already-redacted
/// lines. A caller that wants them on stderr passes a closure that prints; a
/// caller that wants them nowhere passes one that drops them. Taking a sink
/// rather than printing directly keeps this module free of a policy about
/// where a program's output goes -- and it is what lets the tests below exist
/// at all.
///
/// # Errors
///
/// [`EaccessError`], naming the stage. The failure paths are deliberately
/// specific: a game code the server does not offer, a launch refusal, and a
/// character that is not on the account each produce their own message rather
/// than a generic rejection.
///
/// # Panics
///
/// Does not panic.
pub async fn authenticate(
    creds: Credentials<'_>,
    mut progress: impl FnMut(&str),
) -> Result<LaunchPayload, EaccessError> {
    // Three weakenings, all required, all documented on `connect_tls`: no SNI,
    // no cert verification, no hostname check. The cert is self-signed with no
    // chain, so there is nothing to verify against; we do NOT pin, which
    // `connect_tls` records as the cost.
    progress(&format!(
        "[stage: tls_handshake] {EACCESS_HOST}:{EACCESS_PORT}, no SNI (matching Lich)"
    ));
    let mut conn = LiveSource::connect_tls(EACCESS_HOST, EACCESS_PORT)
        .await
        .map_err(|e| err("tls_handshake", e))?;

    prove_identity(&mut conn, creds, &mut progress).await?;
    select_instance(&mut conn, creds, &mut progress).await?;
    let char_code = resolve_character(&mut conn, creds, &mut progress).await?;
    let launch = launch_character(&mut conn, &char_code, &mut progress).await?;

    // The eaccess socket has done its job. Close it: the game socket is a
    // different connection, and leaving this one open is a leaked socket
    // (criterion 6) for the whole life of the session.
    let _ = conn.shutdown().await;

    Ok(launch)
}

/// `K` then `A`: get the server's hash key, and prove we know the password.
///
/// Split out of [`authenticate`] under `plan/05` Rule 4.1 -- **move code down,
/// do not raise the cap.** The eight-step sequence ran to 107 lines against a
/// 100-line limit, and held six single-letter bindings in one scope (`a`, `m`,
/// `f`, `g`, `c`, `l` -- one per protocol letter). Splitting fixes both, and
/// the letters stay: the response to `A` being called `a` is the clearest name
/// available, once only one of them is in scope at a time.
async fn prove_identity(
    conn: &mut LiveSource,
    creds: Credentials<'_>,
    progress: &mut impl FnMut(&str),
) -> Result<(), EaccessError> {
    send(conn, "K", "k_request").await?;
    // Read as BYTES, not through `read_response`: the key is key material and
    // need not be valid UTF-8 (the 2026-09-18 capture contained 0x7F), so a
    // `from_utf8_lossy` round trip could alter it before the XOR. Bounded by
    // the same [`STAGE_DEADLINE`] as every other read.
    let key_raw = {
        let mut buf = vec![0u8; READ_BUF];
        let n = tokio::time::timeout(STAGE_DEADLINE, conn.read(&mut buf))
            .await
            .map_err(|_elapsed| {
                err(
                    "k_response",
                    format!("no hash key within {STAGE_DEADLINE:?}"),
                )
            })?
            .map_err(|e| err("k_response", e))?;
        if n == 0 {
            return Err(err("k_response", "connection closed reading the hash key"));
        }
        buf.truncate(n);
        buf
    };
    // **The key is used WHOLE. Nothing is trimmed off it.**
    //
    // This used to call `trim_ascii_whitespace`, citing VellumFE
    // (`network.rs:760`, `hash_key.trim()`) as a working implementation against
    // this server. Vellum does do that, and it is still wrong -- `plan/10`
    // measured why, in the capture that settled "the single most dangerous open
    // question":
    //
    //   | K | 32 | 32 random bytes, **not printable ASCII** (contained DEL) |
    //                                                        (`plan/10:1734`)
    //
    // and `:1749`: "`K` has no trailing newline, so the hash loop consumes
    // exactly the bytes sent." The key is 32 bytes of RANDOM BINARY with no
    // terminator, so a leading `0x09`, `0x0A`, `0x0D` or `0x20` is an ordinary
    // key byte and not framing.
    //
    // Trimming one shifts every XOR index by one. Nothing errors: a WRONG
    // PASSWORD goes to the auth server, which answers `PASSWORD`, which is a
    // FATAL stop plus one bad-password strike against the account.
    //
    // 4 of 256 first bytes are whitespace, so this is roughly one login in 64 --
    // a coin-flip that has not come up yet, not an exotic case. The trim bought
    // nothing in either direction, which is what makes removing it free.
    //
    // `crates/cena-platform/tests/eaccess_mandated_vectors.rs` pins it.
    let key = key_raw.as_slice();
    if key.is_empty() {
        return Err(err("k_response", "MALFORMED_K_RESPONSE (empty)"));
    }
    progress(&format!(
        "[stage: k_response] {} bytes, used whole (not printed -- key material)",
        key.len()
    ));

    // The hashed password is ARBITRARY BYTES, not UTF-8 (`plan/10` §10.3a
    // hazard 2). Routing it through a String mangles every byte above 0x7F
    // into U+FFFD, silently corrupting the credential. Ruby has no such
    // problem because its Strings are byte arrays. Build the request as bytes.
    let hashed = hash_password(creds.password.as_bytes(), key)?;
    let mut a_req = Vec::with_capacity(creds.account.len() + hashed.len() + 8);
    a_req.extend_from_slice(b"A\t");
    a_req.extend_from_slice(creds.account.as_bytes());
    a_req.push(b'\t');
    a_req.extend_from_slice(&hashed);
    a_req.push(b'\n');
    conn.write_all(&a_req)
        .await
        .map_err(|e| err("a_request", e))?;

    let a = read_response(conn, "a_response").await?;
    if !a.contains("\tKEY\t") {
        // **Two cases, and they do NOT share a verdict.** This used to mark
        // every keyless `A` fatal and echo the raw body. Both halves were wrong;
        // see `super::reject` for the full argument and Lich's precedent.
        //
        // In short: a recognised rejection token is the credentials being wrong,
        // which retrying cannot fix and which costs a strike. Anything else is
        // "we do not know what that was", where stopping strands a session whose
        // credentials are good -- the direction `wire.rs` itself calls unsafe.
        let rejection = super::reject::classify_a_rejection(&a);
        let detail = match &rejection {
            super::reject::Rejection::Credentials(token) => {
                format!("authentication rejected by the server: {token}")
            }
            // The body is deliberately absent: an unrecognised reply is the case
            // most likely to be a WAF intercept page or a truncated fragment,
            // and a length is all a diagnostic needs.
            super::reject::Rejection::Divergence { bytes } => format!(
                "unrecognised response to A ({bytes} bytes, body not logged -- it \
                 may be an intercept page). Treated as TRANSIENT: this is not \
                 evidence the credentials are wrong, and stopping here would \
                 strand a session a retry could recover."
            ),
        };
        let error = err("a_response", detail);
        return Err(if rejection.is_fatal() {
            error.fatal()
        } else {
            error
        });
    }
    progress(&format!("[stage: a_response] OK: {}", redact(a.trim())));
    Ok(())
}

/// `M`, `F`, `G`, `P`: check the instance exists, that the account may enter
/// it, and select it.
///
/// **`M` is used as a guard, not for information.** Lich sends it; `VellumFE`
/// skips it (`network.rs:773` goes A -> F). We send it because it is the only
/// place the server will tell us a game code is wrong.
///
/// REFUSE, don't warn. A code missing from `M` is unrecognised, and the server
/// does **not** say so at the point of use: `F\t{bad}` still answers -- about
/// the account's DEFAULT instance -- so the session silently proceeds pointed
/// at the wrong game and fails four commands later at `L` with PROBLEM 3.
/// MEASURED 2026-09-18: `gs3` lowercase produced exactly that chain
/// (F=PREMIUM, C=16 slots, L=PROBLEM 3).
async fn select_instance(
    conn: &mut LiveSource,
    creds: Credentials<'_>,
    progress: &mut impl FnMut(&str),
) -> Result<(), EaccessError> {
    send(conn, "M", "m_request").await?;
    let m = read_response(conn, "m_response").await?;
    expect_echo(&m, 'M', "m_response")?;
    let codes = offered_game_codes(&m);
    if !codes.contains(&creds.game_code) {
        // FATAL: the server's own list of instances does not contain this code.
        // No number of retries adds one.
        return Err(err(
            "m_response",
            format!(
                "game code {:?} is not offered by the server. Codes are \
                 CASE-SENSITIVE; the server offers: {}. An unrecognised code \
                 does not fail here -- F answers about the account's default \
                 instance instead, and the login fails later at L with an \
                 unrelated-looking PROBLEM.",
                creds.game_code,
                codes.join(", ")
            ),
        ));
    }
    // The WHOLE line, not just the parsed codes. `offered_game_codes` keeps
    // every other field, which drops the human-readable instance names -- and
    // those are what tell the author at a glance whether GST is the instance
    // they meant. The spike printed the full response (`:259`); this run is
    // one-shot, so a diagnostic dropped here cannot be recovered without a
    // second live login.
    progress(&format!("[stage: m_response] {:?}", m.trim()));
    progress(&format!("[stage: m_response] codes: {}", codes.join(", ")));

    // F: entitlement.
    send(conn, &format!("F\t{}", creds.game_code), "f_request").await?;
    let f = read_response(conn, "f_response").await?;
    expect_echo(&f, 'F', "f_response")?;
    if !["NORMAL", "PREMIUM", "TRIAL", "INTERNAL", "FREE"]
        .iter()
        .any(|t| f.contains(t))
    {
        // FATAL: an entitlement is account state. The fix is a subscription,
        // not another login -- the same reasoning `refusal.rs` gives for
        // PROBLEM 1.
        return Err(err(
            "f_response",
            format!("no entitlement for {}: {}", creds.game_code, f.trim()),
        )
        .fatal());
    }
    // The RAW response, not a parsed field: parsing before seeing it fail
    // turned the server's bare "?" into an innocuous `tier="?"`.
    progress(&format!("[stage: f_response] {:?}", f.trim()));

    // G SELECTS the game; it is not merely an info query (`plan/10` §4.4).
    // Everything after it -- C's character list, and therefore L -- depends on
    // it having succeeded.
    send(conn, &format!("G\t{}", creds.game_code), "g_request").await?;
    let g = read_response(conn, "g_response").await?;
    expect_echo(&g, 'G', "g_response")?;
    progress(&format!("[stage: g_response] {:?}", g.trim()));

    // P: pricing. Lich sends it (`eaccess.rb:259`), VellumFE sends it
    // (`network.rs:777`), both discard the response. TESTED AND DISPROVEN
    // 2026-09-18 that it is what moves the session: a run with P removed still
    // produced 16 slots and PROBLEM 3. Kept because both working
    // implementations send it.
    send(conn, &format!("P\t{}", creds.game_code), "p_request").await?;
    let p = read_response(conn, "p_response").await?;
    // PRINTED, not discarded. `plan/10` §12.2 files an open anomaly that only
    // this response shows: in the 2026-09-18 capture, `P` asked about **GST**
    // and answered `P\tGSX\t1000\tGSX.EC\t-1\tGS3.P\t2500` -- GST's documented
    // pricing under a **GSX** code, still unexplained. The spike printed it
    // (`:346`); discarding it here would remove the one place a reader could
    // ever see the echo recur.
    progress(&format!("[stage: p_response] {:?}", p.trim()));
    Ok(())
}

/// `C`: list the account's characters on the selected instance and find the
/// one asked for.
async fn resolve_character(
    conn: &mut LiveSource,
    creds: Credentials<'_>,
    progress: &mut impl FnMut(&str),
) -> Result<String, EaccessError> {
    send(conn, "C", "c_request").await?;
    let c = read_response(conn, "c_response").await?;
    expect_echo(&c, 'C', "c_response")?;
    // The second field is the account's MAX CHARACTER SLOTS.
    //
    // **It reflects the account's ENTITLEMENT, not which instance was
    // selected.** An earlier version of this line claimed the number
    // "identifies the SELECTED instance" and glossed 100 as GST and 16 as
    // premium -- which is wrong on any account holding more than one
    // entitlement, and the author's holds Shattered and Premium. `M` offers it
    // ten codes; 16 is simply what its tier grants, on several of them.
    //
    // What the spike actually observed was a *contrast*: 16 where a working
    // Lich login showed 100, for the same account and character. The signal
    // was the disagreement, never the value. A bare count identifies nothing.
    //
    // Context for a human reading a failure, not a check.
    //
    // This used to say the instance "is confirmed by F, G and P echoing the
    // code that was sent, which `expect_echo` already enforces". Three things
    // wrong with that (review PL-7):
    //
    // * `expect_echo` checks the command LETTER only -- `response.starts_with("G\t")`
    //   -- and never the code.
    // * `F` does not echo the code at all. It answers an entitlement word
    //   (`NORMAL`, `PREMIUM`, `TRIAL`, `INTERNAL`, `FREE`), which the arm
    //   above checks for by name.
    // * `P` is not passed to `expect_echo` anywhere; its response is
    //   deliberately discarded, as the comment at the `P` send records.
    //
    // So nothing verifies that the instance the server selected is the
    // instance that was asked for. `refusal.rs`'s PROBLEM 3 advice says a
    // rejected `G` produces exactly that failure, which is the case this
    // would catch -- but catching it means comparing `G`'s echoed field to
    // `creds.game_code`, which is a check nobody has written. Left unwritten
    // rather than half-claimed: an unverified assumption stated as a
    // guarantee is worse than one stated as an assumption.
    if let Some(slots) = c.trim().split('\t').nth(2) {
        progress(&format!(
            "[stage: c_response] max slots={slots} for {} (entitlement, not \
             instance -- a count that DISAGREES with a previous login on the \
             same code is the signal, not any particular number)",
            creds.game_code
        ));
    }

    let code = resolve_char_code(&c, creds.character).ok_or_else(|| {
        // FATAL, and the one that prompted this sweep: a MISTYPED CHARACTER
        // NAME retried every 30 seconds sends the password to eaccess
        // indefinitely, which is precisely the account-lock risk `retry.rs`
        // quotes from `VellumFE`. The author hit this case live.
        err(
            "resolve_char",
            format!(
                "character {:?} is not on this account's {} list",
                creds.character, creds.game_code
            ),
        )
        .fatal()
    })?;
    // {:?} so a stray control byte in the parsed code is visible rather than
    // invisibly breaking the L request -- and REDACTED, because a character code
    // is `W_<ACCOUNT>_<SLOT>` (`plan/10:472`) and this line printed the account
    // on every login. Same exposure `redact` was added for; it closed the `A`
    // response and left this one, because here the account is inside a field
    // rather than a field of its own.
    progress(&format!(
        "[stage: resolve_char] code={:?}",
        redact_char_code(code)
    ));
    Ok(code.to_owned())
}

/// `L`: launch, and read back where the game is.
///
/// The success guard MUST be `L\tOK`, not `^L\t`. A refusal is
/// `L\tPROBLEM\t<n>`, which also starts with `L\t`; Lich's own analysis records
/// that a loose guard accepts it and then parses a garbage launch payload
/// (`plan/10` §4.7 item 2).
async fn launch_character(
    conn: &mut LiveSource,
    char_code: &str,
    progress: &mut impl FnMut(&str),
) -> Result<LaunchPayload, EaccessError> {
    send(conn, &format!("L\t{char_code}\tSTORM"), "l_request").await?;
    let l = read_response(conn, "l_response").await?;
    if !is_launch_ok(&l) {
        // Fatal for sub-codes 1-3 and NOT for 4: `launch_refusal_is_fatal`
        // reads them one at a time, because "the account service failed while
        // assigning the character" is a hiccup and the other three are not.
        // An earlier version marked every launch refusal fatal, on the
        // strength of a summary that said all four advise against retrying;
        // a test that read the strings caught it.
        let error = err("l_response", describe_launch_refusal(&l));
        return Err(if launch_refusal_is_fatal(&l) {
            error.fatal()
        } else {
            error
        });
    }
    progress(&format!("[stage: l_response] {}", redact(l.trim())));
    parse_launch(&l)
}
