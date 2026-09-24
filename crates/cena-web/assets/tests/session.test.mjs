import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { HydraSession, MAX_STORY_LINES, commandError, launchSession, takeLaunchToken } from "../session.js";
import { lifecycleText, mount, placeLine } from "../app.js";

// Shared synthetic contract fixture, also round-tripped by Rust cena-ui tests.
export const fixture = () => JSON.parse(readFileSync(new URL("../../../cena-ui/tests/fixtures/snapshot-v1.json", import.meta.url)));

export class FakeSocket {
  constructor(url) { this.url = url; this.readyState = 0; this.sent = []; }
  open() { this.readyState = 1; this.onopen(); }
  send(data) { this.sent.push(JSON.parse(data)); }
  message(data) { this.onmessage({ data: JSON.stringify(data) }); }
  close(code = 1000) { this.readyState = 3; this.onclose?.({ code }); }
}

function setup() {
  const timers = [];
  const session = new HydraSession({ url: "ws://127.0.0.1:1234/ws", token: "synthetic-token",
    WebSocketImpl: FakeSocket, onChange() {}, schedule(fn, delay) { timers.push({ fn, delay }); return timers.length; }, cancel() {} });
  session.connect();
  session.socket.open();
  return { session, timers, socket: session.socket };
}

function readySnapshot() {
  const value = fixture();
  value.view.lifecycle = { kind: "ready" };
  return value;
}

function update(snapshot, extra = {}) {
  const { story, history_gap, ...value } = structuredClone(snapshot);
  return { ...value, kind: "update", cursor: String(BigInt(value.cursor) + 1n), lines: [], ...extra };
}

// "We do not know whether that command ran" -- matched as a PROPERTY of the
// message rather than as its exact words. A display string was previously also
// used as a sentinel in session.js, and rewording it broke a comparison
// silently; these assertions are kept loose for the same reason.
const UNSURE = /may or may not|Not sure if/;

test("launch token is removed before use and query strings never supply authentication", () => {
  const writes = [];
  const token = takeLaunchToken({ hash: "#token=synthetic-token", pathname: "/", search: "" },
    { replaceState(...args) { writes.push(args); } });
  assert.equal(token, "synthetic-token");
  assert.deepEqual(writes, [[null, "", "/"]]);
  assert.equal(takeLaunchToken({ hash: "", pathname: "/", search: "?token=ignored" }, {}), "");
});

test("a page names its session from the launch fragment, and says so when it authenticates", () => {
  // plan/29 step 5: one listener serves every character, so a page opened for
  // one of them names it. Read before the token, which clears the fragment.
  assert.equal(launchSession({ hash: "#token=t&session=7" }), "7");
  assert.equal(launchSession({ hash: "#token=t" }), null);
  for (const bad of ["07", "-1", "x", ""]) {
    assert.equal(launchSession({ hash: `#token=t&session=${bad}` }), null, bad);
  }
  const session = new HydraSession({ url: "ws://127.0.0.1/ws", token: "synthetic-token", sessionId: "7",
    WebSocketImpl: FakeSocket, onChange() {}, schedule() { return 1; }, cancel() {} });
  session.connect();
  session.socket.open();
  assert.deepEqual(session.socket.sent,
    [{ kind: "authenticate", version: 1, token: "synthetic-token", session: "7" }]);
  session.close();
});

test("authenticate is first; commands require authenticated snapshot and Ready lifecycle", () => {
  const { session, socket } = setup();
  assert.deepEqual(socket.sent, [{ kind: "authenticate", version: 1, token: "synthetic-token" }]);
  assert.equal(session.command("look"), false);
  socket.message(fixture());
  assert.equal(session.command("look"), false);
  socket.message(update(fixture(), { view: readySnapshot().view }));
  assert.equal(session.command(" look "), true);
  assert.equal(socket.sent[1].line, " look ");
  assert.equal(socket.sent[1].session, "18446744073709551615");
  assert.equal(socket.sent[1].generation, "9007199254740993");
});

