import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { HydraSession, MAX_STORY_LINES, commandError, takeLaunchToken } from "../session.js";
import { lifecycleText } from "../app.js";

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

test("launch token is removed before use and query strings never supply authentication", () => {
  const writes = [];
  const token = takeLaunchToken({ hash: "#token=synthetic-token", pathname: "/", search: "" },
    { replaceState(...args) { writes.push(args); } });
  assert.equal(token, "synthetic-token");
  assert.deepEqual(writes, [[null, "", "/"]]);
  assert.equal(takeLaunchToken({ hash: "", pathname: "/", search: "?token=ignored" }, {}), "");
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
  assert.match(session.state.commandStatus, /uncertain/);
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
  assert.match(session.state.commandStatus, /uncertain/);
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
  assert.match(session.state.commandStatus, /uncertain/);
  socket.message({ kind: "receipt", version: 1, session: fixture().session, generation: fixture().generation,
    request_id: "1", status: "sent", detail: "Old generation" });
  assert.match(session.state.commandStatus, /uncertain/);
});

test("sent/refused/uncertain receipts distinguish byte delivery from game outcome", () => {
  const { session, socket } = setup();
  socket.message(readySnapshot());
  for (const [status, pattern] of [["sent", /game outcome unconfirmed/], ["refused", /Command refused/], ["uncertain", /Delivery uncertain/]]) {
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
  assert.match(session.state.commandStatus, /uncertain/);
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
  assert.match(session.state.commandStatus, /delivery is uncertain/);
  assert.match(session.state.commandStatus, /access refused/);
});

test("browser timers are called without the session as their receiver", () => {
  const session = new HydraSession({ url: "ws://127.0.0.1/ws", token: "synthetic", WebSocketImpl: FakeSocket,
    onChange() {}, schedule() { assert.equal(this, undefined); return 1; },
    cancel() { assert.equal(this, undefined); } });
  session.connect();
  session.socket.close(1006);
  session.close();
});
