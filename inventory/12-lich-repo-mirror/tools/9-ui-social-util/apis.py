"""Summarise Lich API usage and wire tags per script: python apis.py a.lic b.lic ..."""
import re, sys, collections
L = 'E:/Cena/reference/lich_repo_mirror/lib/'
API = re.compile(r"\b(XMLData|GameObj|Effects::\w+|Spell|Spells|Char|Stats|Skills|Infomon|Bounty|Wounds|Scars|CMan|Feat|Shield|Weapon|Armor|Warcry|Resources|Society|Group|Lich::Gemstone::Group|Room|Map|Familiar|Experience|Gift|Currency|Account|Lich::Util|PSMS|Ascension|Enhancive|Injured|Claim|Disk|Creature|ActiveSpell|Lich::Messaging|Messaging|Vars|CharSettings|UserVars)(?:\.|\[|::)(\w+)?")
TAG = re.compile(r"<(/?)([A-Za-z][A-Za-z0-9_]*)(?=[\s/>])")
IDS = re.compile(r"id=\\?['\"]([A-Za-z][\w ]*)\\?['\"]")
for name in sys.argv[1:]:
    try:
        txt = open(L + name, 'rb').read().decode('utf-8', 'replace')
    except FileNotFoundError:
        print('MISSING', name)
        continue
    apis = collections.Counter()
    for m in API.finditer(txt):
        apis[m.group(1) + ('.' + m.group(2) if m.group(2) else '')] += 1
    tags = collections.Counter(m.group(2) for m in TAG.finditer(txt))
    ids = collections.Counter(IDS.findall(txt))
    print('=====', name, len(txt.splitlines()), 'lines')
    print('  API:', ', '.join(f'{k}x{v}' for k, v in apis.most_common(45)))
    print('  TAGS:', ', '.join(f'{k}x{v}' for k, v in tags.most_common(40)))
    print('  IDS:', ', '.join(f'{k}x{v}' for k, v in ids.most_common(30)))
