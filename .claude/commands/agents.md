---
name: agents
description: Show what background agents and workflows are doing right now
argument-hint: ""
tools: Bash
---

# Agents

Report the live state of every background workflow in this project, read from the
journals on disk rather than from memory.

## What to do

Run this, and report what it prints:

```bash
python - <<'PY'
import json, io, glob, os, time
root = r"C:/Users/<USER>/.claude/projects/e--Cena"
runs = sorted(glob.glob(root + "/*/subagents/workflows/wf_*"), key=os.path.getmtime)
if not runs:
    print("No workflow runs found.")
    raise SystemExit
now = time.time()
for d in runs[-3:]:
    # A directory's mtime does NOT update when files inside it are written, so
    # it reports the run as far staler than it is. Age must come from the
    # freshest file in the run. (This bug once made a live agent look hung.)
    files = glob.glob(d + "/*.jsonl")
    age = (now - max(os.path.getmtime(f) for f in files)) / 60 if files else 999
    started, finished = [], 0
    for line in io.open(d + "/journal.jsonl", encoding="utf-8", errors="replace"):
        try: e = json.loads(line)
        except: continue
        k = e.get("kind") or e.get("type")
        if k == "started": started.append(e.get("label") or "?")
        elif k == "result": finished += 1
    live = started[finished:] if finished < len(started) else []
    state = "RUNNING" if live else "done"
    print(f"\n{os.path.basename(d)}  [{state}]  last write {age:.1f} min ago")
    print(f"  agents started {len(started)}, finished {finished}")
    # Per-agent mtime and size: a growing file is working, not hung.
    for f in sorted(files, key=os.path.getmtime, reverse=True):
        meta = f.replace(".jsonl", ".meta.json")
        if not os.path.exists(meta): continue
        try: label = json.load(io.open(meta, encoding="utf-8")).get("description", "?")
        except: label = "?"
        mins = (now - os.path.getmtime(f)) / 60
        mark = "  <-- RUNNING" if label in live else ""
        print(f"    {label:24} {os.path.getsize(f)//1024:>6} KB   {mins:5.1f} min ago{mark}")
PY
```

## How to report it

- Say plainly whether anything is still running, and what.
- **Judge liveness by the per-agent line, not the run header.** A running agent's
  `.jsonl` grows as it works; a large file written minutes ago is an agent thinking,
  not an agent hung.
- Genuinely stuck looks like: **no file in the run touched for 15+ minutes** while an
  agent is still listed RUNNING. Then say it is stuck, and offer to kill it.
- Do **not** guess at results that have not landed. If an agent has not finished,
  its findings do not exist yet.
- Before killing anything, check `journal.jsonl` for completed `result` entries —
  finished agents' work can be drained to `.workflows/findings/` and the run resumed
  from cache, rather than re-run from scratch.
