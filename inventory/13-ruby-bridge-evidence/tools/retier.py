# Re-tier the census for a runner that is ordinary Ruby: Gtk (G11), the OS and
# network (G12) and eval/metaprogramming (G14) run as they do under Lich; only
# rewriting hooks (G10) and raw XML / Lich internals (G13) need Hydra's answer.
import json
d = json.load(open('census.json'))
for name in ('A', 'B'):
    rows = [r for r in d[name] if not r.get('dr')]
    def rewrites(r):
        return any(h[1].startswith(('rewrite', 'unclear')) for h in r.get('hooks', []))
    def xml(r):
        return 'G13' in r['groups']
    total_f, total_l = len(rows), sum(r['lines'] for r in rows)
    def row(label, pred):
        sel = [r for r in rows if pred(r)]
        f, l = len(sel), sum(r['lines'] for r in sel)
        print(f"  {label:<58} {f:>5} ({100*f/total_f:4.1f}%)  lines {l:>7} ({100*l/total_l:4.1f}%)")
    print(f"{name}: {total_f} GemStone files, {total_l} lines")
    row("runs with read-only hooks, no raw XML", lambda r: not rewrites(r) and not xml(r))
    row("...adding display/input hooks through the runner", lambda r: not xml(r))
    row("blocked by raw XML / Lich internals (G13)", xml)
    row("  of which G13 is the only blocker once hooks exist", lambda r: xml(r))
    row("needs the gtk gem (G11)", lambda r: 'G11' in r['groups'])
    row("rewrites display or input (G10 rewrite)", rewrites)
    # G13's reasons, among files G13 blocks
    reasons = {}
    for r in rows:
        for why in r['groups'].get('G13', []):
            reasons[why] = reasons.get(why, 0) + 1
    print("  G13 reasons (files):", reasons)
