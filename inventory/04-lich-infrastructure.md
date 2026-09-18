# Lich-5 Infrastructure Inventory

**Scope.** Everything in Lich-5 that is *not* game logic and *not* the script-facing
API: process startup, argument handling, authentication, the proxy/socket layer,
frontend launching, detachable clients, the update mechanism, settings storage,
logging and platform handling. Then a per-concern comparison against VellumFE's
existing Rust implementations.

Files read for this document (Lich-5 v5.21.0, `lib/version.rb:3`):
`lich.rbw`, `lib/constants.rb`, `lib/version.rb`, `lib/init.rb`, `lib/wine.rb`,
`lib/lich.rb`, `lib/games.rb`, `lib/global_defs.rb`, `lib/update.rb`,
`lib/main/` (all 11 files), `lib/common/authentication/` (all 10 files),
`lib/common/frontend*.rb`, `lib/common/frontend/`, `lib/common/process_launcher.rb`,
`lib/common/class_exts/synchronizedsocket.rb`, `lib/common/pipe_io.rb`,
`lib/common/detachable_client_registry.rb`, `lib/common/buffer.rb`,
`lib/common/sharedbuffer.rb`, `lib/common/db_store.rb`, `lib/common/log.rb`,
`lib/common/gui/password_cipher.rb`, `lib/common/credential_scrub.rb`,
`lib/util/opts.rb`, `lib/api/active_sessions.rb`, `lib/internal_api/active_sessions.rb`,
`lib/common/update/`.
VellumFE side (v0.3.0-beta.51): `src/network.rs`, `src/main.rs`, `src/platform.rs`,
`src/session_cache.rs`, `src/webui.rs`, `src/launcher/`, `src/config/paths.rs`,
`src/config/profiles.rs`, `src/core/session_registry.rs`.

Date: 2026-09-17.

---

## 0. Size at a glance

| Area | Files | LOC | Notes |
|---|---|---|---|
| `lib/main/` | 11 | 2,333 | `main.rb` alone is 1,155 |
| `lib/common/authentication/` | 10 | 4,418 | `entry_store.rb` 1,131, `cli_password.rb` 817 |
| `lib/common/frontend/` (catalog) | 8 | 146 | one Ruby-hash file per frontend |
| `lib/common/frontend*.rb` | 5 | 2,170 | `frontend.rb` 928, `frontend_locator.rb` 425 |
| `lib/common/update/` | 11 | 1,818 | plus `lib/update.rb` 346 |
| `lib/common/gui*` (GTK launcher) | 27 + 2 | ~10,050 | `lib/common/gui/*.rb` = 9,300; `gui_login.rb`, `gtk.rb` |
| `lib/internal_api/active_sessions.rb` | 1 | 645 | multi-process session registry |
| `lib/util/` | 6 | 2,314 | `memoryreleaser.rb` 977, `textstripper.rb` 593 |
| `lib/lich.rb` | 1 | 1,265 | SQLite, hosts file, SGE/SAL registry linking |
| `lib/init.rb` | 1 | 593 | ~350 lines of it is raw Win32 Fiddle FFI |
| `lib/games.rb` | 1 | 1,677 | contains the two socket pump threads |
| `lib/global_defs.rb` | 1 | 2,370 | script API + detachable-client fan-out |

---

## 1. Startup and argument handling

### 1.1 Load order

Entry point is `lich.rbw` (174 lines), *not* a `lib/` file.

| Step | Where | What |
|---|---|---|
| 1 | `lich.rbw:10-30` | Pre-scans `ARGV` **before** anything is required, to set the eight path constants (`--home=`, `--temp-dir=`, `--script-dir=`, `--map-dir=`, `--log-dir=`, `--backup-dir=`, `--data-dir=`, `--lib-dir=`, `--active-session-dir=`). This is a raw regex loop, not the option parser. |
| 2 | `lich.rbw:32-37` | `require constants.rb` then `version.rb`. `constants.rb:1-8` defines `LICH_DIR/TEMP_DIR/DATA_DIR/SCRIPT_DIR/LIB_DIR/MAP_DIR/LOG_DIR/BACKUP_DIR` with `||=` so step 1 wins. `version.rb:8-27` hard-exits if Ruby < `REQUIRED_RUBY = '4.0'`. |
| 3 | `lich.rbw:43-44` | `Lich::Main::EarlyExit.dispatch!` — `--help` / `--version` print and exit before any gem or GTK load (`lib/main/early_exit.rb`, 84 lines). |
| 4 | `lich.rbw:46-47` | `Lich::GemCheck.verify!` (`lib/gemcheck.rb`, 575 lines). |
| 5 | `lich.rbw:54-55` | `Lich::Util::GtkCompaction.install!` — must run before `require 'gtk3'`. |
| 6 | `lich.rbw:58-75` | 18 stdlib/gem requires: `base64 digest/md5 digest/sha1 drb/drb json monitor net/http ostruct ox resolv rexml socket stringio terminal-table time yaml zlib`. |
| 7 | `lich.rbw:77-78` | `lich.rb`, then `init.rb`. |
| 8 | `init.rb:29-376` | On Windows only, defines `module Win32` as ~350 lines of Fiddle FFI over `kernel32`/`user32`/`advapi32`/`shell32`/`psapi` (`CreateProcess`, `ShellExecuteEx`, `Reg*`, `EnumProcesses`, `GetTokenInformation`). |
| 9 | `init.rb:386-415` | `require 'sqlite3'`, with a consent-gated self-heal path on `LoadError`. |
| 10 | `init.rb:417-454` | `require 'gtk3'` unless `--no-gtk`/`--no-gui`; sets `HAVE_GTK`. |
| 11 | `init.rb:456-568` | `Dir.chdir(LICH_DIR)`; creates `TEMP_DIR`, `DATA_DIR`, `SCRIPT_DIR`, `SCRIPT_DIR/custom`, `MAP_DIR`, `LOG_DIR`, `BACKUP_DIR`. |
| 12 | `init.rb:492-506` | **`$stderr` is reassigned to `TEMP_DIR/debug-<timestamp>.log`.** This is the entire logging mechanism (see §9). |
| 13 | `init.rb:570-572` | `Lich.init_db`, `Lich.cleanup_debug_logs`. |
| 14 | `lich.rbw:79-141` | ~45 further `require`s: frontend, active_sessions, update, GTK, db_store, class extensions, hooks, settings, script, util, games, gameobj. |
| 15 | `lich.rbw:145-146` | `argv_options.rb` (which self-executes at its line 412), then `main.rb` (which self-starts `@main_thread` at its line 53). |
| 16 | `lich.rbw:163-173` | `Gtk.main` if GTK loaded, else `@main_thread.join`. |

**Porting note.** The load order encodes real ordering constraints (GTK compaction before gtk3; path constants before everything; `$stderr` redirect before the first `Lich.log`). In Rust these become explicit init-phase functions; the fragile part is not the order but the fact that *self-executing files* (`argv_options.rb:412`, `main.rb:53`) do work at require time.

### 1.2 Argument processing pipeline

`Lich::Main::ArgvOptions.process_argv` (`lib/main/argv_options.rb:382-406`) is five ordered steps:

1. `ARGV.delete_if { |arg| arg =~ /launcher\.exe/i }` (`:384`) — strips the Simutronics launcher's own argv0.
2. `ArgNormalization.normalize!(ARGV)` (`:387`) — rewrites `--headless PORT` into `--without-frontend --detachable-client=PORT` (`lib/main/arg_normalization.rb:28-55`). `--headless auto` → port `0`; `--headless HOST:PORT` accepted with HOST ∈ {`tailscale`, `lan`, `any`, an IP, a hostname}. Bare `--headless` raises (`:45`).
3. `Lich::Common::CLI::CLIOrchestration.execute` (`:394`, `lib/common/cli/cli_orchestration.rb`, 352 lines) — early-exit operations: `--link-to-sge`, `--unlink-from-sge`, `--link-to-sal`, `--unlink-from-sal`, `--install`, `--uninstall` (`argv_options.rb:33-58`).
4. `OptionParser.execute` (`:397`).
5. `SideEffects.execute` then `GameConnection.execute` (`:400-403`).

The generic parser is `Lich::Util::Opts.parse(argv, schema)` (`lib/util/opts.rb:32-70`): a schema hash of `{key => {type:, default:, short:, parser:}}` with types `:boolean :string :integer :array`, matching both `--foo value` and `--foo=value`, returning a **frozen `OpenStruct`**. Only 113 lines; trivially replaceable by `clap`.

`GameConnection` (`argv_options.rb:245-380`) hardcodes host/port per game:

| Flag combination | Host | Port |
|---|---|---|
| GemStone (default) | `storm.gs4.game.play.net` | `10024`, or `10624` with `--test` (`:290`) |
| GemStone `--platinum` | `storm.gs4.game.play.net` | `10124` (`:279`) |
| DragonRealms (default) | — | `11024`, or `11624` with `--test` (`:350`) |

`main.rb:382-386` sets `$platinum = true` when `gameport` is `10121` or `10124`.

**Full public CLI surface** is documented in `lib/main/help_text.rb` (270 lines). Grouped: path overrides (9 flags, `:215-225`), frontend selection (6, `:112-117`), game selection (6, `:104-109`), login/session (7, `:94-101`), account management (8, `:146-153`), session inspection (2, `:184-185`), transport (`--pipe`, `--without-frontend`, `--detachable-client`, `--bind-address`, `--frontend=`, `--frontend-command=`, `--game=HOST:PORT`, `:239-247`).

### 1.3 main.rb control flow

`main.rb` builds one `@main_thread` (`:53`) with `abort_on_exception = true` (`:54`) and everything below runs inside it.

