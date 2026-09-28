//! Throwaway. Can real Lich, in pipe mode, take Hydra as its game server?
//!
//! The relay: Hydra keeps the game connection, and starts Lich with
//! `--pipe -g 127.0.0.1:<port>`, a port Hydra holds. Lich connects to it as
//! it would to the game, and Hydra hands it the game's bytes. The player's
//! typing goes to Lich's stdin; what Lich would show a frontend comes out of
//! its stdout (lib/main/main.rb:564-571, lib/common/pipe_io.rb).
//!
//! Nothing here reaches the game. The "game" is committed recordings, and
//! Lich runs from a fresh folder: no one's install, and no login cache.
//!
//! cargo run -- <lich dir> <work dir> <effect-list.xml> <fixture>... [--fe=<frontend>]
//!
//! `--fe=stormfront` starts Lich as `--stormfront`: the frontend it names
//! reaches Lich only with pipe mode keeping it (`lich-pipe-frontend.patch`).

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// One line off a stream, with when it arrived. `None` is end of stream.
type Line = (Instant, Option<String>);

fn lines(reader: impl Read + Send + 'static) -> Receiver<Line> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut buf = Vec::new();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => {
                    let _ = tx.send((Instant::now(), None));
                    return;
                }
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    if tx.send((Instant::now(), Some(text))).is_err() {
                        return;
                    }
                }
            }
        }
    });
    rx
}

