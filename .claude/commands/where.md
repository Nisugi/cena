---
name: where
description: Where the project stands — milestone, last commit, uncommitted work, what is next
argument-hint: ""
tools: Bash, Read
---

# Where

Orientation after a break or a compaction. Facts from the repo, not from memory.

## What to do

```bash
cd E:/Cena
echo "=== commits ==="
git --no-pager log --oneline -5
echo "=== uncommitted ==="
git status --short
echo "=== crates ==="
ls crates/
echo "=== tests ==="
cargo test --workspace 2>&1 | grep -cE '^test .* ok$'
echo "=== findings ==="
ls .workflows/findings/ 2>/dev/null
```

Then read `CLAUDE.md`'s "Next step" section and `plan/12-implementation-spec.md` §7.2
for the Milestone 1 criteria, and say which are met.

## How to report it

Four short sections:

1. **Committed** — what is done and verified.
2. **Uncommitted** — what is in the tree but not yet committed, and why.
3. **Running** — any live workflow (see `/agents`). Never report a pending agent's
   findings as though they had landed.
4. **Next** — the single next action, not a list of options.

Be honest about what is unfinished. This project has a standing habit of independent
verification precisely because self-reports have been wrong four times.
