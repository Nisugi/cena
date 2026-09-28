//! `plan/46` §9's measurements (§11 step 6), printed rather than asserted:
//! what a runner costs, and how long a line takes, over a real hunt's combat
//! replayed through a scripted game -- `cena-behavior`'s `smithy_kill.xml`,
//! a pegasus fought and killed: 28 chunks, 278 lines, 70 KB in 11 game
//! seconds, each chunk the answer to one `next`.
//!
//! Tier 2: it needs Ruby 4.0 and only prints. Run it with
//! `cargo test -p cena-agent --test measure -- --ignored --nocapture --test-threads 1`.
//! A debug build, as every build here is (`CLAUDE.md`): Hydra's own side is
//! slower than a release's; the runner's Ruby is the same either way.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::Request;
use cena_agent::scripts::runner::{Start, find_ruby, start, unpack};
use cena_agent::scripts::{Runners, router, serve};
use cena_platform::AnsweringSource;
use cena_session::{Event, Session, SessionHandle};
use http_body_util::BodyExt;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

const DEADLINE: Duration = Duration::from_secs(5);
const WARM: &[u8] = b"You glance about.\n<prompt time=\"2\">&gt;</prompt>\n";
const QUIET_ROOM: &[u8] = b"You see a quiet room.\n<prompt time=\"3\">&gt;</prompt>\n";
/// The game seconds the combat spans: its first prompt's time to its last.
const GAME_SECONDS: usize = 11;

/// The combat, a chunk to each prompt.
fn chunks() -> Option<Vec<Vec<u8>>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-behavior/tests/fixtures/smithy_kill.xml");
    let text = std::fs::read(path).ok()?;
    let mut chunks = Vec::new();
    let mut rest = text.as_slice();
    while let Some(at) = find(rest, b"</prompt>") {
        let mut end = at + b"</prompt>".len();
        while rest.get(end).is_some_and(|b| *b == b'\n' || *b == b'\r') {
            end += 1;
        }
        chunks.push(rest[..end].to_vec());
        rest = &rest[end..];
    }
    Some(chunks)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Median, 95th percentile and most of `values`, in milliseconds.
fn summary(label: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        println!("{label}: none");
        return;
    }
    let at = |percent: usize| values[(values.len() - 1) * percent / 100];
    println!(
        "{label}: n={} median={:.2} ms p95={:.2} ms max={:.2} ms",
        values.len(),
        at(50),
        at(95),
        at(100)
    );
}

/// The runner's working set and private memory, in MiB, as `Get-Process`
/// reads them.
fn memory(pid: u32) -> String {
    if !cfg!(windows) {
        return "not measured here (Get-Process is Windows')".to_owned();
    }
    let read = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$p = Get-Process -Id {pid}; '{{0}} {{1}}' -f $p.WorkingSet64, $p.PrivateMemorySize64"
            ),
        ])
        .output();
    let Ok(read) = read else {
        return "unread".to_owned();
    };
    let text = String::from_utf8_lossy(&read.stdout);
    let mib: Vec<f64> = text
        .split_whitespace()
        .filter_map(|n| n.parse::<f64>().ok())
        .map(|b| b / 1_048_576.0)
        .collect();
    match mib.as_slice() {
        [working, private] => format!("{working:.1} MiB working set, {private:.1} MiB private"),
        _ => format!("unread: {text}"),
    }
}

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-measure-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A runner's start: from starting Ruby to its first `listen` reaching a
/// socket, as §9 measured it before; without windows and with them (the
/// gtk3 gem loaded), with the memory of the last of each.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Tier 2: plan/46 §9's measurements; needs Ruby 4.0; prints"]
async fn a_runners_start() {
    let ruby = find_ruby().expect("no Ruby");
    let dir = scratch("start");
    unpack(&dir.join("runner")).unwrap();
    for folder in ["scripts", "data"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
    }
    for windows in [false, true] {
        let mut times = Vec::new();
        let mut kept = String::new();
        for run in 0..6 {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            let url = format!("http://{}/mcp", listener.local_addr().unwrap());
            let started = Instant::now();
            let mut child = start(&Start {
                ruby: &ruby,
                dir: &dir.join("runner"),
                url: &url,
                token: "measure",
                character: "Nisugi",
                game: "GS3",
                scripts: &dir.join("scripts"),
                data: &dir.join("data"),
                symbol: ';',
                windows,
            })
            .unwrap();
            let accepted = tokio::time::timeout(Duration::from_mins(1), listener.accept()).await;
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            assert!(accepted.is_ok(), "the runner never listened");
            if run == 5 {
                tokio::time::sleep(Duration::from_secs(3)).await;
                kept = memory(child.id().unwrap_or_default());
            }
            let _ = child.kill().await;
        }
        let with = if windows {
            "with windows"
        } else {
            "no windows"
        };
        summary(
            &format!("a runner's start, {with}, Ruby to its first listen"),
            times,
        );
        println!("a runner, {with}, no script: {kept}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn request(token: &str, body: &serde_json::Value) -> Request<Body> {
    Request::post("/mcp")
        .header("host", "127.0.0.1")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap_or_default()
}

/// `listen`'s answer from `since`, at once.
async fn listen(app: &axum::Router, token: &str, since: u64) -> Option<serde_json::Value> {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "listen", "arguments": {"since": since, "timeout_ms": 0}}});
    let response = app.clone().oneshot(request(token, &body)).await.ok()?;
    let bytes = response.into_body().collect().await.ok()?.to_bytes();
    let reply: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    serde_json::from_str(reply.pointer("/result/content/0/text")?.as_str()?).ok()
}