test("unknowns, emptiness, styled text, and history gaps survive the shared fixture", () => {
  const { session, socket } = setup();
  socket.message(fixture());
  assert.deepEqual(session.state.view, fixture().view);
  assert.deepEqual(session.state.story, fixture().story);
  assert.equal(session.state.historyGap, true);
});

test("cursor comparison is lossless and stale messages cannot regress state", () => {
  const { session, socket } = setup();
  const snapshot = readySnapshot();
  socket.message(snapshot);
  const next = update(snapshot, { cursor: "9007199254740995" });
  next.view.room.title = "New room";
  socket.message(next);
  socket.message(snapshot);
  socket.message(update(snapshot, { cursor: "9007199254740996", generation: "9007199254740992" }));
  socket.message(update(snapshot, { cursor: "9007199254740996", session: "1" }));
  assert.equal(session.state.cursor, "9007199254740995");
  assert.equal(session.state.view.room.title, "New room");
});

test("reconnect authenticates afresh, replaces history, and never replays commands", () => {
  const { session, socket, timers } = setup();
  socket.message(readySnapshot());
  session.command("north");
  socket.close(1006);
  assert.equal(session.ready, false);
  assert.equal(session.state.view, null);
  assert.match(session.state.commandStatus, UNSURE);
  assert.equal(timers[0].delay, 1000);
  timers[0].fn();
  const replacement = session.socket;
  replacement.open();
  assert.deepEqual(replacement.sent.map((m) => m.kind), ["authenticate"]);
  const fresh = readySnapshot();
  fresh.cursor = "0";
  fresh.story = [];
  replacement.message(fresh);
  assert.equal(session.ready, true);
  assert.equal(session.state.story.length, 0);
  assert.match(session.state.commandStatus, UNSURE);
  socket.message(update(fresh)); // late callback from old socket is ignored
  assert.equal(session.state.cursor, "0");
});

test("generation change invalidates pending delivery and respects supplied unknown state", () => {
  const { session, socket } = setup();
  socket.message(readySnapshot());
  session.command("look");
  const next = update(fixture(), { generation: "9007199254740994" });
  socket.message(next);
  assert.equal(session.ready, false);
  assert.equal(session.state.view.left_hand.kind, "unknown");
  assert.equal(session.pending.size, 0);
  assert.match(session.state.commandStatus, UNSURE);
  socket.message({ kind: "receipt", version: 1, session: fixture().session, generation: fixture().generation,
    request_id: "1", status: "sent", detail: "Old generation" });
  assert.match(session.state.commandStatus, UNSURE);
});

test("sent/refused/uncertain receipts distinguish byte delivery from game outcome", () => {
  const { session, socket } = setup();
  socket.message(readySnapshot());
  // The three must stay DISTINGUISHABLE and must not overstate: `sent` may not
  // claim the game acted, `uncertain` may not read as failure. Matched on the
  // live labels rather than on prose, so a rewording fails loudly here instead
  // of quietly passing a regex that no longer describes the text.
  for (const [status, pattern] of [["sent", /^Sent:/], ["refused", /^Not sent:/], ["uncertain", UNSURE],
    ["handled", /^Done by Hydra:/]]) {
    session.command("look");
    const command = socket.sent.at(-1);
    socket.message({ kind: "receipt", version: 1, session: command.session, generation: command.generation,
      request_id: command.request_id, status, detail: "Synthetic receipt" });
    assert.match(session.state.commandStatus, pattern);
    assert.equal(session.pending.size, 0);
  }
});

test("single-line UTF-8 command bounds prevent control injection without stripping spaces", () => {
  for (const line of ["", "   ", "look\nnorth", "look\r", "look\0", "é".repeat(2049)]) assert.ok(commandError(line));
  for (const line of [" look ", "é".repeat(2048), "x".repeat(4096)]) assert.equal(commandError(line), null);
});

