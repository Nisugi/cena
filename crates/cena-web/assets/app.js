import { HydraSession, takeLaunchToken } from "./session.js";

// Despana presentation adapted from VellumFE's despana/app.js and app.css.
// Hydra uses its own DTOs, and never interprets game text or presets as HTML/CSS.
const PRESETS = new Map([
  ["roomName", "text-room"], ["roomDesc", "text-description"],
  ["monsterbold", "text-monster"], ["speech", "text-speech"],
  ["whisper", "text-whisper"], ["thought", "text-thought"],
]);

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
  const text = (id, value) => { element(id).textContent = value; };
  const hand = (value) => value?.kind === "empty" ? "Empty" : value?.kind === "holding" ? value.name : "Unknown";

  function renderStory(lines) {
    if (renderedLines === lines) return;
    const atBottom = story.scrollHeight - story.clientHeight - story.scrollTop < 48;
    // Preserve existing nodes (and screen reader position) for ordinary appends.
    const first = lines.indexOf(renderedLines[0]);
    let overlap = first === 0 ? renderedLines.length : 0;
    if (!overlap && renderedLines.length && lines.length) {
      const offset = renderedLines.indexOf(lines[0]);
      if (offset >= 0) {
        for (let i = 0; i < offset; i++) story.firstChild?.remove();
        overlap = renderedLines.length - offset;
      }
    }
    if (!overlap) story.replaceChildren();
    for (const line of lines.slice(overlap)) {
      const node = document.createElement("p");
      node.className = "text-line";
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
    renderStory(state.story);
  }

  const protocol = environment.location.protocol === "https:" ? "wss:" : "ws:";
  const session = new HydraSession({ url: `${protocol}//${environment.location.host}/ws`, token,
    onChange: render, WebSocketImpl: environment.WebSocket });
  element("command-form").addEventListener("submit", (event) => {
    event.preventDefault();
    if (session.command(input.value)) input.value = "";
    if (!input.disabled) input.focus();
  });
  element("story-bottom").addEventListener("click", () => { story.scrollTop = story.scrollHeight; });
  const timer = environment.setInterval(renderRoundtime, 250);
  environment.addEventListener("pagehide", () => {
    environment.clearInterval(timer);
    session.close();
  }, { once: true });
  session.connect();
  return session;
}

if (typeof window !== "undefined" && typeof document !== "undefined") mount(document, window);
