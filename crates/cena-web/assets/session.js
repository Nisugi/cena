// Hydra v1 browser transport. The native session owns game state and lifetime.
export const MAX_STORY_LINES = 1000;
const decimal = (value) => typeof value === "string" && /^(0|[1-9][0-9]{0,19})$/.test(value)
  && BigInt(value) <= 18446744073709551615n;

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

export class HydraSession {
  constructor({ url, token, onChange, WebSocketImpl = WebSocket,
    schedule = setTimeout, cancel = clearTimeout }) {
    this.url = url;
    this.token = token;
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
    this.state = { connection: "idle", view: null, story: [], session: null,
      generation: null, cursor: null, historyGap: false, commandStatus: "Waiting for connection." };
  }

  get ready() {
    return this.state.connection === "connected" && this.state.view?.lifecycle.kind === "ready"
      && this.socket?.readyState === 1 && this.pending.size === 0;
  }

  emit() { this.onChange(this.state, this.ready); }

  connect() {
    if (this.stopped || this.socket) return;
    if (!this.token) {
      this.state.connection = "unpaired";
      this.state.commandStatus = "Open the pairing link printed by Hydra to connect this viewer.";
      this.emit();
      return;
    }
    this.state.connection = "connecting";
    this.state.view = null;
    this.state.cursor = null;
    this.state.session = null;
    this.state.generation = null;
    this.emit();
    let socket;
    try { socket = new this.WebSocketImpl(this.url); }
    catch { this.disconnected(1006); return; }
    this.socket = socket;
    socket.onopen = () => {
      if (this.socket !== socket) return;
      this.state.connection = "authenticating";
      socket.send(JSON.stringify({ kind: "authenticate", version: 1, token: this.token }));
      this.emit();
    };
    socket.onmessage = (event) => {
      if (this.socket !== socket) return;
      try { this.receive(JSON.parse(event.data)); }
      catch {
        this.state.commandStatus = "Unsupported or malformed server message. Reopen Hydra's pairing link.";
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
    this.state.commandStatus = `${this.pending.size} command receipt(s) lost; delivery is uncertain. Nothing will be resent.`;
    this.pending.clear();
  }

  disconnected(code) {
    const hadPending = this.pending.size > 0;
    this.uncertain();
    this.state.view = null;
    this.state.connection = code === 1008 ? "denied" : "reconnecting";
    if (code === 1008) {
      this.state.commandStatus = (hadPending ? this.state.commandStatus + " " : "")
        + "Viewer access refused. Open a fresh pairing link from Hydra.";
      this.stopped = true;
    } else if (!this.stopped) {
      const delay = this.retryMs;
      this.retryMs = Math.min(10000, delay * 2);
      this.timer = this.schedule(() => { this.timer = null; this.connect(); }, delay);
    }
    this.emit();
  }

  receive(message) {
    if (!message || message.version !== 1 || !decimal(message.session) || !decimal(message.generation)) {
      throw new Error("Invalid envelope");
    }
    const state = this.state;
    if (message.kind === "receipt") {
      if (state.connection !== "connected") throw new Error("Receipt before snapshot");
      if (message.session !== state.session || message.generation !== state.generation) return;
      if (!["sent", "refused", "uncertain"].includes(message.status) || typeof message.detail !== "string") {
        throw new Error("Invalid receipt");
      }
      if (!this.pending.delete(message.request_id)) return;
      const label = { sent: "Bytes sent (game outcome unconfirmed)", refused: "Command refused", uncertain: "Delivery uncertain; do not assume it failed" };
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
      && typeof line.truncated === "boolean" && validRuns(line.runs))) throw new Error("Invalid Story");
    if (message.kind === "snapshot" && typeof message.history_gap !== "boolean") throw new Error("Invalid gap");
    if (state.generation !== null && message.generation !== state.generation) this.uncertain();
    state.story = (message.kind === "snapshot" ? lines : [...state.story, ...lines]).slice(-MAX_STORY_LINES);
    if (message.kind === "snapshot") state.historyGap = message.history_gap;
    state.view = message.view;
    state.session = message.session;
    state.generation = message.generation;
    state.cursor = message.cursor;
    state.connection = "connected";
    if (state.commandStatus === "Waiting for connection.") {
      state.commandStatus = "Manual commands are available when the game session is ready.";
    }
    this.retryMs = 1000;
    this.emit();
  }

  command(line) {
    const error = commandError(line);
    if (error || !this.ready) {
      this.state.commandStatus = error || "Wait for a ready session and pending command receipts.";
      this.emit();
      return false;
    }
    const request_id = String(++this.nextRequest);
    this.pending.set(request_id, true);
    try {
      this.socket.send(JSON.stringify({ kind: "command", version: 1, session: this.state.session,
        generation: this.state.generation, request_id, line }));
      this.state.commandStatus = "Command submitted; waiting for a delivery receipt.";
    } catch {
      this.uncertain();
      this.socket.close();
    }
    this.emit();
    return true;
  }

  // This only closes the viewer socket. No game command or session-close message exists.
  close(connection = "closed") {
    this.stopped = true;
    if (this.timer !== null) this.cancel(this.timer);
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
