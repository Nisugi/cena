---
name: check
description: Build, test, lint and format the workspace, and report exactly what failed
argument-hint: ""
tools: Bash
---

# Check

The full verification run. Use it before any commit, and after any agent claims
the tree is green — agents in this project have overclaimed four times and been
caught by independent checking.

## What to do

```bash
cd "$(git rev-parse --show-toplevel)"
echo "=== tests ==="
cargo test --workspace 2>&1 | grep -E 'test result:|^error|FAILED|panicked at' | tail -25
echo "=== count ==="
cargo test --workspace 2>&1 | grep -cE '^test .* ok$'
echo "=== clippy ==="
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
echo "=== fmt ==="
cargo fmt --check 2>&1 | head -20
echo "=== tree ==="
git status --short
```

## How to report it

State the passing test count, and clippy/fmt as clean or not. If anything fails,
paste the actual error rather than summarising it.

**Check for probe debris.** Agents leave files behind in `crates/*/examples/`.
Anything there that is not `cut_fixtures.rs` is probably a leftover probe — name it
rather than deleting silently.

Never report "all green" without having run this.
