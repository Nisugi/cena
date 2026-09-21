# M4 mobile compile gate verification

2026-09-21. The binding requirement remains `plan/12` §1a: `cena-platform`,
`cena-protocol`, `cena-model`, `cena-session`, and `cena-behavior` compile for
`aarch64-linux-android` and `aarch64-apple-ios`. Mobile frontends and device
testing are outside this requirement.

## Implemented gate

`.github/workflows/ci.yml` now has separate Android and iOS jobs, in addition
to the existing desktop checks. Both build all five libraries with the committed
lockfile. They do not run tests, start a game connection, package an app, or
require signing credentials. No job permits failure or skips the native
dependencies.

**VERIFIED from source:** the implementation follows the reference port read
first at `../VellumFE-current/.github/workflows/android-check.yml` and
`ios-check.yml`: Android uses `cargo-ndk` with the runner's
`ANDROID_NDK_LATEST_HOME`; iOS uses a macOS runner and its iPhoneOS SDK.
`cena-platform/Cargo.toml` already enables vendored OpenSSL on Linux/Android
and retains native TLS on Apple targets. `cena-session` also brings bundled
SQLite, so checking only the platform/model crates would omit a native build.

The new jobs install target standard libraries with `rustup target add` **inside
the checkout**, so the `rust-toolchain.toml` pin applies. Putting `targets` only
on the stable action could instead install them for its newer default toolchain.
The Android helper is pinned to `cargo-ndk` 4.1.2; its own published source
declares Rust 1.86 as the minimum.

Public infrastructure references checked for this implementation:

- [cargo-ndk configuration and NDK selection](https://github.com/bbqsrc/cargo-ndk)
- [cargo-ndk version and Rust requirement](https://github.com/bbqsrc/cargo-ndk/blob/main/Cargo.toml)
- [Ubuntu 24.04 runner inventory, including NDK and Perl](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md)
- [Rust action installation behavior](https://github.com/dtolnay/rust-toolchain/blob/master/action.yml)

## Local probes and limits

**VERIFIED:** YAML parsing succeeded, and `bash -n` accepted every new mobile
shell step. `git diff --check` passed. These checks establish syntax only.

**VERIFIED latest local environment, after installing the pinned toolchain and
fetching the workspace dependencies:**

- `rustc --version` reported `rustc 1.96.1 (31fca3adb 2026-06-26)`, matching
  `rust-toolchain.toml`.
- `rustup target list --installed` listed only `x86_64-unknown-linux-gnu`.
- `command -v cargo-ndk xcrun clang perl` found only `/usr/bin/perl`.
  `/usr/lib/android-sdk` contained only `platform-tools`; the configured
  `/home/atari/Android/Sdk` directory was absent. No Android NDK was available
  at those inspected locations. This is not an exhaustive filesystem census.

The pinned checks used separate temporary build directories to avoid waiting
on the concurrent desktop build's Cargo lock:

```sh
cargo check --offline --locked \
  --target-dir /tmp/cena-mobile-check.VXnGb3/android \
  -p cena-platform -p cena-protocol -p cena-model \
  -p cena-session -p cena-behavior --target aarch64-linux-android
cargo check --offline --locked \
  --target-dir /tmp/cena-mobile-check.VXnGb3/ios \
  -p cena-platform -p cena-protocol -p cena-model \
  -p cena-session -p cena-behavior --target aarch64-apple-ios
```

**VERIFIED result:** both exited 101 during dependency compilation with
`error[E0463]: can't find crate for core`. Each diagnostic names its missing
target and suggests `rustup target add` for that target. Dependency resolution
now succeeds; the missing target standard libraries are the observed blocker.
Neither probe reached the workspace crates or the native TLS/SQLite builds.

Earlier exploratory probes with the installed stable 1.98.1 stopped at a
missing cached `jiff`; that cache blocker and the initially missing pinned
compiler are resolved. The results above supersede those preliminary probes.

## Native build prerequisite review

**VERIFIED from the locked dependency graph and downloaded crate source:**

- `cargo tree --offline --locked -p cena-platform --target
  aarch64-linux-android -i openssl-src` resolves
  `openssl-src 300.6.1+3.6.3` through `openssl-sys 0.9.117`. The source invokes
  Perl (`openssl-src/src/lib.rs:157`), gets its cross compiler and archiver
  from `cc` (`:439`), and runs make (`:654`). The NDK supplied through
  `cargo-ndk`, plus the Ubuntu runner's Perl/make, addresses these prerequisites.
  A host compiler alone does not provide Android headers, libraries, or tools.
- The same tree query for `aarch64-apple-ios -i openssl-src` prints no
  dependencies. Querying `-i security-framework-sys` resolves version 2.17.0
  through `native-tls 0.2.18` / `security-framework 3.7.0`.
  `security-framework-sys/src/lib.rs:3` declares the Apple Security framework
  link. This is an Apple SDK requirement, not an OpenSSL installation problem.
- `cargo tree --offline --locked -p cena-session --target
  aarch64-linux-android -e features -i libsqlite3-sys` shows `bundled`,
  `bundled_bindings`, and `cc`, without `buildtime_bindgen`. Version 0.38.2's
  `build.rs:137-152` copies pregenerated bindings and compiles `sqlite3.c`
  with `cc`. Both mobile targets therefore need a working target C toolchain;
  they do not need an additional bindgen installation for this feature set.

**INFERRED:** the declared CI runners and NDK setup satisfy the identified
native prerequisites. This source review found no additional concrete missing
dependency in the proposed jobs. The local probes stop too early to establish
that these native builds succeed; only actual mobile CI builds settle that.

## CI evidence and independent split

**VERIFIED 2026-09-21:** both jobs passed on the original combined M4 branch at
`153b25c1796db610b03bd607aba32a24c50296f5`:

- [Android core build](https://github.com/Nisugi/cena/actions/runs/35581947042/job/106276606119)
- [iOS core build](https://github.com/Nisugi/cena/actions/runs/35581947042/job/106276606523)

This branch extracts those mobile jobs independently, based on
`804068238c91a4a4377b3ed7996dac9188ca88b7`. Frontend Node/browser checks stay in
the frontend PR; neither mobile job depends on those crates. The extracted
revision still needs its own CI run. Library builds do not establish mobile app
linking, packaging, runtime behavior, or device support. Existing desktop checks
are unchanged; their failures must not be confused with mobile build failures.
