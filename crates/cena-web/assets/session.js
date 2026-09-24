// Hydra v1 browser transport. The native session owns game state and lifetime.
export const MAX_STORY_LINES = 1000;
const decimal = (value) => typeof value === "string" && /^(0|[1-9][0-9]{0,19})$/.test(value)
  && BigInt(value) <= 18446744073709551615n;

// Which session this page is for, from the launch fragment (`#token=…&session=N`).
// Read BEFORE takeLaunchToken, which removes the fragment. `null`: the page
// names none, and the server serves its only session.
export function launchSession(location) {
  const session = new URLSearchParams(location.hash.slice(1)).get("session");
  return decimal(session) ? session : null;
}

export function takeLaunchToken(location, history) {
  const token = new URLSearchParams(location.hash.slice(1)).get("token") || "";
  // Remove all fragment material before opening a socket or touching the view.
  if (location.hash) history.replaceState(null, "", location.pathname + location.search);
  return token;
}

export function commandError(line) {
  if (typeof line !== "string" || !line.trim()) return "Enter a command.";
  if (/[\r\n\0]/.test(line)) return "Commands must be a single line without NUL.";
  if (new TextEncoder().encode(line).length > 4096) return "Command exceeds 4096 bytes.";
  return null;
}

function validRuns(runs) {
  return Array.isArray(runs) && runs.every((run) => run && typeof run.text === "string"
    && typeof run.bold === "boolean" && typeof run.monospace === "boolean"
    && (run.preset === null || typeof run.preset === "string"));
}

// A line's closed-window declaration. Shape only: an unrecognised `kind` is
// ACCEPTED, because a newer server adding a behaviour must not make this viewer
// throw away the whole message -- `placeLine` shows such a line in the Story,
// which is the safe direction. A missing `closed` is likewise tolerated.
function validClosed(closed) {
  if (closed === undefined || closed === null) return true;
  if (typeof closed !== "object" || typeof closed.kind !== "string") return false;
  if (closed.kind === "styled") return typeof closed.style === "string";
  if (closed.kind === "route") return typeof closed.window === "string";
  return true;
}

function validView(view) {
  if (!view || !view.room || !view.vitals || !view.roundtime || !view.lifecycle) return false;
  const room = view.room;
  const nullableText = (v) => v === null || typeof v === "string";
  const nullableNumber = (v) => v === null || (typeof v === "number" && Number.isFinite(v));
  if (!nullableText(room.id) || !nullableText(room.title) || !nullableText(view.prompt)
    || !(room.description === null || validRuns(room.description))) return false;
  if (!(room.exits === null || (Array.isArray(room.exits) && room.exits.every((v) => typeof v === "string")))) return false;
  for (const key of ["creatures", "objects", "players"]) {
    if (!(room[key] === null || (Array.isArray(room[key]) && room[key].every((item) =>
      item && typeof item.text === "string" && nullableText(item.status))))) return false;
  }
  for (const hand of [view.left_hand, view.right_hand]) {
    if (!hand || !["unknown", "empty", "holding"].includes(hand.kind)
      || (hand.kind === "holding" && typeof hand.name !== "string")) return false;
  }
  for (const key of ["health", "mana", "stamina", "spirit"]) {
    const vital = view.vitals[key];
    if (vital !== null && (!vital || !Number.isFinite(vital.percent)
      || !nullableNumber(vital.current) || !nullableNumber(vital.max))) return false;
  }
  if (!nullableNumber(view.roundtime.ends_at) || !nullableNumber(view.roundtime.remaining_seconds)) return false;
  if (!["connecting", "ready", "reconnecting", "closed"].includes(view.lifecycle.kind)) return false;
  if (view.lifecycle.kind === "reconnecting" && (!(view.lifecycle.attempt === null || Number.isSafeInteger(view.lifecycle.attempt))
    || !nullableNumber(view.lifecycle.retry_delay_ms) || !nullableText(view.lifecycle.detail))) return false;
  return Array.isArray(view.unknown_tags) && view.unknown_tags.every((tag) => tag
    && typeof tag.name === "string" && typeof tag.raw === "string" && typeof tag.truncated === "boolean");
}

// A hub card (plan/29 step 5b): one character at a glance.
const LIFECYCLES = ["connecting", "ready", "reconnecting", "closed"];
function validCard(card) {
  return card && decimal(card.session) && typeof card.name === "string"
    && card.lifecycle && LIFECYCLES.includes(card.lifecycle.kind)
    && card.vitals && typeof card.vitals === "object"
    && card.roundtime && typeof card.roundtime === "object"
    && (card.room === null || typeof card.room === "string");
}