| Lines | Phase |
|---|---|
| `:15` | Snapshots `original_argv = ARGV.map(&:dup)` **before** the post-login scrub, because `--reconnect` must `exec` with the real password. |
| `:17-51` | `reconnect_if_wanted` proc: re-`exec`s the process after `--reconnect-delay=N[+M]` seconds (default 60/0, `:27-28`). |
| `:56-60` | Sets `$SEND_CHARACTER='>'`, `$cmd_prefix='<c>'`, `$clean_lich_char` = `,` for Genie else `;`. |
| `:112-193` | `--login` path: CLI authentication via saved entries. |
| `:197-200` | GUI path: `gui_login` if GTK is defined and (`ARGV.empty?` or `--gui`). |
| `:206-209` | `$_SERVERBUFFER_` (max 400) and `$_CLIENTBUFFER_` (max 100) as `LimitedArray`. |
| `:213-224` | `--sal FILE`: reads a `.sal` launch file line-by-line into `@launch_data`. |
| `:226-543` | Launch-data path: parse GAMECODE/GAMEPORT/GAMEHOST/GAME/KEY, resolve frontend, bind listener, spawn frontend, wait 30 s for it to connect. |
| `:564-606` | `--pipe` + `-g HOST:PORT`: stdin/stdout as client, direct game connect, 30 s bounded. |
| `:607-677` | `-g HOST:PORT` without `--pipe`: **hosts-file impersonation mode** (see §4.3). |
| `:689-691` | **Credential scrub** of `@launch_data`, `ARGV`, `@argv_options`. |
| `:695` | `undef :exit!` — removes `exit!` from the top-level object so scripts cannot hard-kill the process. |
| `:697-719` | `--without-frontend`: a thread sends game key + `Frontend::CLIENT_STRING` + two `<c>` with 0.3 s spacing. |
| `:744-884` | `client_thread`: the frontend→game pump. |
| `:887-895`, `:968-976` | Two parallel session-lifecycle registrations (`Lich::InternalAPI::ActiveSessions::Lifecycle` and `Lich::Common::SessionLifecycle`). |
| `:897-964` | detachable client accept loop (see §5). |
| `:991` | `Game.thread.join` — the main blocking point for the whole session. |
| `:993-1147` | Instrumented shutdown: 11 named `shutdown_step` calls, each timed with per-step thresholds (`Vars.save` and `Lich.db.close` at 0.5 s, others 0.75 s, `:998-1001`), a watchdog armed at `:1072` and disarmed at `:1130`. |

**Difficulty.** `main.rb` is the single hardest file to port: 1,155 lines of branching where the connection mode, the frontend identity, the handshake dialect and the shutdown path are all decided inline in one lexical scope, sharing mutable locals (`game_key`, `launcher_cmd`, `custom_launch`, `listener`) across thread boundaries. `game_key = nil` at `:852` is documented as clearing "the only binding". In Rust this must become explicit state machine + owned values.

---

## 2. Authentication

This is the highest-risk area for Cena. There are **two independent providers**
behind one `Authenticator` facade.

### 2.1 EAccess (SGE) — `lib/common/authentication/eaccess.rb`, 396 lines

Transport: TLS over TCP to `eaccess.play.net:7910` (`eaccess.rb:100`, `:145`).

**Certificate handling** — the server cert is self-signed, so there is no chain of trust:
- Pin file is `DATA_DIR/simu.pem` (`:91`).
- If absent, `download_pem` connects with a bare `SSLContext` (no verification), and writes `ssl.peer_cert` to disk (`:100-122`).
- The real connection uses `VERIFY_PEER` with an `X509::Store` containing only that file (`:151-155`).
- `verify_pem` then compares `conn.peer_cert.to_s == File.read(pem)` and **silently re-pins on mismatch** (`:125-142`). The code comments that a legitimate rotation and a MITM are indistinguishable here.
- `CONNECT_TIMEOUT = 5` bounds only the TCP connect (`:36`); the whole exchange is bounded separately by `auth_with_timeout(timeout: 30)` (`:372-392`).
- `EAccess.read(conn)` is a single `conn.sysread(8192)` — **`PACKET_SIZE = 8192`, no framing, no loop** (`:23`, `:353-355`). One read per command.

**The wire protocol, exactly as `auth` (`:190-333`) performs it.** Every command is `conn.puts "<CMD>\n"`, i.e. the line is written with a trailing `\n` and `puts` adds another.

| # | Send | Read / check | Code |
|---|---|---|---|
| 1 | `K\n` | hash key; raises `MALFORMED_K_RESPONSE` if blank | `:204-214` |
| 2 | — | **password obfuscation**: `password[i] = ((password[i] - 32) ^ hashkey[i]) + 32`, byte-wise, zipped against the hash key | `:216-219` |
| 3 | `A\t<account>\t<obfuscated>\n` | must match `/KEY\t(?<key>.*)\t/`; else the last whitespace-delimited token is the error code | `:221-228` |
| 4 | `M\n` | must match `/^M\t/` | `:230-236` |
| 5 | `F\t<game_code>\n` | must match `/NORMAL\|PREMIUM\|TRIAL\|INTERNAL\|FREE/` (or `NEW_TO_GAME` on the generator path). Stored as `Account.subscription` | `:240-254` |
| 6 | `G\t<game_code>\n` | response read and discarded | `:256-257` |
| 7 | `P\t<game_code>\n` | response read and discarded | `:259-260` |
| 8 | `C\n` | character list. Stored as `Account.members` | `:262-267` |
| 9 | — | `resolve_char_code`: strip leading `/^C\t\d+\t\d+\t\d+\t\d+[\t\n]/`, then `scan(/[^\t]+\t[^\t\n]+/)` for code/name pairs, match name **exactly** (`==`, case-sensitive). Raises `CHARACTER_NOT_FOUND` | `:342-350` |
| 10 | `L\t<char_code>\tSTORM\n` | must match `/^L\tOK\t/` — both success and failure are `L\t`-prefixed, e.g. `L\tPROBLEM\t1`. Then strip `L\tOK\t`, split on `\t`, split each on `=`, downcase keys → hash | `:272-295` |

The `L OK` payload yields `key`, `gamehost`, `gameport`, `game`, `gamecode`,
`fullgamename`, `gamefile`. `LaunchResult.normalize` (`launch_result.rb:35-42`)
lowercases keys, requires only `key` (`REQUIRED_KEYS = %w[key]`, `:22`) and freezes.

Character-generator mode sends `NEW_CHARACTER_CODE = "0"` instead of a resolved code (`:27`, `:270`).

A `legacy: true` mode (`:296-328`) instead enumerates every game from the `M`
response with `N\t<code>`, then repeats F/G/P/C per game, returning an Array of
`{game_code, game_name, char_code, char_name}` hashes.

**Diagnostics.** Every step is wrapped in `EAccess.stage(name, probable_cause:)`
(`:78-87`), which stamps `Thread.current[:eaccess_stage]` so `auth_with_timeout`
can report which step hung (`:386`). Stage names: `tcp_connect:pem_bootstrap`,
`tls_handshake:pem_bootstrap`, `tcp_connect:main`, `tls_handshake:main`,
`cert_pin_mismatch`, `k_response`, `a_response`, `m_response`,
`entitlement_response`, `l_response`.
`KNOWN_REJECTION_TOKENS = %w[REJECT NORECORD INVALID PASSWORD]` (`:43`) separates a
real credential rejection from backend divergence.

### 2.2 HTTPS web-login fallback — `lib/common/authentication/web_login.rb`, 576 lines

A completely different protocol against `BASE_HOST = "www.play.net"` port 443
(`web_login.rb:47`, `:204`). It scrapes HTML.

- A private in-memory `CookieJar` class (`:62-98`), keyed by cookie name, absorbing `Set-Cookie` per response.
- `USER_AGENT` is a **hardcoded Chrome 120 string** (`:103`) — the code notes Ruby's default `Ruby/x.y.z` UA is rejected outright, including on the first GET.
- `CONFIRMED_INSTANCES` (`:144-150`) maps game code → `{family, web_game_code, character_list_path, expected_host, expected_port}`:

| Code | family | web code | character list path | expected host | port |
|---|---|---|---|---|---|
| `DR` | dr | DR | `/dr/play/home.asp` | `storm.dr.game.play.net` | 11024 |
| `DRT` | dr | DRT | `/dr/play/playdrt.asp` | `hydra.simutronics.com` | 11624 |
| `DRX` | dr | DRX | `/dr/play/playx.asp` | *unverified* | — |
| `DRF` | dr | DRF | `/dr/play/playf.asp` | *unverified* | — |
| `GS3` | gs4 | GS4 | `/gs4/play/home.asp` | `storm.gs4.game.play.net` | 10024 |
| `GST` | gs4 | GST | `/gs4/play/play_test.asp` | `chimera.simutronics.com` | 10624 |
| `GSF` | gs4 | GSF | `/gs4/play/playf.asp` | `storm.gs4.game.play.net` | 10324 |

Flow (`auth`, `:191-225`):

1. **`login`** (`:280-306`) — `GET /<family>/signin_needed.asp` first, purely to obtain an ASP session cookie (the code notes a cold POST gets a bare 500). Then `POST /includes/common/login/login.asp` form-encoded with fields `return_okay_page`, `return_error_page`, `remember_account: ""`, `remember_password: ""`, `account_name`, `account_password`, `submit: "Login"`. Success is decided **by the redirect `Location` path only** — `/<family>/play/home.asp` or `/playdotnet/account/security_qa.asp` (`SECURITY_QA_PATH`, `:255`); `/<family>/login_error.asp` → `LOGIN_FAILED`.
2. **`resolve_char_code`** (`:323-364`) — `GET instance[:character_list_path]`, then **scrape the HTML** with
   `/id="(W_[A-Za-z0-9_]+)"[^>]*>\s*<label for="\1"><span[^>]*>([^<]+)<\/span>/`
   and match the character name case-insensitively (`:359-361`). A 3xx to `subscription(_to_\w+)?_needed.asp` → `NO_SUBSCRIPTION`; any other non-200 → `UNEXPECTED_CHARACTER_LIST_RESPONSE`.