/// Every event `listen` gives after `since`, and where to ask next.
async fn heard(app: &axum::Router, token: &str, mut since: u64) -> (Vec<serde_json::Value>, u64) {
    let mut events = Vec::new();
    while let Some(answer) = listen(app, token, since).await {
        let batch = answer["events"].as_array().cloned().unwrap_or_default();
        if batch.is_empty() {
            break;
        }
        events.extend(batch);
        since = answer["next"].as_u64().unwrap_or(since);
    }
    (events, since)
}

/// A session over the scripted game, answering `glance` once to build its
/// classifiers (1.6 s the first time, in a debug build), `look` with one
/// line, and `next` with the combat's chunks, `rounds` times over.
async fn character(
    rounds: usize,
    looks: usize,
) -> Option<(SessionHandle, cena_session::SessionObserver)> {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    transcript.answer("glance", WARM);
    for _ in 0..looks {
        transcript.answer("look", QUIET_ROOM);
    }
    for _ in 0..rounds {
        for chunk in chunks()? {
            transcript.answer("next", &chunk);
        }
    }
    let session = Session::new(source);
    let (handle, observer) = (session.handle(), session.observer());
    tokio::spawn(session.into_actor().run());
    handle
        .send_manual_at(handle.generation(), "glance", DEADLINE)
        .await;
    Some((handle, observer))
}

/// What a runner is told of the combat (§9: the local copy's update size
/// and rate): its `state` events and its lines, as `listen` gives them.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Tier 2: plan/46 §9's measurements; prints"]
async fn the_local_copys_updates_in_combat() {
    let (handle, observer) = character(1, 0).await.unwrap();
    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let app = router(runners.clone(), &CancellationToken::new());
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (first, since) = heard(&app, &token, 0).await;
    let whole: usize = first
        .iter()
        .filter(|e| e["kind"] == "state")
        .map(|e| e.to_string().len())
        .sum();
    let chunks = chunks().unwrap();
    for _ in &chunks {
        handle
            .send_manual_at(handle.generation(), "next", DEADLINE)
            .await;
    }
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (events, _) = heard(&app, &token, since).await;
    let size = |kind: &str| -> (usize, usize) {
        let picked: Vec<usize> = events
            .iter()
            .filter(|e| e["kind"] == kind)
            .map(|e| e.to_string().len())
            .collect();
        (picked.len(), picked.iter().sum())
    };
    let (states, state_bytes) = size("state");
    let (lines, line_bytes) = size("line");
    let (prompts, _) = size("prompt");
    let mut each: Vec<usize> = events
        .iter()
        .filter(|e| e["kind"] == "state")
        .map(|e| e.to_string().len())
        .collect();
    each.sort_unstable();
    println!("the first copy, whole: {whole} bytes");
    println!(
        "combat, {} chunks over {GAME_SECONDS} game seconds: {states} state events, {state_bytes} bytes \
         ({} bytes a chunk, {} bytes a game second; median {}, most {} bytes each); \
         {lines} lines, {line_bytes} bytes; {prompts} prompts",
        chunks.len(),
        state_bytes / chunks.len(),
        state_bytes / GAME_SECONDS,
        each.get(each.len() / 2).copied().unwrap_or_default(),
        each.last().copied().unwrap_or_default(),
    );
    let mut fields: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for event in events.iter().filter(|e| e["kind"] == "state") {
        for field in event["fields"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k)
        {
            *fields.entry(field.clone()).or_default() += 1;
        }
    }
    println!("fields changed, by how many state events: {fields:?}");
}

