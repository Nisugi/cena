//! The certificate pin: the decision, the file, and one real handshake.
//!
//! Nothing here leaves the machine. [`verify`] is driven against files in the
//! temp directory; [`open_pinned`] against a TLS server on LOOPBACK in this
//! test's own process, the same allowance `tests/keepalive.rs` uses. The
//! server's certificate and key are throwaway fixtures made for these tests
//! (`tests/fixtures/pin/`, self-signed, `CN=cena pin test ...`); they
//! authenticate nothing.

use super::{PIN_FILENAME, Pinned, der_to_pem, open_pinned, pem_to_der, verify};
use std::path::PathBuf;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pin/");

/// A fixture file's text. Read at run time rather than embedded: the
/// architecture tests limit what `include_str!` may embed, and a `.pem` is
/// not on that list.
fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// The DER of a fixture certificate.
fn der(name: &str) -> Vec<u8> {
    pem_to_der(fixture(name).as_bytes()).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// A fresh directory for one test, and the pin path inside it. The pin sits
/// one level down so first use has to create its directory.
fn scratch(test: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cena-pin-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let pin = dir.join("data").join(PIN_FILENAME);
    (dir, pin)
}

#[test]
fn first_use_records_the_presented_certificate() {
    let (dir, pin) = scratch("first-use");
    let presented = der("loopback.cert.pem");

    assert_eq!(verify(&pin, &presented), Ok(Pinned::FirstUse));

    let stored = std::fs::read(&pin).expect("the pin was written");
    assert_eq!(
        pem_to_der(&stored),
        Ok(presented),
        "the pin is the DER presented"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_matching_certificate_proceeds() {
    let (dir, pin) = scratch("match");
    let presented = der("loopback.cert.pem");
    std::fs::create_dir_all(pin.parent().expect("has a parent")).expect("mkdir");
    std::fs::write(&pin, fixture("loopback.cert.pem")).expect("write pin");

    assert_eq!(verify(&pin, &presented), Ok(Pinned::Matched));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_changed_certificate_is_fatal_and_the_pin_is_untouched() {
    let (dir, pin) = scratch("mismatch");
    std::fs::create_dir_all(pin.parent().expect("has a parent")).expect("mkdir");
    std::fs::write(&pin, fixture("loopback.cert.pem")).expect("write pin");
    let before = std::fs::read(&pin).expect("read pin");

    let error = verify(&pin, &der("rotated.cert.pem")).expect_err("a different cert is refused");

    assert!(
        error.fatal,
        "a mismatch must stop the retry ladder: {error}"
    );
    assert_eq!(error.stage, "cert_pin");
    assert!(error.detail.contains("CHANGED"), "{error}");
    assert!(
        error.detail.contains(&format!("delete {}", pin.display())),
        "the operator is told which file to delete: {error}"
    );
    // No silent re-pin: Lich's hole (`plan/10` section 2.3).
    assert_eq!(
        std::fs::read(&pin).expect("read pin"),
        before,
        "the pin was rewritten"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn crlf_and_lf_pins_decode_to_the_same_der() {
    let presented = der("loopback.cert.pem");
    let lf = der_to_pem(&presented);
    assert!(!lf.contains('\r'), "the written form is LF");
    let crlf = lf.replace('\n', "\r\n");

    assert_eq!(pem_to_der(lf.as_bytes()), Ok(presented.clone()));
    assert_eq!(pem_to_der(crlf.as_bytes()), Ok(presented.clone()));

    // And the case that matters: a pin file an editor or git converted to CRLF
    // still MATCHES. Lich compares PEM text and re-pins on every login here.
    let (dir, pin) = scratch("crlf");
    std::fs::create_dir_all(pin.parent().expect("has a parent")).expect("mkdir");
    std::fs::write(&pin, crlf).expect("write pin");
    assert_eq!(verify(&pin, &presented), Ok(Pinned::Matched));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_garbage_pin_is_fatal_with_instructions_and_is_left_alone() {
    let valid = der_to_pem(&der("loopback.cert.pem"));
    let body_only = valid.replace("-----END CERTIFICATE-----\n", "");
    for (case, garbage) in [
        ("empty", String::new()),
        ("prose", "this is not a certificate\n".to_owned()),
        ("no end line", body_only),
        (
            "bad base64",
            "-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----\n".to_owned(),
        ),
        (
            "empty block",
            "-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----\n".to_owned(),
        ),
    ] {
        let (dir, pin) = scratch(&format!("garbage-{}", case.replace(' ', "-")));
        std::fs::create_dir_all(pin.parent().expect("has a parent")).expect("mkdir");
        std::fs::write(&pin, &garbage).expect("write pin");

        let error = verify(&pin, &der("loopback.cert.pem")).expect_err(case);

        assert!(
            error.fatal,
            "{case}: an unreadable pin must not log in: {error}"
        );
        assert_eq!(error.stage, "cert_pin", "{case}");
        assert!(
            error.detail.contains("not a PEM certificate"),
            "{case}: {error}"
        );
        assert!(
            error.detail.contains(&format!("Delete {}", pin.display())),
            "{case}: the operator is told what to do: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(&pin).expect("read pin"),
            garbage,
            "{case}: a bad pin is reported, not replaced"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Start a TLS server on loopback presenting `loopback.cert.pem`, accepting
/// any number of connections. Returns its port.
async fn loopback_server() -> u16 {
    use tokio::io::AsyncReadExt;

    let identity = native_tls::Identity::from_pkcs8(
        fixture("loopback.cert.pem").as_bytes(),
        fixture("loopback.key.pem").as_bytes(),
    )
    .expect("fixture identity");
    let acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::new(identity).expect("tls acceptor"),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let port = listener.local_addr().expect("local addr").port();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                if let Ok(mut tls) = acceptor.accept(tcp).await {
                    // Hold the connection until the client closes it.
                    let mut buf = [0u8; 64];
                    let _ = tls.read(&mut buf).await;
                }
            });
        }
    });
    port
}

/// The wiring: the certificate read off a REAL handshake is the one pinned,
/// the second connection matches it, and a changed pin refuses the
/// connection rather than returning it.
#[tokio::test]
async fn open_pinned_checks_the_certificate_a_real_handshake_presents() {
    let port = loopback_server().await;
    let (dir, pin) = scratch("loopback");
    let mut lines = Vec::new();

    let first = open_pinned("127.0.0.1", port, &pin, &mut |l: &str| {
        lines.push(l.to_owned());
    })
    .await
    .expect("first use proceeds");
    drop(first);
    let stored = std::fs::read(&pin).expect("first use wrote the pin");
    assert_eq!(
        pem_to_der(&stored),
        Ok(der("loopback.cert.pem")),
        "the pin is the certificate the server presented"
    );
    assert!(lines[0].contains("FIRST USE"), "{lines:?}");

    open_pinned("127.0.0.1", port, &pin, &mut |l: &str| {
        lines.push(l.to_owned());
    })
    .await
    .expect("the pinned certificate matches");
    assert!(lines[1].contains("matches the pin"), "{lines:?}");

    // Simulate a MITM: pin some other certificate, then connect to this one.
    std::fs::write(&pin, fixture("rotated.cert.pem")).expect("repin");
    let before = std::fs::read(&pin).expect("read pin");
    let refused = open_pinned("127.0.0.1", port, &pin, &mut |_: &str| {}).await;
    let error = refused.expect_err("a changed certificate is refused");
    assert!(error.fatal, "{error}");
    assert_eq!(error.stage, "cert_pin");
    assert_eq!(std::fs::read(&pin).expect("read pin"), before, "no re-pin");
    let _ = std::fs::remove_dir_all(dir);
}