export class HydraSession {
  constructor({ url, token, sessionId = null, onChange, WebSocketImpl = WebSocket,
    schedule = setTimeout, cancel = clearTimeout }) {
    this.url = url;
    this.token = token;
    // The session this page is for, or null for the server's only session.
    this.sessionId = sessionId;
    this.onChange = onChange;
    this.WebSocketImpl = WebSocketImpl;
    // Browser timer functions must not be invoked with the session as receiver.
    this.schedule = (...args) => schedule(...args);
    this.cancel = (...args) => cancel(...args);
    this.socket = null;
    this.timer = null;
    this.retryMs = 1000;
    this.stopped = false;
    this.pending = new Map();
    this.nextRequest = 0n;
    // `commandStatus` is display text, and `untouched` is the FACT that
    // nothing has reported on a command yet. They were one thing: the first
    // snapshot compared `commandStatus` against the literal it was initialised
    // with, so rewording the message silently stopped the comparison matching.
    this.untouched = true;
    // An upper bound on where the last reported hole sits in the Story.
    this.linesBeforeGap = 0;
    // `hub`: the character cards, when this page is the hub rather than one
    // character's page; null otherwise.
    // `available`: characters the hub may add; `hubNote`: what became of the
    // last hub request.
    this.state = { connection: "idle", view: null, story: [], session: null,
      generation: null, cursor: null, historyGap: false, commandStatus: "Connecting…", hub: null,
      available: [], hubNote: "" };
  }

  get ready() {
    return this.state.connection === "connected" && this.state.view?.lifecycle.kind === "ready"
      && this.socket?.readyState === 1 && this.pending.size === 0;
  }

  emit() { this.onChange(this.state, this.ready); }

  // Explicit operator re-pair, including after refresh or an auth refusal.
  // close() abandons pending receipts honestly and detaches old socket callbacks;
  // connect() requires a new snapshot before any command can be submitted.
  pair(token) {
    if (!token) return;
    if (["idle", "unpaired"].includes(this.state.connection)) {
      this.state.commandStatus = "Connecting…";
    }
    this.close();
    this.token = token;
    this.stopped = false;
    this.retryMs = 1000;
    this.connect();
  }