test("Story is bounded and only one manual command can await its receipt", () => {
  const { session, socket } = setup();
  socket.message(readySnapshot());
  const line = fixture().story[0];
  socket.message(update(readySnapshot(), { lines: Array.from({ length: MAX_STORY_LINES + 10 }, () => line) }));
  assert.equal(session.state.story.length, MAX_STORY_LINES);
  assert.equal(session.command("look"), true);
  assert.equal(session.command("look"), false);
  assert.equal(session.pending.size, 1);
  const command = socket.sent.at(-1);
  socket.message({ kind: "receipt", version: 1, session: command.session, generation: command.generation,
    request_id: command.request_id, status: "refused", detail: "Synthetic refusal" });
  assert.equal(session.command("look"), true);
});

test("protocol failure, invalid decimal strings, and update-before-snapshot disable input", () => {
  for (const broken of [update(fixture()), { ...fixture(), version: 2 }, { ...fixture(), cursor: 9007199254740994 },
    { ...fixture(), cursor: "01" }, { ...fixture(), session: "18446744073709551616" },
    { ...fixture(), view: {} }, { ...fixture(), kind: "unexpected" }]) {
    const { session, socket, timers } = setup();
    socket.message(broken);
    assert.equal(session.ready, false);
    assert.equal(session.state.connection, "protocol-error");
    assert.equal(timers.length, 0);
  }
});

test("auth rejection stops retries; closing viewer sends no game command", () => {
  const first = setup();
  first.socket.close(1008);
  assert.equal(first.session.state.connection, "denied");
  assert.equal(first.timers.length, 0);
  const second = setup();
  second.socket.message(readySnapshot());
  second.session.close();
  assert.equal(second.socket.sent.length, 1);
  assert.equal(second.timers.length, 0);
});

test("explicit re-pair revives a refused viewer and fences the old socket", () => {
  const { session, socket } = setup();
  socket.message(readySnapshot());
  session.command("look");
  socket.close(1008);
  session.pair("replacement-token");
  const replacement = session.socket;
  assert.equal(session.ready, false);
  assert.equal(session.state.view, null);
  replacement.open();
  assert.deepEqual(replacement.sent, [{ kind: "authenticate", version: 1, token: "replacement-token" }]);
  socket.message(readySnapshot());
  assert.equal(session.ready, false);
  replacement.message(readySnapshot());
  assert.equal(session.ready, true);
  assert.equal(session.pending.size, 0);
  assert.match(session.state.commandStatus, UNSURE);
  assert.equal(replacement.sent.length, 1);
});

test("re-pair cancels the old reconnect timer and empty fragments do not disconnect", () => {
  const cancelled = [];
  const { session, socket } = setup();
  session.cancel = (id) => cancelled.push(id);
  socket.close(1006);
  const timer = session.timer;
  session.pair("replacement-token");
  assert.deepEqual(cancelled, [timer]);
  assert.equal(session.timer, null);
  const replacement = session.socket;
  session.pair("");
  assert.equal(session.socket, replacement);
});

test("unknown retry attempt is not fabricated and scheduled backoff is not a countdown", () => {
  const { session, socket } = setup();
  const snapshot = fixture();
  snapshot.view.lifecycle.attempt = null;
  socket.message(snapshot);
  assert.equal(session.state.connection, "connected");
  assert.equal(lifecycleText(snapshot.view.lifecycle), "Game reconnecting · retry delay 2.0s — Connection lost");
});

test("policy closure after command submission preserves delivery uncertainty", () => {
  const { session, socket } = setup();
  socket.message(readySnapshot());
  session.command("look");
  socket.close(1008);
  assert.match(session.state.commandStatus, UNSURE);
  assert.match(session.state.commandStatus, /[Aa]ccess refused/);
});

test("browser timers are called without the session as their receiver", () => {
  const session = new HydraSession({ url: "ws://127.0.0.1/ws", token: "synthetic", WebSocketImpl: FakeSocket,
    onChange() {}, schedule() { assert.equal(this, undefined); return 1; },
    cancel() { assert.equal(this, undefined); } });
  session.connect();
  session.socket.close(1006);
  session.close();
});

