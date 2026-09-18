---
name: corpus
description: Query the 49.55 GB XML log corpus for a tag, phrase or pattern
argument-hint: "<pattern>"
tools: Bash
---

# Corpus

Ask the log archive a question. Two years of real wire traffic is the tiebreaker
whenever a source disagrees with another — Vellum, the protocol wiki and Saga are
all secondhand; this is what the server actually sent.

`E:\Gemstone\data\log archive` — 10,849 `.xml`, 49.55 GB, Oct 2024 → Sep 2026,
~64 character directories, layout `<Character>/<year>/<month>/xml/<file>.xml`.

## What to do

Sample **stratified, never `head`** — sorted, then every Nth, so the sample spans
characters, years and months and is reproducible:

```bash
cd "E:/Gemstone/data/log archive"
ls */*/*/xml/*.xml | awk 'NR%40==1' > /tmp/sample.txt
wc -l < /tmp/sample.txt
grep -oh "$ARGUMENTS" $(cat /tmp/sample.txt) 2>/dev/null | sort | uniq -c | sort -rn | head -20
```

Widen by lowering the divisor; a rare tag needs a bigger sample than a common one.
Read at most ~50 files in full — these average 4.5 MB.

## Rules that have already cost time here

- **The 11,862 `.log` files are a different, tag-stripped format.** Never use them as
  protocol evidence. Only `.xml` is raw wire.
- **Zero hits does not mean "not real."** Setup-phase tags (`FEStart`) and
  request-response tags (`inventoryManager`) legitimately never appear, because Lich
  logs start after setup and never send the request. Say "not observed in this
  sample", not "does not exist".
- **Crit and combat messages carry embedded markup** — a creature name arrives as
  `<pushBold/><a exist="..." noun="kobold">kobold</a><popBold/>`. Grepping for plain
  prose will miss them. Match markup-tolerantly or strip first.
- Logs contain the author's real character names, and 20.5% carry `druby://` URIs with
  real link-local IPv6 addresses. **Never paste raw corpus lines into a commit, an
  issue, or a committed fixture.**

## How to report it

Give the command, the sample size, and the counts. A corpus claim without the command
that produced it is not evidence (`plan/05` §−2).