3. **`select_character`** (`:376-398`) — `POST /includes/common/play/goplay2.asp` with `charID`, `NEWCHARSUB: "TRUE"`, `managesub: 0`, `gameName: <family>`, `instanceID: 0`, `game: <web_game_code>`, `frontend: "web"`.
4. **`follow_redirects`** (`:421-...`) — follows relative `Location` redirects by GET until an absolute URL; `validate_final_url!` (`:483-489`) requires exactly `https://www.play.net/play/home.asp`, port 443, no userinfo.
5. **`extract_connection_info`** (`:516-543`) — parses host/port/key from the final URL's **query string**. For instances with no confirmed host, the returned host must end with one of `TRUSTED_GAME_HOST_SUFFIXES = [".simutronics.com", ".simutronics.net", ".game.play.net"]` (`:158`) or `UNTRUSTED_CONNECTION_HOST`.

The result is normalized to the same shape as EAccess, synthesizing
`game: "STORM"`, `fullgamename: "Wrayth"`, `gamefile: "WRAYTH.EXE"` (`:219-222`).

**This is the single most fragile piece of Lich's infrastructure.** It depends on
a specific HTML `id=`/`<label>`/`<span>` nesting, on specific ASP page paths per
instance, and on a hardcoded browser UA. The file's own comments cite
`docs/web-login-protocol-analysis.md` and call it "the most fragile part of this
module" (`:311`).

### 2.3 Provider selection and retry — `authenticator.rb`, 269 lines

`Authenticator.authenticate(auth_provider: :eaccess)` (`:89`) tries EAccess, and on
failure falls back to web login when `character` and `game_code` are present and
neither `legacy` nor `generator` is set (`web_fallback_supported?`, `:190`).
`auth_provider: :web` forces web login (`:103-112`).

- `FATAL_ERROR_CODES = %w[REJECT NORECORD INVALID PASSWORD CHARACTER_NOT_FOUND GENERATOR_NOT_AVAILABLE LOGIN_FAILED NO_SUBSCRIPTION]` (`:29`) — never retried.
- `AUTH_RETRY_BASE_DELAY = 5`, doubling: 5 s, 10 s, 20 s (`:18`).
- `unreachable_error?` (`:56`) + `fast_fail_unreachable` — skips retrying an unreachable endpoint when a fallback provider exists (`:238-241`).

### 2.4 Saved logins — `entry_store.rb` (1,131 lines) + `password_cipher.rb` (152 lines)

Storage is `DATA_DIR/entry.yaml`, written with mode `0o600` (`entry_store.rb:1049-1052`),
migrated from a legacy `DATA_DIR/entry.dat` (`:141-203`). Entry shape is built by
`LaunchData.create_entry` (`launch_data.rb:88-99`): `char_name, game_code,
game_name, user_id, password, frontend, custom_launch, custom_launch_dir`.
Favorites, ordering and autosort are layered on top (`:496-700`).

Password encryption (`password_cipher.rb`):

| Property | Value | Line |
|---|---|---|
| Algorithm | `AES-256-CBC` | `:26` |
| KDF | `PBKDF2-HMAC-SHA256` | `:140-146` |
| Iterations | `10_000` | `:29` |
| Key length | 32 bytes | `KEY_LENGTH` |
| Salt | `"lich5-password-encryption-#{mode}"` — **a fixed, non-secret, non-per-account string** | `:137` |
| Passphrase (`:standard` mode) | `account_name.upcase` — i.e. **the key is derived from a public identifier** | `:129-130` |
| Passphrase (`:enhanced` mode) | a user master password | `:131-132` |
| Wire format | `Base64.strict_encode64(iv + ciphertext)`, random IV per encrypt | `:54`, `:60` |

`:standard` mode is obfuscation, not encryption: anyone with `entry.yaml` and the
account name can derive the key. There is also a Windows Credential Manager path
(`lib/common/gui/windows_credential_manager.rb`, 241 lines).

### 2.5 Credential scrubbing — `credential_scrub.rb`

Runs at `main.rb:689-691`, once every connection path has consumed the credentials.

- `LAUNCH_DATA_SECRET = /\AKEY=/i` → replaced with `KEY=[scrubbed]` (`:30`, `:52`).
- `ARGV_SECRET = /\A(--(?:password|master-password)=)(.+)\z/i` — flag prefix preserved so later `ARGV.include?` checks still work (`:34`, `:72`). `--account=` deliberately excluded (`:32-33`).
- `OPTION_SECRET_KEYS = [:password]` (`:37`).
- Values are overwritten **in place**, not reassigned, because the login GUI and the option pipeline hold references to the same Array/Hash/String objects (`:15-19`).
- `shred_file` is used for the temporary `.sal` file, with an `at_exit` backstop (`main.rb:488`, `:529`, `:542`).
- The module's own doc is explicit that this closes script-readable and log-readable paths only, not process memory (`:21-24`).

---

## 3. Networking and proxy

### 3.1 Topology

```
frontend (TCP)            Lich process                     game server (TCP)
   |                                                              |
   |  $_CLIENT_ ──► client_thread ──► dispatch_client_input ──► Game._puts
   |                (main.rb:744)      (global_defs:2231)          |
   |                                                              |
   |◄── SynchronizedSocket writer thread ◄── parser thread ◄── reader thread
        (one per socket)                     (games.rb:891)    (games.rb:791)
   |
   +── N detachable clients, same fan-out via detachable_clients_respond
```

### 3.2 The two pump threads — `lib/games.rb`

Both are created by `Game.open` (`games.rb:487-492` calls `start_wrap_thread` and `start_main_thread`).

**Socket reader** — `start_socket_reader_thread` (`:790-888`), thread named
`'game socket reader'`, `priority = 5` (`:886-887`):
- `read_server_string` (`:926-930`) does `IO.select([@socket], nil, nil, 100)` then `@socket.gets`. The explicit select is because `SO_RCVTIMEO` is not reliably surfaced through `TCPSocket#gets` on all platforms (`:918-922`).
- `READ_TIMEOUT_SECONDS = 100`, `MAX_CONSECUTIVE_READ_TIMEOUTS = 3` (`:375`, `:378`) — so 300 s of silence kills the link.
- Runs `Lich::Common::SocketReadHook.run(server_string, received_at:, monotonic_received_at:)` (`:819-823`) on the reader thread, then enqueues onto `@server_queue`.

**Parser / server processor** — `start_server_processor_thread` (`:890-916`),
thread named `'game parser'`, `priority = 4`:
- Pops from `@server_queue` (a bounded queue, `SERVER_QUEUE_CAPACITY = 4_096`, `:385`; overflow raises `ServerQueueOverflow`, `:372`).
- Sets `Thread.current.thread_variable_set(:lich_game_ingress_time, …)` so `Game.current_ingress_time` (`:395-400`) can attribute provenance — and only for this exact thread (`:396`).
- `process_server_string` (`:938-984`) does, in order: GSL prefix detection (`\034GSw`), lazy `GameLoader.load!`, game-instance selection, `clean_serverstring`, push to `$_SERVERBUFFER_`, autostart handling, XML parse, `Inventory.observe`, then `process_downstream_hooks`.

Separating read from parse is deliberate: the socket keeps draining while the
parser works. Cena needs the same split.

### 3.3 `SynchronizedSocket` — `lib/common/class_exts/synchronizedsocket.rb`, 630 lines

Wraps any delegate socket. This is a surprisingly deep piece of engineering and
the most direct thing Cena can learn from.

| Constant | Value | Line |
|---|---|---|
| `FATAL_WRITE_ERRORS` | `ECONNRESET, EPIPE, ECONNABORTED, ENOTCONN, IOError` | `:38-44` |
| `DEFAULT_WRITE_QUEUE_CAPACITY` | 4,096 | `:47` |
| `OVERFLOW_KEEP_PROMPT_GROUPS` | 10 | `:52` |
| `ROLES` | `%i[primary detachable]` | `:65` |
| `PROMPT_TAG` | `/<prompt\b/i` | `:57` |

Behaviour:
- **Every write is asynchronous.** `puts`/`write` enqueue onto a `SizedQueue`; one writer thread per socket drains it (`:189-217`). A slow frontend cannot block the game parser.
- **Main-stream deferral**: `puts_main_stream` holds output back while a frontend stream is open, releasing it after the matching `popStream` or a `<prompt>` (`:107-127`, `:245-260`). `note_stream_xml!` (`:266-281`) maintains a `@stream_stack` by scanning tags: `<pushStream>` pushes, `<popStream>` pops by id, `<prompt>` clears.
- **Overflow compaction** (`:320-343`): rather than dying when the queue fills, it drops the oldest *prompt-delimited groups* — safe because a prompt balances the stream stack — replays the last dropped prompt as a resync, and injects a plain-text `--- Lich: frontend fell behind; dropped N lines of display output ---` notice.
- **Liveness is one-way** (`:93-95`): any fatal write error sets `@alive = false`, closes the delegate (which unblocks blocked readers — the fix for "issue #594" zombie sockets, `:478-485`), and for `role: :primary` records the failure on `ShutdownCoordinator`.
- Reads delegate via `method_missing` (`:155-157`) so read-side errors propagate normally.

### 3.4 Listener and binding

