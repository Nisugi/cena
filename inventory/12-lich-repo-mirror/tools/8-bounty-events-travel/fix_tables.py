import pathlib
p = pathlib.Path(r"../8-bounty-events-travel.md")
t = p.read_text(encoding="utf-8")

# 1. col.lic row lacks the "fields" column.
old = "| col.lic:18-40 (spiffyjr, v1.0) | Council of Light tasks | 2 Q&A, 13 items -> shop rooms, entrance rooms |"
new = "| col.lic:18-40 (spiffyjr, v1.0) | Council of Light tasks | 2 Q&A, 13 items, 3 rooms | question -> answer; item -> shop room ids; poohbah and entrance rooms |"
assert old in t
t = t.replace(old, new)

# 2. OSA rows with an extra "what" cell: merge cells 2 and 3.
lines = t.split("\n")
for i, l in enumerate(lines):
    for key in ("| osacrew.lic:1767-1787, osatask.lic:98-172 |",
                "| ocean-go2.lic:37378-37478, osa-map-plugin.lic:9242-9280, stolenvalor.lic:77-129 |",
                "| newenemy.lic:1-345, MapEnemyShip.lic |",
                "| osacommander.lic:2157-2216 |"):
        if l.startswith(key):
            rest = l[len(key):]
            # rest = " what | data | Cena |"
            what, sep, remainder = rest.partition(" | ")
            lines[i] = key + what + ": " + remainder
t = "\n".join(lines)

# 3. E14 row lacks the "data or capture" column.
old3 = "| player-run games, spirit beasts (`extraordinary\\|perfect\\|robust\\|average\\|unimpressive specimen`), Rings of Lumnis, one-off quests, newbie start quests (Vaalor guard errands `Now, take this to Guardsman X`, sprite quest lines), Raging Thrak answers | none | GAP | LOW |"
new3 = "| player-run games, spirit beasts, Rings of Lumnis, one-off quests, newbie start quests, Raging Thrak quiz | spirit beast grades `extraordinary\\|perfect\\|robust\\|average\\|unimpressive specimen`; Vaalor errands `Now, take this to Guardsman X`; sprite quest lines; quiz answers | none | GAP | LOW |"
assert old3 in t, "E14"
t = t.replace(old3, new3)
p.write_text(t, encoding="utf-8")
print("ok")
