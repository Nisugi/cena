import { HydraSession, takeLaunchToken } from "./session.js";

// Despana presentation adapted from VellumFE's despana/app.js and app.css.
// Hydra uses its own DTOs, and never interprets game text or presets as HTML/CSS.
const PRESETS = new Map([
  ["roomName", "text-room"], ["roomDesc", "text-description"],
  ["monsterbold", "text-monster"], ["speech", "text-speech"],
  ["whisper", "text-whisper"], ["thought", "text-thought"],
]);

// Which stream windows this viewer has open. Per-viewer by nature -- the server
// broadcasts one message to every viewer, so it ships each line's DECLARATION
// (`closed`) and the decision is made here.
//
// Nothing is open by default: Despana has one Story pane, so the wire's
// fall-through behaviours are exactly what a new viewer wants.
export function placeLine(line, isOpen, seen = new Set()) {
  const stream = line.stream;
  if (!stream || stream === "main") return { where: "story" };
  let at = stream;
  for (;;) {
    if (isOpen(at)) return { where: "window", id: at };
    if (seen.has(at)) return { where: "story" };   // a cycle of closed windows
    seen.add(at);
    const closed = at === stream ? line.closed : null;
    // Only the line's own stream carries a declaration. A `route` target's
    // behaviour is unknown here, so an open check is all we can do for it.
    if (!closed) return { where: "story" };
    if (closed.kind === "drop") return { where: "dropped" };
    if (closed.kind === "styled") return { where: "story", style: closed.style };
    if (closed.kind === "route") { at = closed.window; continue; }
    return { where: "story" };
  }
}

export function appendRuns(document, target, runs) {
  for (const run of runs) {
    const span = document.createElement("span");
    span.textContent = run.text;
    if (run.bold) span.classList.add("text-bold");
    if (run.monospace) span.classList.add("text-mono");
    if (PRESETS.has(run.preset)) span.classList.add(PRESETS.get(run.preset));
    else if (run.preset) span.title = `Unmapped text preset: ${run.preset}`;
    target.appendChild(span);
  }
}

export function lifecycleText(lifecycle) {
  if (!lifecycle) return "Waiting for game state";
  if (lifecycle.kind === "ready") return "Ready";
  if (lifecycle.kind === "connecting") return "Game connecting";
  if (lifecycle.kind === "closed") return `Game closed${lifecycle.detail ? ` — ${lifecycle.detail}` : ""}`;
  const delay = lifecycle.retry_delay_ms === null ? "" : ` · retry delay ${(lifecycle.retry_delay_ms / 1000).toFixed(1)}s`;
  const attempt = lifecycle.attempt > 0 ? ` · attempt ${lifecycle.attempt}` : "";
  return `Game reconnecting${attempt}${delay}${lifecycle.detail ? ` — ${lifecycle.detail}` : ""}`;
}

