"""Cut triage.tsv into nine review piles, each sorted by capture+data+api, richest first."""
import re
from pathlib import Path

HERE = Path(__file__).parent
LIB = Path(r"E:\Cena\reference\lich_repo_mirror\lib")

HUNT = re.compile(r"hunt|bigshot|bigshit|ashborne|killbig|fastkill|easykill|universalkill|champkill|"
                  r"\bgroup|follow|wander|campaign|rest_?room|flee|autohunt|grind", re.I)
CREATURE = re.compile(r"creature|critter|\bnpcs?\b|target|boss|invasion|bestiary|findcrea|mob", re.I)
BOXES = re.compile(r"\bbox|lock|pick|trap|disarm|wand|scroll|enchant|ensorcel|apprais|recall|"
                   r"analyze|\blore|tpick|\bpool|plinite|\bcoffer|\bchest|\bcaddy|strongbox", re.I)
OVERRIDE = {
    "character-planner.lic": "7-character-healing-crafting",
    "huntpro.lic": "1-hunting-engines",
    "intel_huntpro.lic": "1-hunting-engines",
    "spamfilter.lic": "9-ui-social-util",
}


def pile(name: str, domain: str, text: str) -> str:
    if name in OVERRIDE:
        return OVERRIDE[name]
    stem = name.lower()
    if domain in ("combat_creatures", "magic_effects") and HUNT.search(stem):
        return "1-hunting-engines"
    if domain == "combat_creatures":
        if HUNT.search(stem):
            return "1-hunting-engines"
        c = len(CREATURE.findall(text))
        k = len(re.findall(r"crit|damage|combat|rage|cman|warcry|maneuver|stance|\bAS\b|\bDS\b", text, re.I))
        if CREATURE.search(stem) or c > k:
            return "3-creatures"
        return "2-combat"
    if domain == "items_loot_trade":
        b = len(BOXES.findall(text))
        return "5-boxes-magic-items" if BOXES.search(stem) or b > 40 else "4-loot-trade"
    return {
        "magic_effects": "6-magic-effects",
        "character_state": "7-character-healing-crafting",
        "healing_herbs": "7-character-healing-crafting",
        "crafting_nonc": "7-character-healing-crafting",
        "bounty_tasks_events": "8-bounty-events-travel",
        "travel_map": "8-bounty-events-travel",
        "ui_social_util": "9-ui-social-util",
        "none": "9-ui-social-util",
        "dragonrealms": "9-ui-social-util",
    }[domain]


def main() -> None:
    rows = [l.split("\t") for l in (HERE / "triage.tsv").read_text(encoding="utf-8").splitlines()[1:]]
    piles: dict[str, list] = {}
    for script, lines, capture, data, api, dr, domain in rows:
        text = (LIB / script).read_bytes().decode("utf-8", errors="replace")
        p = pile(script, domain, text)
        score = int(capture) + int(data) + int(api)
        piles.setdefault(p, []).append((score, script, int(lines), int(capture), int(data), int(api)))
    for p, items in sorted(piles.items()):
        items.sort(key=lambda r: (-r[0], r[1]))
        with (HERE / f"pile-{p}.tsv").open("w", encoding="utf-8", newline="\n") as f:
            f.write("script\tlines\tcapture\tdata\tapi\n")
            for score, script, lines, capture, data, api in items:
                f.write(f"{script}\t{lines}\t{capture}\t{data}\t{api}\n")
        sub = sum(1 for r in items if r[3] + r[4] >= 5)
        print(f"{p:32} scripts {len(items):4}  substantive {sub:4}  lines {sum(r[2] for r in items):7}  "
              f"top: {', '.join(r[1] for r in items[:5])}")


if __name__ == "__main__":
    main()
