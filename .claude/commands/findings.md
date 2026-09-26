---
name: findings
description: List the research findings on disk, or print one in full
argument-hint: "[name]"
tools: Bash, Read
---

# Findings

Agent research in this project is written to `.workflows/findings/` so it survives
the session that produced it. This lists what is there, or prints one.

## What to do

With no argument, list them:

```bash
cd "$(git rev-parse --show-toplevel)" && for f in .workflows/findings/*.md; do
  printf "%-42s %6s lines  " "$(basename "$f")" "$(wc -l < "$f")"
  head -3 "$f" | grep -v '^$' | head -1
done
```

With an argument, read that file in full and summarise what it establishes —
matching on partial name is fine (`/findings crit` finds
`crit-matching-constraints.md`).

## How to report it

These documents carry measured evidence with commands and citations. When summarising,
**keep the evidence attached to the claim** — a number without its command is exactly
the failure mode `plan/05` §−2 exists to prevent.

Say plainly when a finding was later corrected or reversed; several here were.