/// When each game line, each line shown and each prompt was published, as
/// the session's stream gives them.
#[derive(Default)]
struct Published {
    heard: Vec<(f64, String)>,
    shown: Vec<(f64, String)>,
    prompts: Vec<f64>,
}

fn record(observer: &cena_session::SessionObserver) -> Arc<Mutex<Published>> {
    let published = Arc::new(Mutex::new(Published::default()));
    let kept = Arc::clone(&published);
    let observer = observer.clone();
    tokio::spawn(async move {
        let Ok((_, mut events)) = observer.subscribe().await else {
            return;
        };
        while let Ok(event) = events.recv().await {
            let at = now();
            let mut published = kept.lock().unwrap_or_else(PoisonError::into_inner);
            match event.event {
                Event::Heard(line) => published.heard.push((at, line.text().trim().to_owned())),
                Event::Line(line) => published.shown.push((at, line.text().trim().to_owned())),
                Event::Frame(frame) if matches!(*frame, cena_session::Frame::Prompt { .. }) => {
                    published.prompts.push(at);
                }
                _ => {}
            }
        }
    });
    published
}

/// What a script kept: when it sent, and when it heard what.
#[derive(serde::Deserialize)]
struct Measured {
    sent: Vec<f64>,
    heard: Vec<(f64, String)>,
}

