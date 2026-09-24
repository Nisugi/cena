//! The eaccess certificate pin: **trust on first use, and refuse on change.**
//!
//! eaccess.play.net presents a **self-signed certificate with no chain of
//! trust** (`plan/10` §2.3), so ordinary verification cannot succeed and
//! [`LiveSource::connect_tls`] turns it off. This module is what replaces it:
//! the first login records the certificate the server presented, and every
//! later login must present the same one **before a byte of the conversation
//! is sent** -- the password crosses this connection at `A`.
//!
//! # Where this departs from Lich and `VellumFE`, and why
//!
//! Both pin the same way on first use, into a file named `simu.pem`
//! (`reference/lich-5/lib/common/authentication/eaccess.rb:90-122`,
//! `reference/VellumFE/src/network.rs` `ensure_certificate`). Both then
//! **re-pin silently** when the certificate changes: Lich logs and calls
//! `download_pem` (`eaccess.rb:125-142`), Vellum refreshes the file when a
//! handshake fails. `plan/10` §2.3 names the cost: *"An attacker who MITMs one
//! connection installs a persistent pin"* -- and Lich carries on over the
//! unverified connection it has already opened, so that session's password
//! crosses the attacker too.
//!
//! **Here a mismatch is FATAL, and the file is not touched.** The operator is
//! told the path and that deleting it trusts the new certificate. Rotation is
//! rare; a human deciding once is cheap. Fatal rather than transient so that
//! neither the supervisor's retry ladder nor the web-login fallback
//! resubmits the password on the strength of a connection that just failed
//! the one check it had.
//!
//! # DER, compared whole; PEM, only as the file format
//!
//! Lich compares **PEM text** (`peer_cert.to_s == File.read(pem)`), which is
//! line-ending sensitive: a CRLF-normalised `simu.pem` on Windows never
//! matches, and Lich re-pins on every login forever (`plan/10` §2.3, and §12.3
//! lists it under "do not port"). Here the file is decoded to DER and the
//! bytes are compared. `plan/10` §10.2 suggests a SHA-256 of the DER; equality
//! of the DER itself is the same test without the hash, and without a
//! dependency to compute one.
//!
//! The file stays PEM, named `simu.pem`, so the format is Lich's and Vellum's
//! and a person can read it with any certificate tool. The PEM framing is
//! hand-rolled, as Vellum's is: native-tls's `Certificate::from_pem` is a stub
//! that panics on iOS.
//!
//! # What is tested, and how
//!
//! [`verify`] -- the decision and the file -- is tested directly, and
//! [`open_pinned`] is driven against a **loopback** TLS server in the test's
//! own process (`pin_tests.rs`), which covers reading the peer certificate off
//! a real handshake and refusing before returning the stream. Nothing leaves
//! the machine. What is covered by review only is the one line in
//! [`super::authenticate`] that calls [`open_pinned`] rather than
//! `connect_tls`; `connect_tls` is `pub(crate)` so no other crate can make
//! the unpinned call at all.

use super::wire::{EaccessError, err};
use crate::bytes::ByteSource;
use crate::live::LiveSource;
use base64::Engine as _;
use std::io;
use std::path::Path;

/// The pin file's name, inside the data directory.
///
/// Lich's and Vellum's name, so the file is recognisable to anyone who has
/// used either, and one either wrote could be reused.
pub const PIN_FILENAME: &str = "simu.pem";

const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
const END: &str = "-----END CERTIFICATE-----";

/// The stage every pin failure reports.
const STAGE: &str = "cert_pin";

/// Why a connection was allowed to proceed.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Pinned {
    /// There was no pin; this server's certificate is now it.
    FirstUse,
    /// The server presented exactly the pinned certificate.
    Matched,
}

/// Open the eaccess TLS connection and check its certificate against the pin
/// at `pin`, returning the connection only if it passes.
///
/// # Errors
///
/// The connect or handshake failure (transient); a certificate that could not
/// be read off the handshake (transient: nothing was sent, and a retry or the
/// web login is safe); and every [`verify`] refusal (fatal). On any error the
/// connection is shut down before returning.
pub(super) async fn open_pinned(
    host: &str,
    port: u16,
    pin: &Path,
    progress: &mut impl FnMut(&str),
) -> Result<LiveSource, EaccessError> {
    let mut conn = LiveSource::connect_tls(host, port)
        .await
        .map_err(|e| err("tls_handshake", e))?;
    let verdict = conn
        .peer_certificate_der()
        .map_err(|e| {
            err(
                STAGE,
                format!("could not read the server's certificate: {e}"),
            )
        })
        .and_then(|presented| verify(pin, &presented));
    match verdict {
        Ok(Pinned::FirstUse) => progress(&format!(
            "[stage: cert_pin] FIRST USE: no pinned certificate at {}. Trusting \
             the one this server presented and recording it there; every later \
             login must present the same certificate or it will be refused.",
            pin.display()
        )),
        Ok(Pinned::Matched) => progress(&format!(
            "[stage: cert_pin] certificate matches the pin at {}",
            pin.display()
        )),
        Err(e) => {
            // Nothing has been sent on it; close it so it is not leaked.
            let _ = conn.shutdown().await;
            return Err(e);
        }
    }
    Ok(conn)
}

