import json, collections, re, sys

d = json.load(open('E:/Cena/reference/mapdb/map-1789942730.json', encoding='utf-8'))
BS = chr(92)
Q = re.compile('"(?:[^"' + BS*2 + ']|' + BS*2 + '.)*"|' + "'(?:[^'" + BS*2 + "]|" + BS*2 + ".)*'")
RX = re.compile('/(?:[^/ ' + BS*2 + ']|' + BS*2 + '.)(?:[^/' + BS*2 + ']|' + BS*2 + '.)*/[a-z]*')
NUM = re.compile(BS + 'd+(?:' + BS + '.' + BS + 'd+)?')
ARR = re.compile(BS + '[(?:' + BS + 's*(?:N|nil|S)' + BS + 's*,?)+' + BS + 's*' + BS + ']')
WS = re.compile(BS + 's+')


def norm(s):
    s = Q.sub('S', s)
    s = RX.sub('R', s)
    s = NUM.sub('N', s)
    s = ARR.sub('[..]', s)
    return WS.sub(' ', s).strip()


out = sys.argv[1]
for field in ('wayto', 'timeto'):
    c = collections.Counter()
    ex = {}
    for r in d:
        for k, v in r[field].items():
            if isinstance(v, str) and v.startswith(';e'):
                n = norm(v)
                c[n] += 1
                ex.setdefault(n, (r['id'], k, v))
    with open(f'{out}/shapes_{field}.tsv', 'w', encoding='utf-8') as f:
        for n, k in c.most_common():
            rid, to, v = ex[n]
            f.write(f"{k}\t{len(v)}\t{rid}->{to}\t{n}\n")
    tot = sum(c.values())
    print(field, 'procs', tot, 'shapes', len(c), 'singletons', sum(1 for x in c.values() if x == 1),
          'edges-in-singletons-or-pairs', sum(x for x in c.values() if x <= 2))
    run = 0
    for i, (n, k) in enumerate(c.most_common(), 1):
        run += k
        if i in (10, 25, 50, 100, 150, 200, 300):
            print('  top', i, run, f'{run/tot:.1%}')

RD = ['Stats.race', 'Stats.prof', 'Stats.level', 'Stats.', 'Skills.', 'Spells.', 'Spell[', 'checkspell',
      'Society.', 'UserVars.', 'Vars[', '$go2', '$mapdb', 'GameObj.npcs', 'GameObj.loot', 'GameObj.inv',
      'GameObj.right_hand', 'GameObj.left_hand', 'GameObj.pcs', 'GameObj.room_desc', 'checkloot', 'checknpcs',
      'checkpaths', 'XMLData.room_exits', 'XMLData.encumbrance', 'XMLData.', 'Room.current', 'Map.current',
      'checksitting', 'kneeling?', 'standing?', 'hidden?', 'invisible?', 'dead?', 'checkroom', 'checkrt',
      'Char.', 'Lich::', 'bounty?', 'Time.now', 'percentencumbrance', 'checkmana', 'checkstamina', 'Wounds.',
      'Scars.', 'Effects::', 'Group.', 'checkname', 'Currency', 'silver', 'Script.', 'start_script',
      'Settings', 'CharSettings', 'reget', 'matchfind', 'matchwait', 'matchtimeout', 'waitfor', 'dothistimeout',
      'fput', 'multifput', 'put ', 'move ', 'move(', 'waitrt?', 'waitcastrt?', 'pause', 'sleep', 'wait_until',
      'wait_while', 'empty_hands', 'fill_hands', 'empty_hand', 'fill_hand', 'respond', 'echo', 'Map.dijkstra',
      'Map.findpath', 'Room[', 'Map[', '.call', 'rand', 'loop', 'while', 'until', '.times', '.each', 'eval',
      'require', 'File.', 'Lich::Util', 'Gtk', 'Spell[', '.cast', 'affordable?', 'known?', 'get?', 'get ',
      'clear', 'Lich::Stash', 'Inventory', 'Claim', 'kill_script', 'Script.running', 'running?']
reads = collections.Counter()
for r in d:
    for field in ('wayto', 'timeto'):
        for v in r[field].values():
            if isinstance(v, str) and v.startswith(';e'):
                b = Q.sub('S', v)
                for t in set(RD):
                    if t in b:
                        reads[(field, t)] += 1
for f in ('wayto', 'timeto'):
    print(f, sorted([(t, n) for (ff, t), n in reads.items() if ff == f], key=lambda x: -x[1]))