- Frontend-launch path: `TCPServer.new(@argv_options[:bind_address] || '127.0.0.1', nil)` — **port 0, OS-assigned** (`main.rb:418`), read back via `listener.local_address.ip_port` (`:424`). One `accept`, wrapped in `SynchronizedSocket`, in a thread (`:496-499`), with a **30-second timeout implemented as `300.times { sleep 0.1 }`** (`:519`).
- Hosts-impersonation path and the detachable listener use `Lich::Common::ReusableTCPServer.create(host, port)` (`main.rb:616`, `:904`) for `SO_REUSEADDR` semantics.
- `--bind-address` wildcards (`0.0.0.0`, `::`) are handled specially everywhere: the listener may bind a wildcard but the **frontend is always told loopback**, since a client cannot connect to `0.0.0.0` (`main.rb:449-453`, `:471-477`).
- `Socket.do_not_reverse_lookup = true` (`main.rb:211`).

### 3.5 `--pipe` mode — `lib/common/pipe_io.rb`, 62 lines

`PipeIO` (`:17-60`) presents `$stdin`/`$stdout` with a socket-shaped interface
(`gets`, `write`, `puts`, `closed?`, `close`, `sync=`), wrapped in a
`SynchronizedSocket` exactly like a real socket (`main.rb:399`, `:570`).
`close` deliberately does **not** close the underlying fds (`:50`). EOF on stdin
marks the client dead and triggers normal shutdown (`main.rb:394-397`).
Paired with `-g HOST:PORT` it bypasses SGE entirely: stdin supplies the login key
and version string. **This is the cleanest existing model for Cena's in-process
frontend**: the same code path, with the transport swapped.

### 3.6 Buffers

| Class | File | LOC | Shape |
|---|---|---|---|
| `Buffer` (module, global) | `common/buffer.rb` | 176 | Class-variable ring, `@@max_size = 3000` (`:21`). Stream bitmask constants `DOWNSTREAM_STRIPPED=1, DOWNSTREAM_RAW=2, DOWNSTREAM_MOD=4, UPSTREAM=8, UPSTREAM_MOD=16, SCRIPT_OUTPUT=32` (`:10-15`). |
| `SharedBuffer` (instantiable) | `common/sharedbuffer.rb` | 131 | Same design, per-instance, default `max_size = 500` (`:17`). |
| `$_SERVERBUFFER_` / `$_CLIENTBUFFER_` | `main.rb:206-209` | — | `LimitedArray`, 400 / 100 entries. |

Both buffer classes index readers by `Thread.current.object_id` and previously
leaked one entry per dead thread; both now sweep via a 60-second `Throttle`
(`buffer.rb:22-26`, `sharedbuffer.rb:18-20`). Readers block by polling
`sleep 0.05` until their index advances (`sharedbuffer.rb:31`).

**Porting note.** This is a per-thread cursor over a shared ring — in Rust the
natural equivalent is a broadcast channel or an `Arc<RwLock<VecDeque>>` with
per-subscriber cursors. The `Thread#object_id` keying and the 50 ms poll are
Ruby-isms that should not survive the port.

### 3.7 Socket hooks

| Hook | File | LOC | Point of application |
|---|---|---|---|
| `SocketReadHook` | `common/socket_read_hook.rb` | 114 | On the reader thread, pre-queue (`games.rb:819`) |
| `DownstreamHook` | `common/downstreamhook.rb` | 62 | On the parser thread, post-parse (`games.rb:983`) |
| `UpstreamHook` | `common/upstreamhook.rb` | 62 | On client input |
| `HookRegistry` | `common/hook_registry.rb` | 189 | Shared registration/ordering |

`main.rb:793-835` registers two engine-level hooks by hand at startup: an
`inventory_boxes_off` downstream filter that strips `<container>`/`<inv>` tags,
and an `inventory_boxes_toggle` upstream filter that intercepts
`_flag Display Inventory Boxes 0|1` and `set inventory on|off`.

---

## 4. Frontend launching

### 4.1 The frontend registry

`Lich::Common::Frontend` (`lib/common/frontend.rb`, 928 lines) is a mutex-guarded
registry (`registry_synchronize`, `:109`) of frontend definitions, each with an
`id`, a `capabilities` array and a free-form `metadata` hash.

The **built-in catalog is loaded by `module_eval`-ing one Ruby file per frontend**
from `lib/common/frontend/` (`:430-451`); each file must return a Hash with only
the keys `id`, `capabilities`, `metadata` or registration is refused
(`validate_built_in_definition!`, `:458-469`).

| File | LOC | id | Capabilities | `launcher_adapter` |
|---|---|---|---|---|
`BUILT_IN_DEFINITION_FILES` (`frontend.rb:21-30`) is an `[id, filename]` list of
exactly eight entries. Note the first: **the id `stormfront` is defined by
`wrayth.rb`** — the modern client renamed, with the old id retained.

| id | File | LOC | Capabilities | `launcher_adapter` |
|---|---|---|---|---|
| stormfront | `wrayth.rb` | 20 | — | `:simutronics` |
| profanity | `profanity.rb` | 10 | — | — |
| genie | `genie.rb` | 6 | — | — |
| frostbite | `frostbite.rb` | 6 | — | — |
| suks | `suks.rb` | 9 | — | — |
| wizard | `wizard.rb` | 19 | `gsl` | `:simutronics` |
| avalon | `avalon.rb` | 18 | — | `:avalon` |
| saga | `saga.rb` | 58 | `xml streams mono room_window sentinel` | `:environment` |

Capability queries drive protocol behaviour, not just display: `supports_gsl?`,
`supports_xml?`, `supports_streams?`, `supports_mono?`, `supports_room_window?`,
`supports_sentinel?` (`:531-551`). `main.rb:750-777` branches the entire login
handshake on `Frontend.supports_gsl?` vs `frostbite` vs default.

`CLIENT_STRING = "/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML"` (`:515`) is
what Lich announces itself as to the game server.

User-supplied frontends go through `replace_user_configuration!` (`:182-264`)
with a persisted store in `lib/common/frontend_settings.rb` (397 lines), loaded at
`lich.rbw:82` before the launcher. Discovery of the actual executable is
`lib/common/frontend_locator.rb` (425 lines) — executable names, Windows registry
keys (e.g. `SOFTWARE\Simutronics\WIZ32`, `SOFTWARE\WOW6432Node\Simutronics\WIZ32`,
`wizard.rb:13-16`), macOS bundle ids, and per-platform path globs with env-var
expansion (`saga.rb:44-57`).

### 4.2 Launch mechanics — `frontend_launcher.rb` (379 lines) + `process_launcher.rb` (37 lines)

Four launcher adapters (`:66-76`):

| Adapter | Behaviour |
|---|---|
| `:simutronics` | Uses the discovered Simutronics launcher; raises `UnavailableError` if not found (`:72`) |
| `:custom` | Backed by a persisted user command string (`:39`) |
| `:avalon` | macOS: `/usr/bin/open -n -a <bundle> "%1"` (`:374`); arg separator is ` --args ` (`:102`) |
| `:environment` | Builds a `spawn_plan` of `{command, argv, environment}` — the plan's *environment values* carry the connection tokens (`:143-181`) |

Connection tokens are `%host%`, `%port%`, `%key%`
(`CONNECTION_PLACEHOLDER_PATTERN`, `:25`; substitution at `:134-136`), plus `%1`
for a `.sal` file path substituted in `main.rb:490-492`.

Saga is the only `:environment` frontend: `SAGA_LICH_MODE=1`,
`SAGA_LICH_HOST=%host%`, `SAGA_LICH_PORT=%port%`, `SAGA_LICH_KEY=%key%`
(`saga.rb:3-8`), with per-platform plans (`open -n -b com.auchand.saga` on macOS,
the resolved executable on Windows/Linux).

`ProcessLauncher.call(environment, argv)` (`process_launcher.rb:21-33`) always uses
`Process.spawn` with an explicit argv array, and for a one-element argv uses the
`[executable, argv0]` form so paths with spaces stay atomic — deliberately avoiding
Ruby's shell-interpreted single-string form. Note `main.rb:511` still falls back to
bare `spawn(launcher_cmd)` (shell form) for string commands.

### 4.3 The three redirection strategies

This is the crux of "how Lich makes a third-party FE talk to it". There are three
distinct mechanisms, all still live:

**(a) `.sal` file rewrite** — `main.rb:478-492`. Lich takes the launch data it got
from EAccess, rewrites `GAMEPORT=` to its own ephemeral port and `GAMEHOST=` to
loopback, writes it to `TEMP_DIR/lich<rand(10000)>.sal`, and passes that path to
the frontend as `%1`. The frontend connects to Lich thinking it is the game
server. The file is shredded on both the connected and the timeout path, with an
`at_exit` backstop because it contains the live eaccess key (`:485-488`).

**(b) Command-line injection** — `main.rb:277`, `:282`, `:290`, `:295`. For Wizard
and Stormfront, Lich constructs the argv directly:
`Wizard.Exe /G<GS|DR>/H127.0.0.1 /P%port% /K%key%` and
`Stormfront /G<GS|DR>/Hlocalhost/P%port%/K%key%`. Under Wine the same with
`#{Wine::BIN}` prefixed and `Shellwords.escape` applied.

**(c) Hosts-file impersonation** — `lib/lich.rb:601-683`, driven by
`main.rb:607-651`. Used when the user runs a frontend Lich did not launch.
- `find_hosts_file` (`:606-641`) probes, in order: the Windows registry `DataBasePath`, `%windir%\system32\drivers\etc\hosts`, every drive letter, `/etc/hosts` (Linux), `/private/etc/hosts` (macOS).
- `modify_hosts(game_host)` (`:643-663`) backs the file up to `<hosts>.bak`, appends `\r\n127.0.0.1\t\t<game_host>`, and registers `at_exit { Lich.restore_hosts }`.
- Lich then binds the **real game port** (10024 etc.) on loopback, accepts the frontend's connection, and immediately restores the hosts file (`main.rb:645-651`) before connecting outward to the real host.
- Timeout is 120 s (`main.rb:636-643`).
- Requires administrator/root. `--hosts-dir=` and `--hosts-file=` override the probe (`help_text.rb:223-224`).

