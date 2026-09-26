"""Triage lich_repo_mirror/lib/*.lic: capture/data signals and a primary domain per script.

Writes triage.tsv beside this file. Heuristic, not a parser: the counts rank scripts for
reading, they are not findings.
"""
import re
from pathlib import Path

LIB = Path(r"E:\Cena\reference\lich_repo_mirror\lib")
OUT = Path(__file__).with_name("triage.tsv")

CAPTURE = re.compile(
    r"=~\s*/|!~\s*/|waitforre|matchtimeout|matchwait|matchfind|match_?before|\.match\(|"
    r"Regexp\.new|%r\{|%r\(|DownstreamHook|UpstreamHook|when\s+/|\.scan\(/|\bmatch\s*\(|"
    r"reget|waitfor\b|get\?|dothistimeout|\bgrep\s*\(",
)
DATA_LINE = re.compile(r"""^\s*(['"][^'"]{2,}['"]\s*(=>|,|:)|[a-z_]+:\s*['"\[{0-9]|\[\s*['"])""")
API = re.compile(
    r"\b(GameObj|Spell\[|Spells\.|Effects::|Char\.|Stats\.|Skills\.|Wounds\.|Scars\.|Bounty\.|"
    r"XMLData\.|Room\.current|Map\[|Map\.|Infomon|CMan|Feat\.|Shield\.|Weapon\.|Armor\.|Warcry|"
    r"Society\.|Resources\.|Lich::Gemstone|Lich::Util|Currency|Experience\.|Gift\.|Enhancive|"
    r"Creature\.|Group\.|Claim\.|Readylist|StowList|Disk\.|ActiveSpell|checkbounty|checkhealth)"
)
DR = re.compile(r"\b(DRStats|DRSkill|DRC[A-Z]?\b|DRCI|DRCT|DRRoom|dragonrealms|get_settings\(|Flags\.add)")

DOMAINS = {
    "combat_creatures": r"crit|damage|\bAS\b|\bDS\b|endroll|stance|maneuver|cman|warcry|weapon tech|"
                        r"creature|critter|\bnpcs\b|bestiary|undead|\bboss\b|invasion|kill|target|"
                        r"attack|swing|\bhit\b|combat|rage|stun|webbed|prone",
    "magic_effects": r"spell|\bcast\b|\bprep|\bincant|mana|spirit|stamina|effect|cooldown|buff|"
                     r"spellup|spellburst|\brune|wand|sigil|song|\bsign of|infuse|ensorcel|enchant",
    "character_state": r"experience|\bexp\b|skills?\b|stats?\b|resource|voln|favor|\bki\b|"
                       r"encumbr|\bmind:|fame|lumnis|deeds?|society|ascension|enhanciv|"
                       r"training|ranks|level|\binfo\b|profile|citizenship|premium|subscription",
    "items_loot_trade": r"\bloot|container|inventory|\bstow|\bgem|jewel|\bbox|locksmith|lockpick|"
                        r"\btrap|disarm|\bpick\b|scroll|skin|furrier|pawn|\bsell|merchant|shop|"
                        r"silver|\bbank|appraise|analyze|recall|weight|\bgird|disk",
    "bounty_tasks_events": r"bounty|\btask|adventurer|guild|heirloom|escort|bandit|rescue|forage|"
                           r"\bcull|duskruin|rumor woods|ebon gate|raffle|\bnexus|gemstone|"
                           r"hinterwild|festival|event|lottery|\bquest|contract",
    "travel_map": r"\bgo2\b|room\.current|\bmap\b|wayto|obvious exits|\bpath|navigat|chronomage|"
                  r"ferry|\bocean|\bship|sail|teleport|travel|\bexits?\b|uid|\broom\b",
    "healing_herbs": r"\bheal|herb|empath|\bwound|\bscar|injur|\bcure|\bblood|poison|disease|"
                     r"transfer|\bsurge\b|\bhealth\b|\bdeath|\bdead\b|decay|deed|favor|resurrect",
    "crafting_nonc": r"forg|fletch|alchem|cook|fish|garden|sculpt|tinker|craft|brew|mine\b|"
                     r"workorder|recipe|\bpotion|\bdye|tailor|leather",
    "ui_social_util": r"window|highlight|squelch|\blnet|\bchat|\bthink|\bwhisper|\blog\b|gtk|"
                      r"\bgui\b|panel|stream|<pushStream|\bpreset|\bmacro|alias|notify|alert|sound|"
                      r"\bping\b|\bwho\b|\bfriends?\b",
}
DOMAIN_RE = {k: re.compile(v, re.I) for k, v in DOMAINS.items()}


def main() -> None:
    rows = []
    for path in sorted(LIB.glob("*.lic")):
        text = path.read_bytes().decode("utf-8", errors="replace")
        lines = text.splitlines()
        code = [l for l in lines if not l.lstrip().startswith("#")]
        capture = sum(1 for l in code if CAPTURE.search(l))
        data = sum(1 for l in code if DATA_LINE.search(l))
        api = len(API.findall(text))
        dr = len(DR.findall(text))
        name = path.stem.lower()
        scores = {}
        for dom, rx in DOMAIN_RE.items():
            s = len(rx.findall(text)) + 25 * len(rx.findall(name))
            scores[dom] = s / max(1, len(lines)) ** 0.5
        domain = max(scores, key=scores.get) if any(scores.values()) else "none"
        if dr >= 3:
            domain = "dragonrealms"
        rows.append((path.name, len(lines), capture, data, api, dr, domain))
    with OUT.open("w", encoding="utf-8", newline="\n") as f:
        f.write("script\tlines\tcapture\tdata\tapi\tdr\tdomain\n")
        for r in rows:
            f.write("\t".join(map(str, r)) + "\n")
    print("wrote", OUT, len(rows))


if __name__ == "__main__":
    main()