/// What the script wrote for `mode`, once it has; `None` after two minutes.
async fn measured(scripts: &Path, mode: &str) -> Option<Measured> {
    let file = scripts.join(format!("measured-{mode}.json"));
    for _ in 0..1200 {
        if let Ok(text) = std::fs::read_to_string(&file)
            && let Ok(measured) = serde_json::from_str::<Measured>(&text)
        {
            let _ = std::fs::remove_file(&file);
            return Some(measured);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

/// Lich's own path for the same round trip, in one process
/// (`tests/fixtures/lichpath.rb`), in milliseconds.
fn lich_path(ruby: &Path, runner: &Path, out: &Path, count: usize) -> Option<Vec<f64>> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lichpath.rb");
    let ran = std::process::Command::new(ruby)
        .arg(fixture)
        .arg(runner)
        .arg(out)
        .arg(count.to_string())
        .output()
        .ok()?;
    ran.status.success().then_some(())?;
    let times: Vec<f64> = serde_json::from_str(&std::fs::read_to_string(out).ok()?).ok()?;
    Some(times.into_iter().map(|s| s * 1000.0).collect())
}

/// What one pass over the combat measured, printed.
fn report(phase: &str, published: &Published, combat: &Measured, from: f64, to: f64) {
    let [whole, chunk, bridge] = to_the_script(published, combat, from);
    summary(
        &format!("{phase}: a line, the session to the script"),
        whole,
    );
    summary(&format!("{phase}:   of it, to its chunk's prompt"), chunk);
    summary(
        &format!("{phase}:   of it, the prompt to the script"),
        bridge,
    );
    // The chunk the session took longest to read: from its first line to
    // its prompt, the lines after the prompt before it.
    let prompts: Vec<f64> = published
        .prompts
        .iter()
        .copied()
        .filter(|p| *p >= from && *p <= to)
        .collect();
    let slowest = prompts
        .iter()
        .enumerate()
        .filter_map(|(i, prompt)| {
            let after = if i == 0 { from } else { prompts[i - 1] };
            let first = published
                .heard
                .iter()
                .find(|(t, _)| *t > after && t <= prompt)?;
            Some(((prompt - first.0) * 1000.0, first.1.clone()))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((ms, line)) = slowest {
        println!(
            "{phase}: slowest chunk to read, {ms:.0} ms from its first line: {:?}",
            &line[..line.len().min(70)]
        );
    }
    // A reply is what came before the next send: a chunk with no line a
    // script hears has none.
    let replies: Vec<f64> = combat
        .sent
        .iter()
        .enumerate()
        .filter_map(|(i, sent)| {
            let until = combat.sent.get(i + 1).copied().unwrap_or(f64::MAX);
            combat
                .heard
                .iter()
                .find(|(heard, _)| heard >= sent && *heard < until)
                .map(|(heard, _)| (heard - sent) * 1000.0)
        })
        .collect();
    summary(
        &format!("{phase}: `next` to its reply's first line"),
        replies,
    );
    summary(
        &format!("{phase}: a line published to its showing"),
        shown_after(published, from, to),
    );
}

/// Each line the script heard after `from`, against when the session
/// published it: how long a line takes from the session to a script, and
/// the two parts of it, until its chunk's prompt (the session reading the
/// rest of the chunk) and from the prompt (the bridge).
fn to_the_script(published: &Published, measured: &Measured, from: f64) -> [Vec<f64>; 3] {
    let mut at = published.heard.partition_point(|(t, _)| *t < from);
    let [mut whole, mut chunk, mut bridge] = [Vec::new(), Vec::new(), Vec::new()];
    for (heard_at, text) in &measured.heard {
        let text = text.trim();
        if let Some(offset) = published.heard[at..].iter().position(|(_, h)| h == text) {
            let published_at = published.heard[at + offset].0;
            whole.push((heard_at - published_at) * 1000.0);
            let prompt = published.prompts
                [published.prompts.partition_point(|t| *t < published_at)..]
                .first()
                .copied();
            if let Some(prompt) = prompt.filter(|p| p <= heard_at) {
                chunk.push((prompt - published_at) * 1000.0);
                bridge.push((heard_at - prompt) * 1000.0);
            }
            at += offset + 1;
        }
    }
    [whole, chunk, bridge]
}

/// Each game line published between `from` and `to`, against when what
/// the player is shown of it was: how long the showing waited.
fn shown_after(published: &Published, from: f64, to: f64) -> Vec<f64> {
    let mut at = published.shown.partition_point(|(t, _)| *t < from);
    let mut delays = Vec::new();
    for (heard_at, text) in published
        .heard
        .iter()
        .filter(|(t, _)| *t >= from && *t <= to)
    {
        if let Some(offset) = published.shown[at..].iter().position(|(_, s)| s == text) {
            delays.push((published.shown[at + offset].0 - heard_at) * 1000.0);
            at += offset + 1;
        }
    }
    delays
}

/// A runner in combat, and at rest: a line's time from the session to a
/// script, a send to its reply against Lich's own path, a hooked line's
/// wait to be shown, and the runner's memory with 0, 2 and 12 scripts.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Tier 2: plan/46 §9's measurements; needs Ruby 4.0; prints"]
async fn a_runner_in_combat() {
    let ruby = find_ruby().expect("no Ruby");
    let dir = scratch("combat");
    let scripts = dir.join("scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in ["measure.lic", "measurehook.lic"] {
        std::fs::copy(fixtures.join(name), scripts.join(name)).unwrap();
    }
    for n in 1..=10 {
        std::fs::write(scripts.join(format!("idle{n}.lic")), "loop { sleep 1 }\n").unwrap();
    }
    let looks = 50;
    let (handle, observer) = character(2, looks).await.unwrap();
    let published = record(&observer);
    let runners = Runners::default();
    let stop = CancellationToken::new();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, runners.clone(), stop.clone()));
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    unpack(&dir.join("runner")).unwrap();
    let mut child = start(&Start {
        ruby: &ruby,
        dir: &dir.join("runner"),
        url: &url,
        token: &token,
        character: "Nisugi",
        game: "GS3",
        scripts: &scripts,
        data: &dir.join("data"),
        symbol: ';',
        windows: false,
    })
    .unwrap();
    let pid = child.id().unwrap_or_default();
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("the runner, no script: {}", memory(pid));

    assert!(runners.typed(&token, &format!("measure look {looks}")));
    let look = measured(&scripts, "look").await.unwrap();
    let round_trips: Vec<f64> = look
        .sent
        .iter()
        .zip(&look.heard)
        .map(|(sent, (heard, _))| (heard - sent) * 1000.0)
        .collect();
    summary(
        "a script's `look` to its one-line reply, under Hydra",
        round_trips,
    );
    let lich = lich_path(
        &ruby,
        &dir.join("runner"),
        &dir.join("lichpath.json"),
        looks,
    );
    summary(
        "the same under Lich's own path, in one process",
        lich.unwrap(),
    );

    for (phase, hook) in [("combat", false), ("combat, a display hook on", true)] {
        if hook {
            assert!(runners.typed(&token, "measurehook"));
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let from = now();
        assert!(runners.typed(&token, &format!("measure next {}", chunks().unwrap().len())));
        let combat = measured(&scripts, "next").await.unwrap();
        let to = now();
        let published = published.lock().unwrap_or_else(PoisonError::into_inner);
        report(phase, &published, &combat, from, to);
        println!("the runner after {phase}: {}", memory(pid));
    }

    for n in 1..=10 {
        assert!(runners.typed(&token, &format!("idle{n}")));
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("the runner, 12 scripts running: {}", memory(pid));

    let _ = child.kill().await;
    runners.dismiss(&token);
    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}