**(d) Registry hijack (SGE/SAL)** — `lib/lich.rb:364-600`. Not per-session; a
persistent install step.
- `link_to_sge` (`:364-430`): rewrites `HKLM\Software\Simutronics\Launcher\Directory` to point at Lich, preserving the original under a new `RealDirectory` value. Under Wine, the same via `Wine.registry_puts` (`:410-426`).
- `link_to_sal` (`:482-549`): rewrites `HKLM\Software\Classes\Simutronics.Autolaunch\Shell\Open\command` so `.sal` files launched from the browser open Lich instead.
- Both self-elevate via `ShellExecuteEx(lpVerb: 'runas')` when not admin (`:393-405`).

### 4.4 Session descriptor files

`Frontend.create_session_file(name, host, port, display_session:)`
(`frontend.rb:600-610`) writes `<tmp_session_dir>/<Name>.session` containing a
host/port descriptor; `cleanup_session_file` (`:615`) removes it. Used both for
the launched-frontend listener (`main.rb:500`) and the detachable listener
(`main.rb:919`). This is the pre-`ActiveSessions` discovery mechanism and is
still written alongside it.

`Frontend.pid` / `set_from_client` / `detect_pid` / `refocus` (`:624-918`) do
OS-level window refocusing — WMI parent-pid walking on Windows (`:755-817`),
`/proc` walking on Linux (`:818`), AppleScript on macOS (`:883`).

---

## 5. Detachable clients — Lich's existing multi-client story

This is the most directly relevant section for Cena's multi-session goal.

### 5.1 Model

One Lich process owns one game connection. It may *additionally* run a persistent
TCP listener that any number of frontends can attach to and detach from, at any
time, without ending the session. Enabled by `--detachable-client=PORT|auto|HOST:PORT`
or the `--headless` alias.

### 5.2 The listener loop — `main.rb:897-964`

A single thread, created only if `@argv_options[:detachable_client_port]` is set:

1. Creates a `ReusableTCPServer` on `(detachable_client_host, detachable_client_port)` with `backlog: 8` (`:904-908`).
2. Publishes `$_DETACHABLE_LISTENER_ = {host:, port:}` from `local_address` — so `port: 0` resolves to the real assigned port (`:909-912`).
3. Writes a session file named after the `--login` character (`:913-922`).
4. Calls `detachable_listener_connected(...)` → `ActiveSessions::Lifecycle.update_listener` (`global_defs.rb:2081-2091`).
5. Prints a `DetachableClientNotice.listening(...)` line to the real stdout (`:929-931`).
6. `accept` → wrap in `SynchronizedSocket.new(socket, role: :detachable)` → `detachable_client_register` → spawn a `handle_detachable_client` thread per client (`:934-939`).
7. **The loop self-heals**: on any accept error it closes the server, clears the listener registration, sleeps 5 s and rebinds (`:940-948`).

### 5.3 Registry and primary-client election — `detachable_client_registry.rb`, 58 lines

A mutex-guarded array. Election is positional and implicit:

| Method | Semantics | Line |
|---|---|---|
| `register(client)` | appends unless present; returns whether the list *became* non-empty | `:13-17` |
| `unregister(client)` | returns `[removed?, now_empty?]` | `:23-26` |
| `primary` | **`@clients.first`** — the oldest still-attached client | `:33-35` |
| `primary?(client)` | `@clients.first.equal?(client)` | `:37-39` |
| `snapshot` / `count` / `empty?` / `remove_all` | — | `:30-53` |

There is no negotiation, no priority and no re-election event: the primary is
simply whoever attached first and has not left.

The primary has exactly one privilege in the current code: only it may set the
frontend PID via `SET_FRONTEND_PID <pid>`, which drives OS window refocus
(`global_defs.rb:2184-2187`).

### 5.4 Globals and fan-out — `global_defs.rb:2057-2229`

```ruby
$_DETACHABLE_CLIENT_REGISTRY_   # the registry object       (:2060)
$_DETACHABLE_CLIENTS_           # cached snapshot Array     (:2061)
$_DETACHABLE_CLIENT_            # cached primary            (:2062)
$_DETACHABLE_LISTENER_          # {host:, port:} or nil     (main.rb:909)
```

`sync_detachable_client_globals` (`:2064-2067`) re-derives the latter two from the
registry after every register/unregister. The two cached globals exist purely for
scripts that inspect them directly (`:2057-2059`).

**Output fan-out.** `respond` and `_respond` (`:1749-1797`) each end with the same
two lines: `$_CLIENT_.puts_main_stream(str) if $_CLIENT_&.alive?` then
`detachable_clients_respond(str)` (`:1774-1775`, `:1792-1793`).
`detachable_clients_respond` (`:2107-2124`) iterates a snapshot, drops dead
clients, and **wraps each client's write in its own rescue** so one failing socket
cannot starve the others (`:2114-2121`).

Note: this is the *script output* path. Game output reaches detachable clients
through the normal downstream hook / client-write path, not through this function.

**Attach-time state replay.** `detachable_client_send_init` (`:2141-2163`) waits up
to 10 s (`100.times { sleep 0.1 }`) for `DetachableClientInit.ready?` — a
per-game module loaded by `GameLoader` — then pushes `DetachableClientInit.init_string`
to the newly attached client. This is how a frontend attaching mid-session gets
the state it missed. Skipped for `--genie` and `--saga` (`:2178`). Saga instead
gets a player-id tag push (`:2165-2175`).

**Input path.** `handle_detachable_client` (`:2177-2229`) reads lines, intercepts
`SET_FRONTEND_PID`, routes exit commands through the shared
`UserExitDispatch.dispatch_detachable_client`, then prefixes `$cmd_prefix` and
calls the same `dispatch_client_input` the primary frontend uses (`:2196-2199`).
On disconnect it prints a `DetachableClientNotice.disconnected` line including the
remaining attached count.

### 5.5 What Cena would need to change

| Lich behaviour | Cena implication |
|---|---|
| Primary = first-attached, no re-election | Needs an explicit election/ownership model if sessions can be handed between devices |
| Attach replay is a single opaque `init_string` per game | Cena wants structured, versioned state replay, not a blob |
| Fan-out is fire-and-forget per client with per-client rescue | Sound model; keep it. In Rust: per-client channel + task, drop on error |
| One game connection per process | Cena's multi-session goal breaks this assumption; the detachable design is per-*process*, not per-*session* |
| `$_DETACHABLE_*` globals | Script-visible state that Lua bindings must replicate or deliberately drop |

---

## 6. Multi-process session registry — `lib/internal_api/active_sessions.rb`, 645 lines

Distinct from detachable clients: this tracks *other Lich processes* on the machine.

- Coordination dir is `ACTIVE_SESSION_DIR` if set, else a default (`:476-477`); `--active-session-dir=PATH` makes all characters coordinate through one directory (`help_text.rb:194`).
- Discovery file: `lich-active-sessions.json` (`DISCOVERY_FILENAME`, `:57`).
- **Ownership is a filesystem lock, not a fixed port** (`:26-31`). One process acquires the lock, binds an **ephemeral** port (`EPHEMERAL_PORT`, `:47-48`), and publishes `{owner_pid, auth_token, port}` into the discovery file (`write_discovery`, `:519-523`). Peers read the file and connect as clients (`service_client`, `:289-300`). The design note is explicit that a fixed well-known port could be squatted by a stuck process (`:31`).
- Requests are authenticated by the published `auth_token`.
- Public surface is read-only: `Lich::API.active_session_snapshot`, `.active_sessions`, `.active_session_service_info` (`lib/api/active_sessions.rb`, 44 lines). Registration and transport stay internal (`:4-8`).
- CLI: `--active-sessions`, `--session-info NAME` (`help_text.rb:184-185`).
- Persisted mirror in SQLite: `session_summary_state` table with `pid, session_name, role, state, frontend, game_code, hidden, started_at, last_heartbeat_at, os_seen_at, os_seen, os_name, last_utilization_at, metadata_json` (`lich.rb:211`), indexed on `session_name` and `last_heartbeat_at` (`:212-213`).

There are **two** parallel lifecycle registrations in `main.rb`:
`Lich::InternalAPI::ActiveSessions::Lifecycle` (`:887-895`) and
`Lich::Common::SessionLifecycle` (`:968-976`), each resolving session name and role
independently. UNVERIFIED: whether these are a migration in progress or
deliberately distinct; I would need to read `lib/common/session_lifecycle.rb`.

---

## 7. Update mechanism

`lib/update.rb` (346 lines) orchestrates; `lib/common/update/` (11 files, 1,818 LOC)
does the work.

| Component | File | LOC | Role |
|---|---|---|---|
| `GithubClient` | `github_client.rb` | 96 | `Net::HTTP` with `VERIFY_PEER`, in-memory TTL cache, optional token from `DATA_DIR/githubtoken.txt` |
| `ChannelResolver` | `channel_resolver.rb` | 158 | Resolves `:stable` / `:beta` to a git tag or branch by semver comparison against the latest non-prerelease release |
| `ReleaseInstaller` | `release_installer.rb` | 388 | Installs a tagged release |
| `BranchInstaller` | `branch_installer.rb` | 137 | Installs from an arbitrary branch |
| `SnapshotManager` | `snapshot_manager.rb` | 127 | Pre-update snapshot for rollback |
| `FileUpdater` / `FileWriter` | | 239 / 49 | File-level apply |
| `ScriptSync` | `script_sync.rb` | 161 | Syncs community script repos |
| `TrackedScripts` | `tracked_scripts.rb` | 220 | Which scripts are managed |
| `CustomRepos` | `custom_repos.rb` | 153 | User-added script repositories |
| `StatusReporter` | `status_reporter.rb` | 90 | Output |

