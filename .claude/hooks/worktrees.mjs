// Worktrees and build folders are allowed; whoever makes one removes it
// (the author, 2026-09-25: "they can do what they want but they need to clean
// up after themselves"). CLAUDE.md, Commands, states the rule; this enforces it.
//
//   start  (SessionStart)  record what already exists, and report it
//   stop   (Stop)          block once if something made since is still there
//
// A worktree locked with `git worktree lock` is kept on purpose and never
// flagged. The record lives in the main repository's .git directory, one file
// per session, and is kept 14 days.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const KEEP_DAYS = 14;

function readInput() {
  const text = fs.readFileSync(0, "utf8").trim();
  return text ? JSON.parse(text) : {};
}

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

/** Every unlocked worktree but the main one, and every `target?*` folder at the main root. */
function leftovers(cwd) {
  const blocks = git(cwd, "worktree", "list", "--porcelain")
    .split(/\n\s*\n/)
    .map((b) => b.split("\n"));
  const mainPath = blocks[0][0].slice("worktree ".length);
  const found = [];
  for (const lines of blocks.slice(1)) {
    if (lines.some((l) => l.startsWith("locked"))) continue;
    const where = lines[0].slice("worktree ".length);
    const branch = lines.find((l) => l.startsWith("branch "));
    const state = branch ? branch.slice("branch refs/heads/".length) : "detached";
    const gone = lines.some((l) => l.startsWith("prunable")) ? ", folder gone: git worktree prune" : "";
    found.push({ key: where, text: `worktree ${where} (${state}${gone})` });
  }
  for (const entry of fs.readdirSync(mainPath, { withFileTypes: true })) {
    if (entry.isDirectory() && /^target./.test(entry.name)) {
      const where = `${mainPath}/${entry.name}`;
      found.push({ key: where, text: `build folder ${where}` });
    }
  }
  return found;
}

function recordFile(cwd, sessionId) {
  const common = git(cwd, "rev-parse", "--path-format=absolute", "--git-common-dir");
  const dir = path.join(common, "claude-hooks");
  fs.mkdirSync(dir, { recursive: true });
  return { dir, file: path.join(dir, `${sessionId}.json`) };
}

function forgetOldRecords(dir) {
  const cutoff = Date.now() - KEEP_DAYS * 24 * 60 * 60 * 1000;
  for (const name of fs.readdirSync(dir)) {
    const file = path.join(dir, name);
    if (fs.statSync(file).mtimeMs < cutoff) fs.rmSync(file);
  }
}

function list(items) {
  return items.map((i) => `- ${i.text}`).join("\n");
}

function start(input, cwd) {
  const found = leftovers(cwd);
  const { dir, file } = recordFile(cwd, input.session_id ?? "unknown");
  forgetOldRecords(dir);
  // A compact or resume keeps the session id; the first record stands.
  if (!fs.existsSync(file)) fs.writeFileSync(file, JSON.stringify(found.map((i) => i.key)));
  if (found.length === 0) return;
  const out = {
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext:
        "Worktrees and build folders left from earlier sessions. They are not this " +
        "session's to remove: ask the author before touching any.\n" + list(found),
    },
  };
  if (input.source === "startup") {
    out.systemMessage = `${found.length} worktree(s) or build folder(s) left from earlier sessions.`;
  }
  process.stdout.write(JSON.stringify(out));
}

function stop(input, cwd) {
  if (input.stop_hook_active) return;
  const found = leftovers(cwd);
  const { file } = recordFile(cwd, input.session_id ?? "unknown");
  if (!fs.existsSync(file)) {
    // The hook arrived mid-session: what exists now is the baseline.
    fs.writeFileSync(file, JSON.stringify(found.map((i) => i.key)));
    return;
  }
  const before = new Set(JSON.parse(fs.readFileSync(file, "utf8")));
  const made = found.filter((i) => !before.has(i.key));
  if (made.length === 0) return;
  const reason =
    "Made during this session and still there:\n" + list(made) + "\n\n" +
    "Clean up after yourself (CLAUDE.md, Commands): take or discard the work, then " +
    "`git worktree remove <path>`, which deletes its target/ too and refuses while there " +
    "is uncommitted work (do not --force past that without the author). Delete a merged " +
    "local branch with `git branch -d`; never a remote one. Remove a target-* folder you " +
    "made. If one is still in use by a running agent, or the author asked to keep it, " +
    "say so and stop; `git worktree lock <path>` marks a worktree as kept.";
  process.stdout.write(JSON.stringify({ decision: "block", reason }));
}

try {
  const input = readInput();
  const cwd = input.cwd || process.cwd();
  const mode = process.argv[2];
  if (mode === "start") start(input, cwd);
  else if (mode === "stop") stop(input, cwd);
  else throw new Error(`unknown mode ${JSON.stringify(mode)}; expected start or stop`);
} catch (err) {
  process.stderr.write(`worktrees hook: ${err.message}\n`);
  process.exit(1);
}