test("the first snapshot's status is not decided by comparing display text", () => {
  // **A display string was doing double duty as a sentinel.** session.js
  // initialised `commandStatus` to a literal and the first snapshot compared
  // against that same literal to decide "nothing has reported on a command
  // yet". Rewording the message broke the comparison with nothing failing --
  // the status silently stopped updating on the first snapshot.
  const { session, socket } = setup();
  socket.message(readySnapshot());
  assert.match(session.state.commandStatus, /Ready/,
    "a fresh viewer's first snapshot reports readiness");

  // And once something HAS reported on a command, a later snapshot must not
  // overwrite it with the readiness message -- which is the reason the
  // comparison existed at all.
  session.command("look");
  const command = socket.sent.at(-1);
  socket.message({ kind: "receipt", version: 1, session: command.session, generation: command.generation,
    request_id: command.request_id, status: "refused", detail: "Synthetic refusal" });
  assert.match(session.state.commandStatus, /^Not sent:/);
  const later = update(fixture(), { cursor: "99" });
  socket.message(later);
  assert.match(session.state.commandStatus, /^Not sent:/,
    "a later snapshot must not bury a real command result");
});


// --- the closed-window rule, in the viewer ---------------------------------
//
// The server ships each line's DECLARATION (what its stream does when its
// window is closed) because the hub broadcasts one message to all viewers.
// Applying it is the viewer's job, since only the viewer knows what it has
// open. These pin that half.

const closedNone = () => false;

test("a speech duplicate is shown once when no speech window is open", () => {
  // **The author's report, as a unit test.** The game sends the same sentence
  // twice: once inside the `speech` stream and once to main. `speech` declares
  // `ifClosed=''`, which the protocol wiki calls a duplicate -- so a viewer
  // with no speech window shows the main copy and drops the stream copy.
  const streamCopy = { stream: "speech", runs: [], truncated: false, closed: { kind: "drop" } };
  const mainCopy = { stream: "", runs: [], truncated: false, closed: { kind: "main" } };
  assert.equal(placeLine(streamCopy, closedNone).where, "dropped");
  assert.equal(placeLine(mainCopy, closedNone).where, "story");
});

test("an open window takes its stream's lines out of the Story", () => {
  const line = { stream: "speech", runs: [], truncated: false, closed: { kind: "drop" } };
  const place = placeLine(line, (id) => id === "speech");
  assert.deepEqual(place, { where: "window", id: "speech" });
});

test("a styled stream falls through to the Story wearing its style", () => {
  // `thoughts` is declared `styleIfClosed='thought'` and no `ifClosed`: ESP
  // shows inline rather than vanishing when the window is shut.
  const line = { stream: "thoughts", runs: [], truncated: false, closed: { kind: "styled", style: "thought" } };
  assert.deepEqual(placeLine(line, closedNone), { where: "story", style: "thought" });
});

test("a routed stream chains, and a cycle of closed windows does not hang", () => {
  const routed = { stream: "voln", runs: [], truncated: false, closed: { kind: "route", window: "thoughts" } };
  // thoughts is open, so the chain stops there.
  assert.deepEqual(placeLine(routed, (id) => id === "thoughts"), { where: "window", id: "thoughts" });
  // Nothing open: the target's own declaration is not on this line, so the
  // viewer can only fall through to the Story rather than guess.
  assert.equal(placeLine(routed, closedNone).where, "story");
  // A self-cycle must terminate.
  const loop = { stream: "a", runs: [], truncated: false, closed: { kind: "route", window: "a" } };
  assert.equal(placeLine(loop, closedNone).where, "story");
});

test("main-window lines and unknown declarations always reach the Story", () => {
  // The safe direction: text shows rather than disappearing. A missing or
  // unrecognised `closed` must never hide a line.
  for (const line of [
    { stream: "", runs: [], truncated: false, closed: { kind: "main" } },
    { stream: "main", runs: [], truncated: false, closed: { kind: "drop" } },
    { stream: "percWindow", runs: [], truncated: false, closed: { kind: "main" } },
    { stream: "mystery", runs: [], truncated: false },
    { stream: "future", runs: [], truncated: false, closed: { kind: "something_new" } },
  ]) {
    assert.equal(placeLine(line, closedNone).where, "story", `stream: ${line.stream}`);
  }
});