Core repo is `GITHUB_REPO = 'elanthia-online/lich-5'` (`update.rb:39`).
`SCRIPT_REPOS` (`:42-73`) hardcodes three entries pointing at
`elanthia-online/dr-scripts` (branch `main`) and `elanthia-online/scripts`
(branch `master`), each with an `api_url` (git trees API, `?recursive=1`) and a
`raw_base_url` (`raw.githubusercontent.com`).

Branch tracking is persisted by **generating a Ruby file** that defines
`LICH_BRANCH` and `LICH_BRANCH_REPO` constants (`update.rb:256-275`), read back at
`init.rb:508-509`.

Auto-update fires on login: `Game.handle_autostart` (`games.rb:986-993`) compares
`LICH_VERSION` to `Lich.core_updated_with_lich_version` and calls
`Lich::Util::Update.update_core_data_and_scripts`, then spawns a background thread
that waits up to 10 s for `XMLData.name` before `sync_all_repos` — explicitly
because HTTP on the game thread would block XML parsing (`:995-1010`).

**Cena note.** A self-updating Ruby-source installer has no equivalent in a single
compiled binary. Cena's update problem splits into (a) binary self-update
(platform stores on mobile; something like `self_update`/`cargo-dist` on desktop)
and (b) Lua script-corpus sync, which *is* close to `ScriptSync` and can reuse the
same GitHub trees-API + raw-download shape.

---

## 8. Settings storage and SQLite

Single database: `DATA_DIR/lich.db3`, opened once into `@@lich_db`
(`lich.rb:198-200`), `busy_timeout` 5,000 ms (`DEFAULT_SQLITE_BUSY_TIMEOUT_MS`, `:3`).

`Lich.init_db` (`:202-241`) creates:

| Table | Key | Payload |
|---|---|---|
| `script_setting` | `(script, name)` | `value BLOB` |
| `script_auto_settings` | `(script, scope)` | `hash BLOB` — the Settings/CharSettings store |
| `lich_settings` | `(name)` | `value TEXT` |
| `uservars` | `(scope)` | `hash BLOB` |
| `session_summary_state` | `pid` | 13 columns, see §6 |
| `trusted_scripts` | `name` | script trust list |
| `simu_game_entry` | `(character, game_code)` | `data BLOB` |
| `enable_inventory_boxes` | `player_id` | presence flag |

`DB_Store` (`common/db_store.rb`, 78 lines) is the accessor. Default scope is
`"#{XMLData.game}:#{XMLData.name}"` (`:13`, `:22`) — i.e. per-game, per-character.
**Values are stored as `Marshal.dump` blobs** (`:34`, `:40`, `:44`) inside
`SQLite3::Blob`, serialized under `Lich.db_mutex` with a `SQLite3::BusyException`
retry loop (`:47-51`).

> **This is a hard blocker for Cena.** `Marshal` is a Ruby-object serialization
> format. Every existing character's settings and uservars are opaque Ruby
> marshal streams. Cena must either implement a Marshal reader for the subset of
> types scripts actually store (Hash/Array/String/Integer/Float/true/false/nil/
> Symbol), or ship a one-time Ruby-side export tool. Recommend the latter, plus
> a native format (e.g. MessagePack or JSON) going forward.

Other stores: `Settings`/`GameSettings`/`CharSettings` (`common/settings.rb`, 622
lines + `common/settings/`), `Vars`/`UserVars` (`common/vars.rb`, `common/uservars.rb`),
`SessionVars` (`lib/sessionvars.rb`, 35 lines, in-memory only).

Maintenance: `db_vacuum_if_due!(months: 6)` runs at `Game.start_wrap_thread`
(`games.rb:527`), guarded by an **advisory OS file lock** at
`DATA_DIR/lich.db3.maint.lock` so only one Lich instance vacuums (`lich.rb:27-30`,
`:73`).

---

## 9. Logging

There is no logging framework.

- `Lich.log(msg)` is three lines: `$stderr.puts "#{Time.now.strftime("%Y-%m-%d %H:%M:%S")}: #{msg}"` (`lich.rb:249-251`).
- `$stderr` was reassigned at startup to `TEMP_DIR/debug-<YYYY-MM-DD-HH-MM-SS-mmm>.log`, with `sync = true` (`init.rb:493-506`). One file per process launch.
- `Lich.cleanup_debug_logs(TEMP_DIR)` prunes old files (`init.rb:572`).
- There are no levels; severity is a string prefix by convention (`"info: "`, `"warn: "`, `"error: "`, `"warning: "` — all four appear, inconsistently).
- `Lich::Common::Log` (`common/log.rb`, 109 lines) is a *separate*, script-facing contextual logger with `on(filter)`/`off`/`on?`, persisting `log_enabled` and `log_filter` into the `lich_settings` table (`:11-35`).
- `Lich::Common::ShutdownLog` (`common/shutdown_log.rb`) adds `info`/`warning`/`error`/`debug` plus a deferred `flush_user_exit_summary!` used across `main.rb:1074-1145`.
- `Lich.deprecated` (`lich.rb:253-259`) keeps an in-memory de-duplicated deprecation list.

**Cena note.** Trivially replaced by `tracing` + `tracing-subscriber`, which
VellumFE already uses. The only thing worth preserving is the per-launch file
naming and the pruning.

---

## 10. Platform and Wine handling

**Windows.** `init.rb:29-376` defines `module Win32` over Fiddle: `Kernel32`
(`CreateProcess`, `GetExitCodeProcess`, `GetModuleFileName`, `GetVersionEx`,
`GetLastError`, `EnumProcesses`), `User32` (`MessageBox`), `Advapi32`
(`RegOpenKeyEx`/`RegQueryValueEx`/`RegSetValueEx`/`RegDeleteValue`/`RegCloseKey`,
`OpenProcessToken`, `GetTokenInformation`), `Shell32`
(`ShellExecute`, `ShellExecuteEx`), with a `Psapi` fallback for `EnumProcesses`
(`:328-343`). Helpers: `Win32.admin?` (token elevation, `:353-362`),
`Win32.isXP?` (`:345`), `Win32.AdminShellExecute` (re-launches self elevated with a
Marshal-packed, base64'd argument, `:364-374` — and `init.rb:378-382` is the
receiving side, `ARGV[0] == 'shellexecute'`). There is a `# TODO: remove as part
of chore/Remove unnecessary Win32 calls` comment at `:26`, noting it is
"Temporarily reinstated for DR".

**Wine.** `lib/wine.rb`, 138 lines.
- `find_wine_binary` (`:11-25`) walks `PATH` in pure Ruby (no `which`, no backticks) looking for `wine`/`wine.exe`.
- Overridable by `--wine=PATH`; disabled by `--no-wine` or `--without-frontend` (`:29-35`).
- Prefix resolution order: `--wine-prefix=`, `$WINEPREFIX`, `$HOME/.wine` (`:38-45`).
- `module Wine` is only defined if both binary and prefix exist as real file/directory (`:48`).
- `Wine.registry_gets(key)` (`:60-94`) **parses `<prefix>/system.reg` as text** — no `wine regedit` invocation. Only `HKEY_LOCAL_MACHINE` is supported; `HKEY_CURRENT_USER` returns `false` (`:88`).
- `Wine.registry_puts(key, value)` (`:105-135`) writes a temporary `REGEDIT4` `.reg` file into `TEMP_DIR` and runs `system(BIN, 'regedit', filename)` with argv-style invocation, then deletes the file in an `ensure`.

**Platform dispatch.** `Frontend.platform_key` returns `:windows`/`:darwin`/`:linux`
(`frontend.rb:363-376`); `Frontend.windows_platform?` is `platform_key == :windows`
(`:385`). A narrower `native_windows_runtime?` (`:35`) gates the direct Fiddle
calls, deliberately distinct from `windows_platform?` (`:43`).

**Mobile: none.** There is no Android or iOS story anywhere in Lich-5.

---

## 11. GUI launcher and WebUI

**GTK launcher.** `lib/common/gui/` is 27 files / 9,300 LOC, plus
`gui_login.rb` and `gtk.rb`. It is a full account manager: saved-login tab,
manual-login tab, game selection, frontend selection and a frontend manager tab,
favorites, master-password prompt/change/recovery, encryption-mode conversion,
accessibility, theming, window settings, and a Windows Credential Manager
integration (241 LOC). Entered from `main.rb:197-200` when GTK is present and
either `ARGV` is empty or `--gui` was passed. Disabled by `--no-gui`/`--no-gtk`
(`init.rb:417`).

Because `Gtk.main` owns the process main loop (`lich.rbw:163-165`), there is a
dedicated teardown backstop — `Lich::Common.shutdown_gtk_before_exit(direct: true)`
(`:170`) — to sweep widgets before the interpreter finalizer disposes them in an
unsafe order and segfaults.

**WebUI: not in the Lich-5 core tree.** `grep -rl "LichWebUI|lich_webui"` over
`lib/` returns nothing, and there is no `webui` directory under `lib/common/`.

But it exists and Vellum talks to it. `src/webui.rs:1-24` documents connecting to
"the per-session Lich WebUI server discovered via the `;ui handshake` ->
`<LichWebUI .../>` exchange", at `ws://<host>:<port>/ws`, authenticated by a
`lich_webui=<token>` cookie plus an Origin allowlist, exchanging
`hello`/`pages`/`render`/`close` (server→client) and
`subscribe`/`unsubscribe`/`event` (client→server) envelopes. The token is noted
as "script-level power (WebUI callbacks run Ruby inside Lich)".