/// The line as written, its control bytes visible.
fn shown(line: &str) -> String {
    line.replace('\x1f', "<US>")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// Waits for a line that `wanted` accepts, printing every line it passes.
/// Returns the line and when it came, or `None` on timeout or end.
fn expect(
    rx: &Receiver<Line>,
    tag: &str,
    wait: Duration,
    mut wanted: impl FnMut(&str) -> bool,
) -> Option<(Instant, String)> {
    let deadline = Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((at, Some(line))) => {
                println!("  {tag} | {}", shown(&line));
                if wanted(&line) {
                    return Some((at, line));
                }
            }
            Ok((_, None)) => {
                println!("  {tag} | <end of stream>");
                return None;
            }
            Err(RecvTimeoutError::Timeout) => {
                println!("  {tag} | <timed out>");
                return None;
            }
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// Prints what arrives on `rx` for `wait`, and returns it.
fn drain(rx: &Receiver<Line>, tag: &str, wait: Duration) -> Vec<String> {
    let deadline = Instant::now() + wait;
    let mut seen = Vec::new();
    while let Ok((_, line)) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        match line {
            Some(line) => {
                println!("  {tag} | {}", shown(&line));
                seen.push(line);
            }
            None => {
                println!("  {tag} | <end of stream>");
                break;
            }
        }
    }
    seen
}

fn memory(pid: u32) -> String {
    let script = format!(
        "$p = Get-Process -Id {pid}; '{{0:N0}} MB working set, {{1:N0}} MB committed' -f ($p.WorkingSet64/1MB), ($p.PrivateMemorySize64/1MB)"
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| format!("unmeasured: {e}"))
}

fn accept(listener: &TcpListener, wait: Duration) -> Option<TcpStream> {
    listener.set_nonblocking(true).ok()?;
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if let Ok((stream, _)) = listener.accept() {
            stream.set_nonblocking(false).ok()?;
            return Some(stream);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

fn step(title: &str) {
    println!("\n== {title}");
}

fn verdict(ok: bool, what: &str) {
    println!("{} {what}", if ok { "PASS" } else { "FAIL" });
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let frontend = args.iter().find_map(|a| a.strip_prefix("--fe=")).map(str::to_string);
    args.retain(|a| !a.starts_with("--fe="));
    let [lich, work, effects, fixtures @ ..] = args.as_slice() else {
        eprintln!("usage: <lich dir> <work dir> <effect-list.xml> <fixture>... [--fe=<frontend>]");
        std::process::exit(2);
    };
    let lich = PathBuf::from(lich);

    // A fresh Lich home. effect-list.xml is given, or Lich fetches it from
    // GitHub (lib/common/spell.rb:188).
    let home = PathBuf::from(work).join("home");
    let dir = |name: &str| home.join(name);
    for name in ["data", "temp", "scripts", "maps", "logs", "backup", "sessions"] {
        fs::create_dir_all(dir(name)).expect("make the Lich home");
    }
    fs::copy(effects, dir("data").join("effect-list.xml")).expect("copy effect-list.xml");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("port").port();
    let flag = |name: &str, path: &Path| format!("--{name}={}", path.display());

    step(&format!("start Lich, pointed at 127.0.0.1:{port}, frontend flag {frontend:?}"));
    let started = Instant::now();
    let mut command = Command::new("ruby");
    command
        .arg(lich.join("lich.rbw"))
        .args(["--pipe", "--no-gtk", "--gemstone", "-g", &format!("127.0.0.1:{port}")])
        .arg(flag("home", &home))
        .arg(flag("lib", &lich.join("lib")))
        .arg(flag("data", &dir("data")))
        .arg(flag("temp", &dir("temp")))
        .arg(flag("scripts", &dir("scripts")))
        .arg(flag("maps", &dir("maps")))
        .arg(flag("logs", &dir("logs")))
        .arg(flag("backup", &dir("backup")))
        .arg(flag("active-session-dir", &dir("sessions")))
        // Offline: Lich syncs its script repositories from GitHub at every
        // login (lib/games.rb:994-1010), through Net::HTTP, which honours a
        // proxy from the environment. This one refuses.
        .env("https_proxy", "http://127.0.0.1:9")
        .env("http_proxy", "http://127.0.0.1:9")
        .env_remove("no_proxy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(frontend) = &frontend {
        command.arg(format!("--{frontend}"));
    }
    let mut child = command.spawn().expect("start ruby");
    let pid = child.id();
    let out = lines(child.stdout.take().expect("stdout"));
    let err = lines(child.stderr.take().expect("stderr"));
    thread::spawn(move || {
        while let Ok((_, Some(line))) = err.recv() {
            println!("  stderr | {}", shown(&line));
        }
    });
    let mut stdin = child.stdin.take().expect("stdin");
    let mut type_line = move |line: &str| {
        println!("  typed  > {line}");
        writeln!(stdin, "{line}").expect("write to Lich");
    };

    // What a frontend sends first: the key, then its version line.
    type_line("SPIKE-KEY-NOT-REAL");
    type_line("/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML");

    let Some(mut game) = accept(&listener, Duration::from_secs(60)) else {
        verdict(false, "Lich connected within 60 s");
        drain(&out, "lich", Duration::from_secs(2));
        let _ = child.kill();
        return;
    };
    let connected = started.elapsed();
    verdict(true, &format!("Lich connected after {:.2} s", connected.as_secs_f64()));
    let from_lich = lines(game.try_clone().expect("clone"));

    step("the key and version, as Lich passes them on");
    let key = expect(&from_lich, "->game", Duration::from_secs(10), |_| true);
    verdict(key.is_some_and(|(_, l)| l.trim() == "SPIKE-KEY-NOT-REAL"), "the key arrives first, unchanged");
    let version = expect(&from_lich, "->game", Duration::from_secs(10), |_| true);
    verdict(version.is_some_and(|(_, l)| l.contains("/FE:")), "the version line arrives second");

    step("Hydra answers as the game: the recorded login");
    let sent = Instant::now();
    for fixture in fixtures {
        game.write_all(&fs::read(fixture).expect("read fixture")).expect("write burst");
    }
    game.write_all(b"<prompt time=\"1790000000\">&gt;</prompt>\r\n").expect("prompt");
    let shown_room = expect(&out, "lich", Duration::from_secs(30), |l| l.contains("Obvious"));
    if let Some((at, _)) = &shown_room {
        verdict(true, &format!("the burst reaches Lich's stdout, first room line {:.0} ms after sending", at.duration_since(sent).as_secs_f64() * 1000.0));
    } else {
        verdict(false, "the burst reaches Lich's stdout");
    }
    drain(&out, "lich", Duration::from_secs(3));
    println!("  (what Lich sent the game after login, unasked:)");
    let unasked = drain(&from_lich, "->game", Duration::from_secs(3));

    step("a Lich line on stdout");
    type_line(";e respond 'hello from lich'");
    let hello = expect(&out, "lich", Duration::from_secs(15), |l| l.contains("hello from lich"));
    let marked = hello.as_ref().is_some_and(|(_, l)| l.starts_with('\x1f'));
    verdict(hello.is_some(), "respond reaches stdout");
    println!("     marked with the origin sentinel: {marked}");

    step("what Lich read from Hydra's bytes");
    type_line(";e respond \"name=#{XMLData.name} game=#{XMLData.game} room=#{XMLData.room_id} right=#{GameObj.right_hand.name} left=#{GameObj.left_hand.name} title=#{XMLData.room_title} exits=#{XMLData.room_exits.inspect}\"");
    let read = expect(&out, "lich", Duration::from_secs(15), |l| l.contains("name="));
    verdict(read.is_some_and(|(_, l)| l.contains("room=2022628") || l.contains("room=7086")), "XMLData knows the room from the replay");

    step("a script's command reaches Hydra");
    type_line(";e put 'look'");
    let look = expect(&from_lich, "->game", Duration::from_secs(15), |l| l.contains("look"));
    verdict(look.is_some(), "put arrives on Hydra's socket");

    step("the player's typing, through Lich");
    type_line("exp");
    let exp = expect(&from_lich, "->game", Duration::from_secs(15), |l| l.contains("exp"));
    verdict(exp.is_some(), "a typed command arrives on Hydra's socket");

    step("a downstream hook: applied to stdout only");
    type_line(";e DownstreamHook.add('spike', proc { |s| s.include?('squelch me') ? nil : s }); respond 'hooked'; sleep 5");
    expect(&out, "lich", Duration::from_secs(15), |l| l.contains("hooked"));
    game.write_all(b"squelch me please\r\nkeep me\r\n<prompt time=\"1790000001\">&gt;</prompt>\r\n").expect("write");
    let mut squelched = true;
    let kept = expect(&out, "lich", Duration::from_secs(15), |l| {
        if l.contains("squelch me") {
            squelched = false;
        }
        l.contains("keep me")
    });
    verdict(kept.is_some() && squelched, "the hook drops the line from what Lich shows");

    step("an upstream hook: Lich rewrites typing before Hydra sees it");
    type_line(";e UpstreamHook.add('spike', proc { |s| s.sub('hello', 'goodbye') }); respond 'uphooked'; sleep 5");
    expect(&out, "lich", Duration::from_secs(15), |l| l.contains("uphooked"));
    type_line("say hello");
    let said = expect(&from_lich, "->game", Duration::from_secs(15), |l| l.contains("say"));
    verdict(said.is_some_and(|(_, l)| l.contains("goodbye")), "the rewritten line is what Hydra gets");

    step("script text with angle brackets");
    type_line(";e respond \"frontend=#{$frontend} <b>not a tag</b> 5 < 6\"");
    let raw = expect(&out, "lich", Duration::from_secs(15), |l| l.contains("not a tag"));
    verdict(raw.is_some_and(|(_, l)| l.contains("&lt;b&gt;")), "script text is escaped for an XML frontend");

    step("a game line after login, marked or not");
    game.write_all(b"a game line after login
<prompt time=\"1790000002\">&gt;</prompt>
").expect("write");
    let line = expect(&out, "lich", Duration::from_secs(15), |l| l.contains("after login"));
    println!("     marked with the origin sentinel: {}", line.is_some_and(|(_, l)| l.starts_with('')));

    step("memory, settled");
    thread::sleep(Duration::from_secs(2));
    println!("  {}", memory(pid));

    step("Hydra stops Lich for this character: its stdin closes");
    let stopping = Instant::now();
    drop(type_line);
    let mut exited = None;
    while stopping.elapsed() < Duration::from_secs(20) {
        if let Ok(Some(status)) = child.try_wait() {
            exited = Some(status);
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let socket_closed = expect(&from_lich, "->game", Duration::from_secs(5), |_| false);
    drain(&out, "lich", Duration::from_millis(500));
    match exited {
        Some(status) => verdict(true, &format!("Lich exited ({status}) {:.2} s after its stdin closed", stopping.elapsed().as_secs_f64())),
        None => {
            verdict(false, "Lich exited within 20 s of its stdin closing");
            let _ = child.kill();
        }
    }
    let _ = socket_closed;

    step("summary");
    println!("  connect after start: {:.2} s", connected.as_secs_f64());
    println!("  lines Lich sent the game unasked after login: {}", unasked.len());
    println!("  Lich lines marked: {marked}");
}