test("a malformed closed declaration is refused but an unknown kind is not", () => {
  // A rejected message does not throw out of `message()`: `onmessage` catches
  // and reports a protocol error, which is what a viewer can act on. Asserting
  // a throw here passed a broken test for the wrong reason at first.
  const { session, socket } = setup();
  const bad = readySnapshot();
  // A `styled` with no style is a broken message.
  bad.story = [{ stream: "thoughts", runs: [], truncated: false, closed: { kind: "styled" } }];
  socket.message(bad);
  assert.equal(session.state.connection, "protocol-error");
  assert.equal(session.state.story.length, 0);

  // But a kind this build has never heard of must NOT break the viewer: a
  // newer server may add one, and refusing the whole message would blank the
  // screen over a line it could simply have shown in the Story.
  const { session: ok, socket: fresh } = setup();
  const future = readySnapshot();
  future.story = [{ stream: "later", runs: [], truncated: false, closed: { kind: "something_new" } }];
  fresh.message(future);
  assert.equal(ok.state.connection, "connected");
  assert.equal(ok.state.story.length, 1);
});


// --- the Story gap notice ----------------------------------------------------

test("the history gap notice clears once the hole can no longer be in the Story", () => {
  // A snapshot says only THAT its Story has a hole. The notice used to stay
  // until the next snapshot, which with no further loss meant forever.
  const { session, socket } = setup();
  const gapped = readySnapshot();
  gapped.history_gap = true;
  gapped.story = Array.from({ length: 5 }, (_, i) => ({ stream: "", runs: [], truncated: false, closed: { kind: "main" } }));
  socket.message(gapped);
  assert.equal(session.state.historyGap, true);
  const line = { stream: "", runs: [], truncated: false, closed: { kind: "main" } };
  // Filling up to the cap evicts nothing: the hole may still be on screen.
  socket.message(update(gapped, { lines: Array.from({ length: MAX_STORY_LINES - 5 }, () => line) }));
  assert.equal(session.state.historyGap, true);
  // Four of the five lines that could precede the hole are gone; one is not.
  socket.message(update(gapped, { cursor: String(BigInt(gapped.cursor) + 2n), lines: Array.from({ length: 4 }, () => line) }));
  assert.equal(session.state.historyGap, true);
  socket.message(update(gapped, { cursor: String(BigInt(gapped.cursor) + 3n), lines: [line] }));
  assert.equal(session.state.historyGap, false, "every line that could precede the hole has been evicted");
});

// --- the page itself, over a minimal DOM -------------------------------------
//
// Just enough of the DOM for `mount`: a tree of nodes with text, children and
// a scroll model (20px per child, 100px tall). Not a browser -- the smoke test
// in browser-tests/ is that -- but enough to hold what the renderer does to
// its nodes, which is where the next two defects lived.

class FakeNode {
  constructor(tag) {
    this.tagName = tag; this.children = []; this.parent = null; this.own = "";
    this.classes = []; this.listeners = {}; this.scrollTop = 0; this.clientHeight = 100; this.hidden = false;
    this.classList = { add: (...names) => this.classes.push(...names) };
  }
  get scrollHeight() { return this.children.length * 20; }
  get firstChild() { return this.children[0] ?? null; }
  get childElementCount() { return this.children.length; }
  get textContent() { return this.own + this.children.map((child) => child.textContent).join(""); }
  set textContent(value) { this.replaceChildren(); this.own = String(value); }
  appendChild(child) { child.remove(); child.parent = this; this.children.push(child); return child; }
  append(...children) { for (const child of children) this.appendChild(child); }
  replaceChildren(...children) {
    for (const child of this.children) child.parent = null;
    this.children = []; this.own = "";
    this.append(...children);
  }
  remove() {
    if (!this.parent) return;
    this.parent.children = this.parent.children.filter((child) => child !== this);
    this.parent = null;
  }
  addEventListener(type, listener) { (this.listeners[type] ??= []).push(listener); }
  fire(type) { for (const listener of this.listeners[type] ?? []) listener({ preventDefault() {} }); }
  focus() {}
}