  connect() {
    if (this.stopped || this.socket) return;
    if (!this.token) {
      this.state.connection = "unpaired";
      this.state.commandStatus = "Open the pairing link Hydra printed to connect this window.";
      this.emit();
      return;
    }
    this.state.connection = "connecting";
    this.state.view = null;
    this.state.cursor = null;
    this.state.session = null;
    this.state.generation = null;
    this.state.hub = null;
    this.emit();
    let socket;
    try { socket = new this.WebSocketImpl(this.url); }
    catch { this.disconnected(1006); return; }
    this.socket = socket;
    socket.onopen = () => {
      if (this.socket !== socket) return;
      this.state.connection = "authenticating";
      const hello = { kind: "authenticate", version: 1, token: this.token };
      if (this.sessionId !== null) hello.session = this.sessionId;
      socket.send(JSON.stringify(hello));
      this.emit();
    };
    socket.onmessage = (event) => {
      if (this.socket !== socket) return;
      try { this.receive(JSON.parse(event.data)); }
      catch {
        this.state.commandStatus = "Did not understand the game connection. Reopen Hydra's pairing link.";
        this.close("protocol-error");
      }
    };
    socket.onclose = (event) => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.disconnected(event.code);
    };
    socket.onerror = () => socket.close();
  }

  uncertain() {
    if (!this.pending.size) return;
    const n = this.pending.size;
    this.untouched = false;
    this.state.commandStatus = `Lost track of ${n === 1 ? "your last command" : `your last ${n} commands`} — ${n === 1 ? "it" : "they"} may or may not have gone through. Nothing was re-sent.`;
    this.pending.clear();
  }

  disconnected(code) {
    const hadPending = this.pending.size > 0;
    this.uncertain();
    this.state.view = null;
    this.state.connection = code === 1008 ? "denied" : "reconnecting";
    if (code === 1008) {
      this.state.commandStatus = (hadPending ? this.state.commandStatus + " " : "")
        + "Access refused. Open a fresh pairing link from Hydra.";
      this.untouched = false;
      this.stopped = true;
    } else if (!this.stopped) {
      const delay = this.retryMs;
      this.retryMs = Math.min(10000, delay * 2);
      this.timer = this.schedule(() => { this.timer = null; this.connect(); }, delay);
    }
    this.emit();
  }

  receive(message) {
    // The hub page: every character's card, replacing the last list whole.
    if (message && message.kind === "sessions") {
      if (message.version !== 1 || !Array.isArray(message.sessions) || !message.sessions.every(validCard)
        || !Array.isArray(message.available) || !message.available.every((name) => typeof name === "string")) {
        throw new Error("Invalid session list");
      }
      this.state.hub = message.sessions;
      this.state.available = message.available;
      this.state.connection = "hub";
      this.state.commandStatus = "Choose a character to play.";
      this.emit();
      return;
    }
    if (message && message.kind === "hub_note") {
      if (message.version !== 1 || typeof message.detail !== "string") throw new Error("Invalid hub note");
      this.state.hubNote = message.detail;
      this.emit();
      return;
    }
    if (!message || message.version !== 1 || !decimal(message.session) || !decimal(message.generation)) {
      throw new Error("Invalid envelope");
    }
    const state = this.state;
    if (message.kind === "receipt") {
      if (state.connection !== "connected") throw new Error("Receipt before snapshot");
      if (message.session !== state.session || message.generation !== state.generation) return;
      if (!["sent", "refused", "uncertain", "handled"].includes(message.status) || typeof message.detail !== "string") {
        throw new Error("Invalid receipt");
      }
      if (!this.pending.delete(message.request_id)) return;
      // Plain wording, same claims. `sent` must not imply the game ACTED --
      // the bytes reached the wire and the outcome is whatever the story
      // shows. `uncertain` must not imply failure: it may well have run.
      // `handled` is a `;` command Hydra ran itself: nothing went to the game.
      const label = { sent: "Sent", refused: "Not sent", uncertain: "Not sure if this one went through",
        handled: "Done by Hydra" };
      this.untouched = false;
      state.commandStatus = `${label[message.status]}: ${message.detail}`;
      this.emit();
      return;
    }
    if (!["snapshot", "update"].includes(message.kind) || !decimal(message.cursor) || !validView(message.view)) {
      throw new Error("Invalid state");
    }
    if (state.cursor === null && message.kind !== "snapshot") throw new Error("Snapshot required");
    if (state.session !== null && message.session !== state.session) return;
    if (state.generation !== null && BigInt(message.generation) < BigInt(state.generation)) return;
    if (state.cursor !== null && BigInt(message.cursor) <= BigInt(state.cursor)) return;
    const lines = message.kind === "snapshot" ? message.story : message.lines;
    if (!Array.isArray(lines) || !lines.every((line) => line && typeof line.stream === "string"
      && typeof line.truncated === "boolean" && validRuns(line.runs)
      && validClosed(line.closed))) throw new Error("Invalid Story");
    if (message.kind === "snapshot" && typeof message.history_gap !== "boolean") throw new Error("Invalid gap");
    if (state.generation !== null && message.generation !== state.generation) this.uncertain();
    // How many lines leave the front of the Story with this message.
    const evicted = message.kind === "snapshot" ? 0
      : Math.max(0, state.story.length + lines.length - MAX_STORY_LINES);
    state.story = (message.kind === "snapshot" ? lines : [...state.story, ...lines]).slice(-MAX_STORY_LINES);
    // **The gap notice clears once the hole cannot still be on screen.** A
    // snapshot says only THAT its Story has a hole, not where; the hole is
    // somewhere among its lines, so it has certainly scrolled out of memory
    // once that many lines have been evicted since. The notice used to stay
    // until the next snapshot -- which, with no further loss, meant forever.
    if (message.kind === "snapshot") {
      state.historyGap = message.history_gap;
      this.linesBeforeGap = message.history_gap ? lines.length : 0;
    } else if (state.historyGap && evicted > 0) {
      this.linesBeforeGap -= evicted;
      if (this.linesBeforeGap <= 0) state.historyGap = false;
    }
    state.view = message.view;
    state.session = message.session;
    state.generation = message.generation;
    state.cursor = message.cursor;
    state.connection = "connected";
    if (this.untouched) {
      state.commandStatus = "Ready when the game is.";
    }
    this.retryMs = 1000;
    this.emit();
  }

  command(line) {
    const error = commandError(line);
    if (error || !this.ready) {
      this.untouched = false;
      this.state.commandStatus = error || "Not ready yet — waiting on the game, or on your last command.";
      this.emit();
      return false;
    }
    const request_id = String(++this.nextRequest);
    this.pending.set(request_id, true);
    try {
      this.socket.send(JSON.stringify({ kind: "command", version: 1, session: this.state.session,
        generation: this.state.generation, request_id, line }));
      this.untouched = false;
      this.state.commandStatus = "Sending…";
    } catch {
      this.uncertain();
      this.socket.close();
    }
    this.emit();
    return true;
  }

  // Hub requests (plan/29 step 5c). The hub starts only characters that
  // have logged in before; no credential is ever sent from here.
  addCharacter(character) {
    if (this.state.hub === null || this.socket?.readyState !== 1) return false;
    this.state.hubNote = `Asking to start ${character}…`;
    this.socket.send(JSON.stringify({ kind: "add_character", version: 1, character }));
    this.emit();
    return true;
  }

  removeSession(session) {
    if (this.state.hub === null || this.socket?.readyState !== 1 || !decimal(session)) return false;
    this.state.hubNote = "Asking the character to quit…";
    this.socket.send(JSON.stringify({ kind: "remove_session", version: 1, session }));
    this.emit();
    return true;
  }

  // This only closes the viewer socket. No game command or session-close message exists.
  close(connection = "closed") {
    this.stopped = true;
    if (this.timer !== null) this.cancel(this.timer);
    this.timer = null;
    this.uncertain();
    const socket = this.socket;
    this.socket = null;
    socket?.close();
    this.state.connection = connection;
    this.state.view = null;
    this.token = "";
    this.emit();
  }
}