export function mount(document, environment) {
  const token = takeLaunchToken(environment.location, environment.history);
  const element = (id) => document.getElementById(id);
  const input = element("command-input");
  const story = element("story-output");
  let renderedLines = [];
  let renderedGeneration = null;
  let currentView = null;
  let roundtimeReceivedAt = 0;
  // Streams this viewer has opened a window for. Per-viewer and local: the
  // server cannot know it, and it is a display preference rather than game
  // state, so it is not sent anywhere.
  const openWindows = new Set();
  let currentStory = [];
  const isOpen = (id) => openWindows.has(id);
  // Streams seen so far, so a toggle only appears once the game has used one.
  // Offering all 16 declared ids up front would list windows this character
  // never fills (MEASURED: 16 declared against 6 ever pushed to).
  const knownStreams = new Map();
  const text = (id, value) => { element(id).textContent = value; };
  const hand = (value) => value?.kind === "empty" ? "Empty" : value?.kind === "holding" ? value.name : "Unknown";

  function renderStory(lines, force = false) {
    // A toggle changes where lines GO without changing the lines, so the
    // identity short-circuit has to be bypassed or the screen would not move.
    if (renderedLines === lines && !force) return;
    if (force) { renderedLines = []; story.replaceChildren(); }
    const atBottom = story.scrollHeight - story.clientHeight - story.scrollTop < 48;
    // Preserve existing nodes (and screen reader position) for ordinary appends.
    const first = lines.indexOf(renderedLines[0]);
    let overlap = first === 0 ? renderedLines.length : 0;
    if (!overlap && renderedLines.length && lines.length) {
      const offset = renderedLines.indexOf(lines[0]);
      if (offset >= 0) {
        // Remove one node per evicted line THAT HAD ONE. Only Story-placed
        // lines own a paragraph -- a dropped duplicate or a line shown in its
        // own window owns nothing here -- so counting every evicted line took
        // still-retained Story paragraphs off the top along with them.
        const owned = renderedLines.slice(0, offset)
          .filter((line) => placeLine(line, isOpen).where === "story").length;
        for (let i = 0; i < owned; i++) story.firstChild?.remove();
        overlap = renderedLines.length - offset;
      }
    }
    if (!overlap) story.replaceChildren();
    for (const line of lines.slice(overlap)) {
      const place = placeLine(line, isOpen);
      // A duplicate the game already sent to main, or text showing in its own
      // window: either way it is not a Story line. THIS is what stops every
      // spoken line appearing twice.
      if (place.where !== "story") continue;
      const node = document.createElement("p");
      node.className = "text-line";
      if (place.style) node.classList.add(PRESETS.get(place.style) ?? "text-stream-styled");
      if (line.stream !== "") {
        const label = document.createElement("span");
        label.className = "stream-label";
        label.textContent = `[${line.stream}] `;
        node.appendChild(label);
      }
      appendRuns(document, node, line.runs);
      if (line.truncated) {
        const marker = document.createElement("span");
        marker.className = "output-gap";
        marker.textContent = " [line truncated]";
        node.appendChild(marker);
      }
      story.appendChild(node);
    }
    if (!lines.length) {
      const empty = document.createElement("p");
      empty.className = "empty-state";
      empty.textContent = "Waiting for game text…";
      story.appendChild(empty);
    }
    renderedLines = lines;
    if (atBottom) story.scrollTop = story.scrollHeight;
  }

  // One checkbox per stream the game has actually used. Ticking it opens a
  // window: that stream's lines leave the Story and show in their own pane.
  function renderStreamToggles(lines) {
    let added = false;
    for (const line of lines) {
      if (!line.stream || line.stream === "main" || knownStreams.has(line.stream)) continue;
      knownStreams.set(line.stream, line.closed?.kind ?? "main");
      added = true;
    }
    if (!added) return;
    const host = element("stream-toggles");
    host.replaceChildren();
    for (const [stream, kind] of [...knownStreams].sort(([a], [b]) => a.localeCompare(b))) {
      const label = document.createElement("label");
      label.className = "stream-toggle";
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = openWindows.has(stream);
      box.addEventListener("change", () => {
        if (box.checked) openWindows.add(stream); else openWindows.delete(stream);
        renderStreams();
        renderStory(currentStory, true);
      });
      const name = document.createElement("span");
      name.textContent = stream;
      label.append(box, name);
      // `drop` streams are the duplicated ones. Say so, because closing such a
      // window loses nothing -- the same text is in the Story already.
      if (kind === "drop") {
        const note = document.createElement("span");
        note.className = "stream-note-inline";
        note.textContent = "also in Story";
        label.appendChild(note);
      }
      host.appendChild(label);
    }
    element("stream-windows").hidden = knownStreams.size === 0;
  }

  // A pane per open window, holding that stream's lines: stream -> the lines
  // it last rendered (null before its first) and the body they are in.
  const panes = new Map();

  // **Rebuild a pane only when its own lines changed, and keep the reader's
  // place.** This used to rebuild every pane on every message and pin each to
  // the bottom. Every message includes a roundtime tick, so during combat the
  // panes were rebuilt and snapped ten times a second and could not be
  // scrolled back at all. A pane now keeps its DOM when its lines are the same
  // objects as last time, and on a real change snaps to the bottom only if the
  // reader was already there.
  function renderStreams() {
    const host = element("stream-panes");
    const streams = [...openWindows].sort((a, b) => a.localeCompare(b));
    if (streams.join("\n") !== [...panes.keys()].join("\n")) {
      // Windows opened or closed. Panes still open keep their node.
      for (const stream of [...panes.keys()]) if (!openWindows.has(stream)) panes.delete(stream);
      for (const stream of streams) {
        if (panes.has(stream)) continue;
        const section = document.createElement("section");
        section.className = "stream-pane";
        const heading = document.createElement("h3");
        heading.textContent = stream;
        const body = document.createElement("div");
        body.className = "text-output stream-body";
        section.append(heading, body);
        panes.set(stream, { lines: null, body, section });
      }
      const ordered = new Map(streams.map((stream) => [stream, panes.get(stream)]));
      panes.clear();
      for (const [stream, pane] of ordered) panes.set(stream, pane);
      host.replaceChildren(...[...panes.values()].map((pane) => pane.section));
    }
    for (const [stream, pane] of panes) {
      const lines = currentStory.filter((line) => {
        const place = placeLine(line, isOpen);
        return place.where === "window" && place.id === stream;
      });
      if (pane.lines && pane.lines.length === lines.length
        && pane.lines.every((line, index) => line === lines[index])) continue;
      const body = pane.body;
      const atBottom = pane.lines === null
        || body.scrollHeight - body.clientHeight - body.scrollTop < 48;
      const top = body.scrollTop;
      body.replaceChildren();
      for (const line of lines) {
        const node = document.createElement("p");
        node.className = "text-line";
        appendRuns(document, node, line.runs);
        body.appendChild(node);
      }
      if (!lines.length) {
        const empty = document.createElement("p");
        empty.className = "empty-state";
        empty.textContent = "Nothing yet.";
        body.appendChild(empty);
      }
      pane.lines = lines;
      body.scrollTop = atBottom ? body.scrollHeight : top;
    }
    host.hidden = openWindows.size === 0;
  }

  function renderRoundtime() {
    const remaining = currentView?.roundtime.remaining_seconds;
    text("roundtime", remaining == null ? "Unknown"
      : `${Math.ceil(Math.max(0, remaining - (environment.performance.now() - roundtimeReceivedAt) / 1000))}s`);
  }

  function render(state, ready) {
    const view = state.view;
    if (currentView !== view) roundtimeReceivedAt = environment.performance.now();
    currentView = view;
    text("connection-status", state.connection === "connected" ? lifecycleText(view.lifecycle)
      : ({ idle: "Viewer idle", connecting: "Connecting viewer…", authenticating: "Authenticating viewer…",
        reconnecting: "Viewer disconnected · reconnecting…", unpaired: "Pairing required", denied: "Pairing refused",
        closed: "Viewer closed", "protocol-error": "Unsupported server message" }[state.connection]));
    text("session-identity", state.session === null ? "Waiting for session" : `Session ${state.session} · generation ${state.generation}`);
    text("hands-left", hand(view?.left_hand));
    text("hands-right", hand(view?.right_hand));
    text("room-title", view?.room.title ?? "Room unknown");
    text("room-id", view?.room.id === null || !view ? "" : `#${view.room.id}`);
    const description = element("room-description");
    description.replaceChildren();
    if (view?.room.description != null) appendRuns(document, description, view.room.description);
    else description.textContent = "Description unknown";
    text("room-exits", view?.room.exits?.join(" · ") || (view?.room.exits ? "None" : "Unknown"));
    for (const kind of ["creatures", "objects", "players"]) {
      const items = view?.room[kind];
      text(`room-${kind}`, items?.map((item) => item.text + (item.status ? ` (${item.status})` : "")).join(", ")
        || (items ? "None" : "Unknown"));
    }
    const unknown = view?.unknown_tags || [];
    text("diagnostics-label", `Protocol diagnostics (${unknown.length})`);
    const tags = element("unknown-tags");
    tags.replaceChildren();
    for (const tag of unknown) {
      const node = document.createElement("p");
      node.textContent = `${tag.name}: ${tag.raw}${tag.truncated ? " [truncated]" : ""}`;
      tags.appendChild(node);
    }
    element("diagnostics").hidden = unknown.length === 0;
    for (const key of ["health", "mana", "stamina", "spirit"]) {
      const vital = view?.vitals[key];
      const meter = element(`${key}-meter`);
      meter.hidden = vital == null;
      if (vital != null) meter.value = Math.max(0, Math.min(100, vital.percent));
      text(`${key}-value`, vital == null ? "Unknown" : vital.current !== null && vital.max !== null
        ? `${vital.current} / ${vital.max} · ${vital.percent}%` : `${vital.percent}%`);
    }
    renderRoundtime();
    text("prompt", view?.prompt ?? ">");
    text("command-status", state.commandStatus);
    element("history-gap").hidden = !state.historyGap;
    // Keep keyboard focus while a receipt is pending; disabling a focused input
    // blurs it in browsers and would require a click before every next command.
    input.readOnly = state.connection === "connected" && view?.lifecycle.kind === "ready"
      && session.pending.size > 0;
    input.disabled = !ready && !input.readOnly;
    element("command-send").disabled = !ready;
    // Drafts have no outbox; do not carry a draft into a different game generation.
    if (renderedGeneration !== null && state.generation !== null && renderedGeneration !== state.generation) input.value = "";
    if (state.generation !== null) renderedGeneration = state.generation;
    currentStory = state.story;
    renderStreamToggles(state.story);
    renderStory(state.story);
    renderStreams();
  }

  const protocol = environment.location.protocol === "https:" ? "wss:" : "ws:";
  const session = new HydraSession({ url: `${protocol}//${environment.location.host}/ws`, token,
    onChange: render, WebSocketImpl: environment.WebSocket });
  const pairFromFragment = () => {
    // A pairing URL opened in this same tab changes only the fragment: mount
    // does not run again. Strip it before rendering or opening another socket.
    const nextToken = takeLaunchToken(environment.location, environment.history);
    if (!nextToken) return;
    input.value = "";
    session.pair(nextToken);
  };
  environment.addEventListener("hashchange", pairFromFragment);
  element("command-form").addEventListener("submit", (event) => {
    event.preventDefault();
    if (session.command(input.value)) input.value = "";
    if (!input.disabled) input.focus();
  });
  element("story-bottom").addEventListener("click", () => { story.scrollTop = story.scrollHeight; });
  const timer = environment.setInterval(renderRoundtime, 250);
  environment.addEventListener("pagehide", () => {
    environment.removeEventListener("hashchange", pairFromFragment);
    environment.clearInterval(timer);
    session.close();
  }, { once: true });
  session.connect();
  return session;
}

if (typeof window !== "undefined" && typeof document !== "undefined") mount(document, window);