function page() {
  const nodes = new Map();
  const document = {
    getElementById(id) { if (!nodes.has(id)) nodes.set(id, new FakeNode("div")); return nodes.get(id); },
    createElement(tag) { return new FakeNode(tag); },
  };
  const environment = {
    location: { hash: "#token=synthetic-token", pathname: "/", search: "", protocol: "http:", host: "127.0.0.1:1" },
    history: { replaceState() {} }, WebSocket: FakeSocket, performance: { now: () => 0 },
    setInterval: () => 1, clearInterval() {}, addEventListener() {}, removeEventListener() {},
  };
  const session = mount(document, environment);
  session.socket.open();
  return { session, socket: session.socket, element: (id) => document.getElementById(id) };
}

const said = (text, stream = "") => ({ stream, runs: [{ text, bold: false, monospace: false, preset: null }],
  truncated: false, closed: { kind: stream ? "drop" : "main" } });

test("evicting lines that own no Story node does not take Story paragraphs with them", () => {
  // Only Story-placed lines get a paragraph. The eviction loop removed one
  // node per evicted LINE, so ten evicted speech duplicates -- which own
  // nothing -- took the ten oldest real paragraphs with them.
  const { socket, element } = page();
  const snapshot = readySnapshot();
  snapshot.story = [
    ...Array.from({ length: 10 }, (_, i) => said(`duplicate ${i}`, "speech")),
    ...Array.from({ length: MAX_STORY_LINES - 10 }, (_, i) => said(`kept ${i}`)),
  ];
  socket.message(snapshot);
  const story = element("story-output");
  assert.equal(story.children.length, MAX_STORY_LINES - 10);
  socket.message(update(snapshot, { lines: Array.from({ length: 10 }, (_, i) => said(`new ${i}`)) }));
  assert.equal(story.children.length, MAX_STORY_LINES, "990 retained + 10 new, one paragraph each");
  assert.equal(story.children[0].textContent, "kept 0", "the oldest retained line is still on screen");
});

test("a stream pane keeps the reader's place when its own lines have not changed", () => {
  // Every message -- including each roundtime tick -- rebuilt every pane and
  // pinned it to the bottom, so a pane could not be scrolled back mid-fight.
  const { socket, element } = page();
  const snapshot = readySnapshot();
  snapshot.story = Array.from({ length: 30 }, (_, i) => said(`spoken ${i}`, "speech"));
  socket.message(snapshot);
  const toggle = element("stream-toggles").children[0].children[0];
  toggle.checked = true;
  toggle.fire("change");
  const body = element("stream-panes").children[0].children[1];
  assert.equal(body.children.length, 30);
  assert.equal(body.scrollTop, body.scrollHeight, "a new pane opens at the bottom");
  const nodes = [...body.children];

  body.scrollTop = 0; // the reader scrolls back
  let cursor = BigInt(snapshot.cursor);
  const next = (lines) => { cursor += 1n; socket.message(update(snapshot, { cursor: String(cursor), lines })); };
  next([]);                        // a tick: nothing new anywhere
  next([said("main text")]);       // new Story text, nothing new for speech
  assert.deepEqual(body.children, nodes, "unchanged lines are not rebuilt");
  assert.equal(body.scrollTop, 0, "and the reader's place is kept");

  next([said("spoken 30", "speech")]);
  assert.equal(body.children.length, 31);
  assert.equal(body.scrollTop, 0, "new text while scrolled back does not yank the reader down");

  body.scrollTop = body.scrollHeight; // back at the bottom: follow new text
  next([said("spoken 31", "speech")]);
  assert.equal(body.scrollTop, body.scrollHeight);
});
