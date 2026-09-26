"""Count rows in gshunting4all.lic data constants.

For each constant: find its start line, scan to the closing bracket at the
constant's own indentation, then count rows with a per-table regex.
"""
import re

PATH = 'E:/Cena/reference/lich_repo_mirror/lib/gshunting4all.lic'
lines = open(PATH, 'rb').read().decode('utf-8', 'replace').splitlines()

# name -> row regex (applied to each line inside the constant's span)
TABLES = {
    'CASTER_NOUNS': r'^\s+[a-z ]+$',  # words; counted by split below
    'REIM_BOSS_NAMES': r'"[^"]+"',
    'MINIBOSS_NAMES': r'"[^"]+"',
    'SANCTUARY_ROOMS': r'^\s+\d+,',
    'WOUND_BODY_PARTS': r'^\s+\["',
    'SCRIPT_TEMPLATES': r'\{ *key:',
    'PROFESSION_SPELL_CIRCLES': r'^\s+"[A-Za-z]+"\s*=>',
    'OFFENSIVE_SPELL_NUMS': r'^\s+\d+,',
    'WEAPON_TYPES': r'\{ *label:',
    'ARMOR_TYPES': r'\{ *asg:',
    'WEAPON_NOUN_HINTS': r'^\s+\[/',
    'ENCUMBRANCE_LEVELS': r'\["',
    'EG_ABILITIES': r'\{ *key:',
    'TG_ABILITIES': r'\{ *key:',
    'TK_ABILITIES': r'\{ *key:',
    'SONIC_WPN_OPTS': r'\{ *label:',
    'SONIC_SHLD_OPTS': r'\{ *label:',
    'SINGING_SWORD_OPTS': r'\{ *label:',
    'BARD_CYCLIC_SONGS': r'\{ *num:',
    'BARD_DEFENSIVE_SONGS': r'\{ *num:',
    'BARD_UTILITY_SONGS': r'\{ *num:',
    'SOCIETY_DATA': r'\{ *name:',
    'SOCIETY_ATTACKS': r'\{ *name:',
    'PASSIVE_CMANS': r'"[^"]+"',
    'SPELL_CIRCLES': r'^\s+\[|\{',
    'WEAPON_TECH_KNOWN': r'^\s+"[a-z]+"\s*=>',
    'TECH_TYPE_MAP': r'=>',
    'OFFENSIVE_SPELLS': r'\{|\[\s*\d+',
    'WG_WARCRIES': r'\{ *key:',
    'GEMSTONE_ACTIVATED_PROPS': r'\{ *name:',
    'SURGE_CD_BY_RANK': r'\d+ *=>',
    'ARCARIUM_VERBS': None,
}


def span(name):
    for i, l in enumerate(lines):
        m = re.match(r'^(\s*)' + name + r'\s*=', l)
        if m:
            ind = len(m.group(1))
            if l.rstrip().endswith('.freeze') and l.count('[') == l.count(']') and l.count('{') == l.count('}'):
                return i, i
            for j in range(i + 1, len(lines)):
                lj = lines[j]
                if re.match(r'^\s{%d}[\]\}]' % ind, lj) and len(lj) - len(lj.lstrip()) == ind:
                    return i, j
    return None


for name, rx in TABLES.items():
    s = span(name)
    if not s:
        print(f'{name}: not found')
        continue
    i, j = s
    body = lines[i:j + 1]
    if name == 'CASTER_NOUNS':
        n = sum(len(x.split()) for x in body[1:-1])
    elif rx is None:
        n = len(re.findall(r'\w+', body[0].split('%w[')[1].split(']')[0]))
    else:
        n = sum(len(re.findall(rx, x)) for x in body[1:] if not x.strip().startswith('#'))
    print(f'{name}: lines {i + 1}-{j + 1}, rows={n}')