/// Check `presented` (the server certificate, DER) against the pin at `path`,
/// recording it there if there is no pin yet.
///
/// # Errors
///
/// Always **fatal**, and each names `path`:
///
/// - the pinned certificate differs from the presented one -- the file is
///   left exactly as it was;
/// - the file exists and does not hold a PEM certificate;
/// - the file exists and cannot be read;
/// - there was no pin and one could not be written. Proceeding would make
///   every login a first use, which is no pin at all.
pub(super) fn verify(path: &Path, presented: &[u8]) -> Result<Pinned, EaccessError> {
    let stored = match std::fs::read(path) {
        Ok(stored) => stored,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            record(path, presented).map_err(|e| {
                refuse(format!(
                    "no certificate was pinned and one could not be recorded at \
                     {}: {e}. Refusing to log in: without a stored pin every login \
                     would be a first use, which is no pin at all.",
                    path.display()
                ))
            })?;
            return Ok(Pinned::FirstUse);
        }
        Err(e) => {
            return Err(refuse(format!(
                "the certificate pin {} exists but could not be read: {e}. \
                 Refusing to log in unpinned.",
                path.display()
            )));
        }
    };
    let pinned = pem_to_der(&stored).map_err(|why| {
        refuse(format!(
            "the certificate pin {} is not a PEM certificate ({why}). Refusing \
             to log in unpinned. Delete {} to trust the certificate the server \
             presents on the next login.",
            path.display(),
            path.display()
        ))
    })?;
    if pinned == presented {
        Ok(Pinned::Matched)
    } else {
        Err(refuse(format!(
            "the eaccess server's certificate CHANGED: it does not match the one \
             pinned in {}. Refusing to send the password over this connection -- \
             an unexpected certificate is what a man-in-the-middle looks like. \
             If Simutronics rotated the certificate, delete {} to trust the new \
             one on the next login.",
            path.display(),
            path.display()
        )))
    }
}

/// A fatal pin error.
fn refuse(detail: String) -> EaccessError {
    err(STAGE, detail).fatal()
}

/// Write `der` to `path` as PEM, creating the directory if needed.
fn record(path: &Path, der: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, der_to_pem(der))
}

/// DER to PEM: the standard armour, 64-character lines, LF endings.
pub(super) fn der_to_pem(der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = String::with_capacity(encoded.len() + encoded.len() / 64 + 64);
    pem.push_str(BEGIN);
    pem.push('\n');
    for line in encoded.as_bytes().chunks(64) {
        // base64's alphabet is ASCII, so every chunk is valid UTF-8.
        pem.push_str(&String::from_utf8_lossy(line));
        pem.push('\n');
    }
    pem.push_str(END);
    pem.push('\n');
    pem
}

/// The first PEM certificate in `pem`, as DER.
///
/// `str::lines` takes CRLF and LF alike and each line is then trimmed, so a
/// file git or an editor converted -- or left trailing spaces in -- decodes to
/// the same bytes. That insensitivity is the point of comparing DER
/// (`plan/10` §2.3).
///
/// # Errors
///
/// A description of what is wrong: not UTF-8, no `BEGIN`, no `END`, an empty
/// body, or invalid base64.
pub(super) fn pem_to_der(pem: &[u8]) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(pem).map_err(|_| "not UTF-8 text".to_owned())?;
    let mut lines = text.lines().map(str::trim);
    if !lines.by_ref().any(|l| l == BEGIN) {
        return Err(format!("no `{BEGIN}` line"));
    }
    let mut body = String::new();
    let mut ended = false;
    for line in lines {
        if line == END {
            ended = true;
            break;
        }
        body.push_str(line);
    }
    if !ended {
        return Err(format!("no `{END}` line"));
    }
    if body.is_empty() {
        return Err("the certificate block is empty".to_owned());
    }
    base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|e| format!("invalid base64: {e}"))
}

#[cfg(test)]
#[path = "pin_tests.rs"]
mod pin_tests;