**UNVERIFIED: where the Lich-side server lives.** It is in neither place I can
check: `grep -rl LichWebUI` over `lib/` returns nothing, and so does the same
grep over both script corpora (`E:/Cena/reference/scripts`,
`E:/Cena/reference/dr-scripts`). So it is in none of the four reference trees.
Candidates: a Lich version newer than this checkout, a separate
`elanthia-online` repository, or a script distributed outside these corpora.
Resolving this needs a source I do not have. The *client* side of the protocol is
fully implemented in `VellumFE/src/webui.rs` (577 LOC) and is readable as a de
facto spec — Cena could serve that protocol natively without ever seeing the
Ruby implementation.

---

## 12. Shutdown

Worth calling out separately: Lich-5 has 10 files dedicated to orderly shutdown
(`orderly_shutdown.rb`, `shutdown_coordinator.rb`, `shutdown_intent.rb`,
`shutdown_log.rb`, `shutdown_result_predicates.rb`, `shutdown_script_drain.rb`,
`shutdown_watchdog.rb`, `best_effort_shutdown_cleanup.rb`, `script_death.rb`,
`user_exit_dispatch.rb`).

The teardown sequence (`main.rb:1072-1147`), in order, each step timed:
`ActiveSessions connection update` → best-effort cleanup if connection loss →
`script shutdown` (runs every script's `before_dying`/`at_exit` inline) →
`Vars.save` → `Game.close` → `client_thread.kill` →
`detachable_client_thread.kill` + join → `detachable clients close` →
`$_CLIENT_.close` → `Lich.db.close` → `ActiveSessions lifecycle stop` →
`SessionLifecycle stop` → disarm watchdog → `reconnect hook`.

A `ShutdownWatchdog` is armed before the unbounded steps and disarmed after
(`:1072`, `:1130`); it dumps thread backtraces and force-exits if teardown stalls.
Both the primary and the detachable exit paths funnel through
`Lich::Main::UserExitDispatch` (`main.rb:94-99`, `global_defs.rb:2193`) so neither
can run the hang-prone inline script drain without the watchdog armed.

**Cena note.** Most of this complexity exists because Ruby scripts run as real
threads with arbitrary `before_dying` hooks that can hang, and because Lich has
no way to kill them. An embedded Lua runtime with instruction-count hooks and
cooperative cancellation makes most of these ten files unnecessary. Keep the
watchdog and the timed-step tracing; drop the rest.

---

## 13. What Vellum already does

Per infrastructure concern: does VellumFE already have Rust code Cena can build
on, or must it be written fresh?

### 13.1 Summary table

| Concern | Vellum status | Where | Cena effort |
|---|---|---|---|
| EAccess/SGE protocol | **Exists, ~80% complete** | `src/network.rs:710-1050` | Fill gaps (§13.2) |
| Cert pinning to `simu.pem` | **Exists** | `network.rs:794-918` | Reuse as-is |
| Web-login HTTPS fallback | **Absent** | — | Write fresh (~600 LOC) |
| Password storage | **Exists, better than Lich** | `config/profiles.rs:411-440` | Reuse; add import from `entry.yaml` |
| Saved-login store (favorites, ordering) | **Partial** | `config/profiles.rs` (1,039) | Extend |
| Credential scrubbing | **N/A by construction** | — | Not needed (no script-readable ARGV) |
| Game socket + read/parse split | **Exists** | `network.rs:517-627` | Reuse shape |
| Attaching to a Lich detachable listener | **Exists (client side)** | `network.rs:341-374`, `:629-669` | Invert to server side |
| Serving a detachable listener | **Absent** | — | Write fresh |
| Async write queue / backpressure | **Partial** (tokio channels) | `network.rs` | Port `SynchronizedSocket`'s compaction logic |
| Frontend launching / `.sal` / hosts file | **Absent, and mostly unnecessary** | — | Drop (§13.5) |
| Process spawning (detached) | **Exists** | `launcher/flow.rs:130-158` | Reuse |
| Remote launch over SSH | **Exists, no Lich equivalent** | `launcher/ssh.rs` (783) | Keep |
| Multi-session registry | **Exists** | `core/session_registry.rs` (1,646) | Reuse; richer than Lich's |
| Session reuse/handoff policy | **Exists, no Lich equivalent** | `launcher/session_lifecycle.rs` (1,314) | Keep |
| Login-only state cache | **Exists** | `session_cache.rs` (54) | Extend |
| Config paths / profiles | **Exists** | `config/paths.rs` (533) | Reuse |
| Settings storage | **Exists (TOML)**; Lich uses Marshal-in-SQLite | `config/settings.rs` (1,388) | Migration tool needed |
| Logging | **Exists** | `main.rs:303-310` | Reuse |
| Platform abstraction | **Exists but thin** | `platform.rs` (21) | Extend |
| Wine support | **Absent, and unnecessary** | — | Drop |
| Win32 FFI | **Absent except window positioning** | `window_position/windows.rs` (485) | Drop the rest |
| WebUI client | **Exists** | `webui.rs` (577) | Reuse; add server side |
| Update mechanism | **UNVERIFIED** | — | See §13.9 |
| Mobile | **Exists (Android/iOS)** | `android/`, `ios/` | Lich has none |

### 13.2 Authentication — the single biggest head start

Vellum **already implements the EAccess protocol in Rust**, in a private
`mod eaccess` inside `src/network.rs` (`:710-1050`, ~340 LOC). It is a faithful
port:

| Lich | Vellum | Match |
|---|---|---|
| `eaccess.play.net:7910` (`eaccess.rb:100`) | `HOST`/`PORT` consts (`network.rs:720-721`) | ✅ |
| `DATA_DIR/simu.pem` (`eaccess.rb:91`) | `CERT_FILENAME` (`network.rs:722`) | ✅ |
| `sysread(8192)` (`eaccess.rb:353`) | `PACKET_SIZE: usize = 8192`, single `read` (`network.rs:952-973`) | ✅ |
| `((c - 32) ^ key[i]) + 32` (`eaccess.rb:218`) | `obfuscate_password` with the same i32 arithmetic (`network.rs:975-988`) | ✅ |
| Manual cert pin, re-pin on mismatch (`eaccess.rb:125-142`) | `connect_with_cert` compares **DER**, re-downloads on mismatch (`network.rs:902-918`) | ✅ better |
| K, A, F, G, P, C, L | `send_line` for each (`network.rs:758-791`) | see below |
| `auth_with_timeout(30)` (`eaccess.rb:372`) | `AUTH_IO_TIMEOUT = 15s` per-op (`network.rs:815`) | ≈ |

Vellum's port even documents *why* it diverges where it does: SNI is disabled to
match Ruby (`network.rs:806-807`); `pem_to_der` is hand-rolled because
native-tls's `Certificate::from_pem` "is a macOS-only stub that panics at runtime
on iOS" (`network.rs:882-884`) — i.e. it has already solved a mobile problem Cena
would otherwise hit.

**What Vellum's EAccess is missing vs Lich:**

1. **No `M` command.** Lich sends `M\n` and requires `/^M\t/` (`eaccess.rb:230-236`); Vellum goes straight from `A` to `F` (`network.rs:765-773`). Lich's comment calls `M` a "session-affinity" check on a load-balanced backend. Cena should include it.
2. **No `F` response validation.** Lich requires `NORMAL|PREMIUM|TRIAL|INTERNAL|FREE` (`eaccess.rb:249`); Vellum reads and discards (`network.rs:774`). Losing this turns "not subscribed" into a confusing later failure.
3. **No `L\tOK\t` guard.** Lich explicitly requires the `OK` because `L\tPROBLEM\t1` is also `L\t`-prefixed and would parse into a garbage hash (`eaccess.rb:276-286`). Vellum only checks `starts_with('L')` (`network.rs:1010`). **This is a real bug to fix in the port.**
4. **No character generator path** (`NEW_CHARACTER_CODE = "0"`).
5. **No `legacy` multi-game enumeration** (the `N` command loop).
6. **No named stages / `Thread[:eaccess_stage]` diagnostics.** Lich's 10-stage taxonomy (`eaccess.rb:78-87`) is genuinely valuable operationally; port it as a span or an enum.
7. **Character matching differs**: Lich splits on `\t` with a regex scan and matches case-**sensitively** (`eaccess.rb:342-350`); Vellum splits tokens from index 5 in pairs and matches case-**insensitively** (`network.rs:990-1006`). Vellum's is the friendlier behaviour; verify against a real `C` response.
8. **No web-login fallback at all.** This is the largest single gap.

**Recommendation.** Lift `mod eaccess` out of `network.rs` into its own crate-level
module, add the seven items above, and port `web_login.rb` fresh. Budget the web
fallback at ~600 LOC plus an HTML-scraping dependency (`scraper` or a regex port
of `web_login.rb:359`). The instance table (`web_login.rb:144-150`) transcribes
directly to a Rust const table.

**Password storage is already better in Vellum.** `config/profiles.rs:413-440`
uses the **OS credential store** via `keyring` (Windows Credential Manager, macOS
Keychain, Linux Secret Service), keyed by lowercased account name, with a
`password_saved` flag per profile and `account_password_in_use` refcounting before
deleting a shared entry (`:345-348`). On non-desktop (Android/iOS) it falls back
to a **ChaCha20-Poly1305 seal** under a Keystore/Keychain-wrapped key
(`launcher/config.rs:239-246`, `:292-322`). Compare Lich's `:standard` mode, whose
key is PBKDF2 of the *uppercased account name* with a fixed salt
(`password_cipher.rb:129-137`). Cena should adopt Vellum's model outright and
write a one-time importer for `entry.yaml`.

### 13.3 Networking

Vellum's `network.rs` (1,476 LOC) has two connection modes that map onto Lich's:

| Vellum | Lich equivalent |
|---|---|
| `LichConnection::start` (`:341-374`) — connects *to* a Lich detachable listener | the client side of `main.rb:897-964` |
| `DirectConnection::start` (`:377-465`) — EAccess then straight to the game server | `main.rb:226-563` minus the frontend |

`run_stream` (`:517-627`) splits the socket with `tokio::io::split`, spawns a
reader task, and `tokio::select!`s reader against the command channel — the same
read/process separation as Lich's reader+parser threads, but async rather than
two OS threads. It also has `AbortOnDrop` (`:471`) to guarantee the reader task
dies with its half of the socket, and `decode_cp1252` (`:498`) for wire encoding
Lich handles via `force_encoding`.

`send_lich_handshake` (`:629-669`) is a precise mirror of the Lich protocol from
the *other* side, and its comments are the best available documentation of Lich's
quirks:

- In `--key` (Lich-launched frontend) mode it sends key, then `/FE:STORMFRONT /VERSION:1.0.1.26 /P:<os> /XML`, then two `<c>` with 300 ms spacing — explicitly matching "Lich main.rb lines 503-507" (`:648`).
- In detachable mode it sends `SET_FRONTEND_PID <pid>` then `;eq $frontend="stormfront"` (`:655-665`).
- **Critically** (`:366-371`): "Lich's detachable-client thread unconditionally prepends `<c>` to every line it receives, so detachable mode must send bare commands or the game sees `<c><c>cmd`". Confirmed on the Lich side at `global_defs.rb:2196`. Cena must preserve this asymmetry or drop it deliberately.

`DirectConnection` also does concurrent multi-address connect with a per-address
timeout (`:424-440`) — better than Lich's single blocking connect.

**What must be written fresh:** the *server* side. Vellum attaches to listeners;
it never serves one. Cena needs the accept loop, the per-client registry, the
primary election, the attach-time state replay and the fan-out — i.e. all of §5.
`SynchronizedSocket`'s write-queue compaction (`synchronizedsocket.rb:320-403`) has
no Vellum counterpart and is worth porting deliberately: a slow mobile client on a
bad link is exactly the case it was built for.

### 13.4 Multi-session — Vellum is ahead of Lich here

`src/core/session_registry.rs` (1,646 LOC) is a **filesystem-based registry of
running Vellum instances**, machine-local rather than under `VELLUM_FE_DIR`
"because launcher profiles may use different data roots, but they still need one
shared view of which processes and Lich detachable-client endpoints are already
owned" (`:1-11`). Reads garbage-collect entries whose pid is gone (`:8-9`), and it
merges a legacy registry location for one release (`:9-11`).

`SessionConnectionIdentity` (`:28-40`) is an enum of `Direct { game, account }` or
`Lich { host, port }` — i.e. it already models both of Cena's connection modes as
first-class identities, with custom `PartialEq` canonicalization (`:42-70`).

`src/launcher/session_lifecycle.rs` (1,314 LOC) then adds a policy layer Lich has
no equivalent of: `LaunchDisposition` of `Spawn` / `Resume(entry)` /
`Replace(entry)` / `EndpointConflict(SwitchRequest)` (`:23-31`), with a
`SwitchRequest` whose internals are deliberately private so "the launcher may
display the proposal or pass it back for execution, but cannot retarget it after
the user confirms" (`:33-41`). Timeouts: 20 s handoff, 100 ms poll, 2 s control
request, 500 ms endpoint probe (`:18-21`).

Compare Lich's `ActiveSessions` (§6): a lock-file + ephemeral-port + auth-token
JSON service, read-only to scripts. Both solve discovery; Vellum additionally
solves *policy* (reuse vs replace vs conflict). **Cena should build on Vellum's
registry and lifecycle, not Lich's.** The one thing to take from Lich is the
ephemeral-port + published-token design for cross-process RPC, if Cena needs
processes to talk rather than just see each other.

`src/session_cache.rs` (54 LOC) caches data "only sent at login (quickbars, etc.)"
so a frontend can attach to a running Lich without losing it — the client-side
answer to Lich's `detachable_client_send_init` (`global_defs.rb:2141-2163`). In
Cena, where the engine and the frontend are one binary, this collapses: the engine
holds the state and replays it directly. Keep the `CharacterState` persistence
idea (`:24-29`) for warm start.

### 13.5 Frontend launching — mostly deleted, not ported

Cena is one binary with its own frontend. That removes, outright:

- `.sal` file rewriting (`main.rb:478-492`)
- Command-line injection for Wizard/Stormfront (`main.rb:277-295`)
- Hosts-file impersonation (`lich.rb:601-683`) — **and with it the requirement to run as administrator/root**
- SGE/SAL registry hijacking (`lich.rb:364-600`)
- Executable discovery across registry keys, bundle ids and path globs (`frontend_locator.rb`, 425 LOC)
- The Wine layer (`wine.rb`, 138 LOC) and most of the Win32 FFI (`init.rb:29-376`)

What survives is **third-party frontend compatibility**, which is a *serving*
problem, not a launching one: Cena serves a detachable-client listener and a
third-party FE attaches. The frontend catalog's *capability* flags
(`supports_gsl?`, `supports_mono?`, `supports_streams?` — `frontend.rb:531-551`)
do survive, because they select the output dialect per attached client. Vellum has
no equivalent: it only ever speaks as one client. Cena needs to reintroduce this
as a per-connection dialect negotiation.

Vellum's `launcher/flow.rs:130-158` (`spawn_local`) is a clean detached-spawn
primitive (Windows `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`, null stdio, a
named reaper thread) and is the right replacement for `ProcessLauncher.call`
(`process_launcher.rb:21-33`) if Cena ever needs to spawn anything.
`launcher/ssh.rs` (783 LOC) supports launching over SSH to a home PC — a capability
Lich has no counterpart for, and which matters for Cena's mobile story.

### 13.6 Settings, config, paths

Vellum uses **TOML files under a per-profile directory** (`config/paths.rs:77-263`:
`config_dir()`, `profile_dir(name)`, plus ~25 typed path accessors for layouts,
highlights, keybinds, skins, sounds, scenes, alertpacks, images, dolls,
hotbars, controller configs), with `write_atomic` (`:23`) and name validation
(`:46`). `src/config/` totals 32,099 LOC across 19 files.

Lich uses **one SQLite database with Marshal-serialized Ruby-object blobs**
(§8). These are not compatible in any direction. Cena's path:

1. Adopt Vellum's TOML/profile layout for Cena's own settings.
2. For script settings (`Settings`, `CharSettings`, `Vars`) the Lua API must expose
   something, and it must be able to read a user's existing data. **Ship a Ruby
   export tool** that walks `script_auto_settings` and `uservars`, `Marshal.load`s
   each blob and re-emits JSON. Attempting a Rust Marshal reader is possible but
   only worth it if the corpus uses a narrow type set — check the script-corpus
   inventory before deciding.

### 13.7 Logging

Vellum: `tracing` + `tracing-subscriber` with a `tracing_appender::rolling::never`
file appender writing `vellum-fe.log`, and `EnvFilter` defaulting to `info`
(`main.rs:303-310`). Lich: `$stderr.puts` with a timestamp prefix (§9).
Vellum's is strictly better; reuse it. The only Lich behaviour worth keeping is
the per-launch timestamped filename and the pruning sweep
(`init.rb:493`, `:572`).

### 13.8 Platform and mobile

`src/platform.rs` is only 21 lines — a single `open_url` shim, `#[cfg]`-gated on
the `desktop` feature, that hard-errors without it (`:17-20`). The real platform
work is elsewhere: `src/window_position/` (windows.rs 485, macos.rs 203, mod.rs
307, storage.rs 87) and the `android/` and `ios/` trees.

**Lich has no mobile story at all.** Everything in §10 — Win32 Fiddle FFI, Wine,
hosts files, registry keys, `ShellExecuteEx` elevation — is desktop-only and much
of it Windows-only. This is the clearest case where Cena inherits from Vellum and
discards Lich: Vellum already builds for Android and iOS with
`--no-default-features`, and its `desktop`-feature gating pattern is the model for
every platform-specific path in Cena.

The one mobile-relevant thing Lich's design gets right is `--pipe` mode
(`pipe_io.rb`): a transport-agnostic client interface. Cena's in-process frontend
is the same idea taken to its conclusion.

### 13.9 Update mechanism

**UNVERIFIED.** I did not find an updater in the Vellum files within this
assignment's scope. Determining whether Vellum self-updates would require
reading `Cargo.toml` for a crate like `self_update`, and checking `src/main.rs`'s
subcommand list (`main.rs:177-251` defines several subcommands I did not enumerate).

Regardless, Lich's updater does not port: it rewrites Ruby source files in place
(`lib/common/update/file_updater.rb`) and persists branch tracking by generating a
Ruby file that defines constants (`update.rb:256-275`). A compiled binary needs a
different mechanism entirely. What *does* port is `ScriptSync`
(`script_sync.rb`, 161 LOC) and the `SCRIPT_REPOS` shape (`update.rb:42-73`):
GitHub trees API `?recursive=1` for listing, `raw.githubusercontent.com` for
fetching, with a cached `GithubClient`. Cena needs exactly that for its Lua
script corpus.

### 13.10 Highest-risk items, ranked

1. **Web-login fallback** — no Rust code exists, it scrapes HTML, and its own source calls it the most fragile part of the module (`web_login.rb:311`). It is also the only path that works when SGE is down.
2. **The `L\tOK\t` bug in Vellum's EAccess port** (`network.rs:1010` vs `eaccess.rb:279`) — a `PROBLEM` response currently parses as success.
3. **Marshal-serialized settings migration** — blocks every existing user.
4. **Detachable-listener server side** — entirely unwritten, and it is the load-bearing piece for Cena's multi-session goal.
5. **Per-client output dialect negotiation** — Lich's capability system (`frontend.rb:531-551`) has no Vellum counterpart, and getting it wrong breaks third-party frontends silently.
6. **`SynchronizedSocket` write-queue compaction** — subtle, well-reasoned, and exactly what a mobile client on a flaky link needs.
