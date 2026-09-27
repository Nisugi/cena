"""Capability-group census of two .lic collections.

Run:  python census.py            -> writes census.json and prints summary tables
Method: each file is lexed by rblex.lex (comments, =begin/=end, __END__, strings, regexes,
heredocs and %-literals removed; #{...} interpolation code kept and appended). Every group
pattern below is matched against that code, with word boundaries, and never after '.', '::',
':' (symbols), '@' or '$' unless the pattern says so.
"""
import glob
import json
import os
import re
import sys
from collections import Counter, defaultdict

from rblex import lex

# The reference clones, three directories up from here (`CLAUDE.md`).
REF = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..', 'reference'))
COLLS = {
    'A': f'{REF}/scripts/scripts/*.lic',
    'B': f'{REF}/lich_repo_mirror/lib/*.lic',
}
OUT = os.path.dirname(os.path.abspath(__file__))


def read(p):
    b = open(p, 'rb').read()
    try:
        return b.decode('utf-8')
    except UnicodeDecodeError:
        return b.decode('latin-1')


def fn(*names):
    """A bare global-function call: not a method (.x), not scope (::x), not a symbol (:x),
    not @x/$x, not a hash key (x:), not a local assignment (x = ...), not a def."""
    alt = '|'.join(re.escape(n) for n in names)
    return re.compile(r'(?<![\w.:@$])(?<!def )(?<!def  )(?:' + alt + r')(?![\w?!])(?!:[^:])(?!\s*(?:=(?![=~])|\+=|-=|\|\|=))')


def const(*names):
    """A constant used as a receiver: X.  X[  X::  (not ::X unless via Lich:: normalisation)."""
    alt = '|'.join(re.escape(n) for n in names)
    return re.compile(r'(?<![\w.:@$])(?:' + alt + r')(?=\s*(?:\.|\[|::|\)|,|$|\s))(?!\w)', re.M)


def const_recv(*names):
    alt = '|'.join(re.escape(n) for n in names)
    return re.compile(r'(?<![\w.:@$])(?:' + alt + r')(?=\s*(?:\.|\[|::))')


# ----------------------------------------------------------------------------------------------
# Groups. Each group: list of (label, compiled regex).
# ----------------------------------------------------------------------------------------------
G = {}
G['G1'] = [
    ('put/fput/multifput/move/dothis/dothistimeout/multimove',
     fn('put', 'fput', 'multifput', 'move', 'dothis', 'dothistimeout', 'multimove', 'forceput', 'selectput')),
    ('send helpers: empty_hands/fill_hands/walk/Stash', fn('empty_hands', 'empty_hand', 'fill_hands', 'fill_hand',
                                                              'empty_left_hand', 'empty_right_hand', 'fill_left_hand',
                                                              'fill_right_hand', 'walk')),
    ('Stash', const_recv('Stash')),
    ('Game.puts/_puts', re.compile(r'(?<![\w.:])Game\._?puts\b')),
]
G['G2'] = [
    ('waitfor/match family/get/reget/clear',
     fn('waitfor', 'waitforre', 'matchwait', 'matchtimeout', 'matchfind', 'matchfindword', 'matchfindexact',
        'matchbefore', 'matchafter', 'matchboth', 'match', 'get', 'get?', 'reget', 'regetall', 'clear', 'wait',
        'unique_get', 'unique_get?', 'unique_waitfor')),
    ('Watchfor', const_recv('Watchfor')),
    ('Lich::Util (send+capture)', re.compile(r'\bLich::Util\b')),
]
G['G3'] = [
    ('waitrt?/waitcastrt?/checkrt/checkcastrt', fn('waitrt?', 'waitcastrt?', 'waitrt', 'waitcastrt', 'checkrt', 'checkcastrt')),
    ('pause', fn('pause')),
    ('sleep', fn('sleep')),
    ('wait_until/wait_while', fn('wait_until', 'wait_while')),
    ('XMLData timing fields', re.compile(r'\bXMLData\.(?:server_time|server_time_offset|roundtime_end|cast_roundtime_end|last_pulse)\b')),
]
G['G4'] = [
    ('echo/respond/_respond', fn('echo', '_echo', 'respond', '_respond')),
    ('Lich::Messaging', re.compile(r'\bLich::Messaging\b|(?<![\w.:])Messaging\.')),
    ('monsterbold', fn('monsterbold', 'monsterbold_start', 'monsterbold_end')),
    ('puts/print (stdout is the client)', fn('puts', 'print')),
]
ROOM_FIELDS = r'room_id|room_count|room_title|room_exits|room_description|room_exits_string|room_num|room_name|current_target_id|current_target_ids|familiar_room_title|familiar_room_description|familiar_room_exits'
G['G5'] = [
    ('GameObj', const_recv('GameObj')),
    ('check* room/hands', fn('checkloot', 'checknpcs', 'checkpcs', 'checkroom', 'checkpaths', 'checkarea', 'checkleft',
                             'checkright', 'checkroomdescrip', 'roomdescription?', 'checkfamroom', 'checkfamnpcs',
                             'checkfampcs', 'checkfampaths', 'checkfamarea', 'checkfamroomdescrip', 'count_npcs',
                             'righthand?', 'lefthand?', 'righthand', 'lefthand', 'outside?', 'checkoutside')),
    ('XMLData room fields', re.compile(r'\bXMLData\.(?:' + ROOM_FIELDS + r')\b')),
    ('Claim/Disk/Creature', const_recv('Claim', 'Disk', 'CreatureInstance')),
]
CHAR_FIELDS = r'name|level|injuries|injury|bounty_task|society_task|society_rank|society|mind_text|mind_value|stance_text|stance_value|encumbrance_text|encumbrance_value|stamina|max_stamina|health|max_health|spirit|max_spirit|mana|max_mana|next_level_text|next_level_value|active_spells|indicator|prepared_spell|profession|prof|race|exp|ascension_exp|dead|position|player_id|riding|group_members|stow_container_id'
G['G6'] = [
    ('Char/Stats/Skills/Spell(s)/Effects/...', const_recv(
        'Char', 'Stats', 'Skills', 'Spell', 'Spells', 'Effects', 'Society', 'Bounty', 'Experience', 'Wounds', 'Scars',
        'Injured', 'CMan', 'Feat', 'Armor', 'Shield', 'Weapon', 'Warcry', 'PSMS', 'Ascension', 'Enhancive', 'Resources',
        'Currency', 'Group', 'Infomon', 'Gift', 'ReadyList', 'StowList', 'SpellRanks', 'Spellsong', 'CritRanks',
        'Stance', 'Overwatch', 'Armaments', 'Mana', 'SK')),
    ('check*/percent*/max* vitals', fn('checkmana', 'checkhealth', 'checkspirit', 'checkstamina', 'checkstance', 'checkmind',
                                       'check_mind', 'checkencumbrance', 'checkbounty', 'checkname', 'checkprep', 'checkspell',
                                       'checkspells', 'checkactive', 'checkprepared', 'prepped?', 'active?', 'mind?',
                                       'percentmana', 'percenthealth', 'percentspirit', 'percentstamina', 'percentmind',
                                       'percentstance', 'percentencumbrance', 'percentconcentration', 'maxmana', 'max_mana',
                                       'maxhealth', 'maxspirit', 'maxstamina', 'maxconcentration', 'mana?', 'health?',
                                       'spirit?', 'stamina?', 'stance?', 'encumbrance?', 'bounty?', 'myname?', 'watchhealth',
                                       'fix_injury_mode')),
    ('status predicates', fn('standing?', 'stunned?', 'dead?', 'bleeding?', 'hidden?', 'hiding?', 'invisible?', 'webbed?',
                             'kneeling?', 'sitting?', 'prone?', 'muckled?', 'poisoned?', 'diseased?', 'calmed?', 'silenced?',
                             'bound?', 'joined?', 'group?', 'grouped?', 'cutthroat?', 'sleeping?', 'fried?', 'saturated?',
                             'thorned?', 'checkstunned', 'checkdead', 'checkbleeding', 'checkreallybleeding', 'checkhidden',
                             'checkhiding', 'checkinvisible', 'checkwebbed', 'checkkneeling', 'checksitting', 'checkprone',
                             'checkstanding', 'checknotstanding', 'checkpoison', 'checkdisease', 'checkgrouped',
                             'checkjoined', 'checkcalmed', 'checksilenced', 'checkbound', 'checkcutthroat',
                             'checksleeping', 'checkfried', 'checksaturated')),
    ('cast', fn('cast')),
    ('XMLData character fields', re.compile(r'\bXMLData\.(?:' + CHAR_FIELDS + r')\b(?!=)')),
]
G['G7'] = [
    ('Room/Map', const_recv('Room', 'Map')),
    ('findpath/dijkstra', re.compile(r'(?<![\w@$])(?:findpath|dijkstra)\b')),
    ('reverse_direction', fn('reverse_direction')),
    # go2-as-a-script is added from the dependency pass
]
G['G8'] = [
    ('Script.*', const_recv('Script', 'ExecScript')),
    ('start/kill/pause/unpause/send_to_script',
     fn('start_script', 'start_scripts', 'force_start_script', 'start_scripts_if_available', 'start_exec_script',
        'kill_script', 'kill_scripts', 'stop_script', 'stop_scripts', 'pause_script', 'pause_scripts', 'unpause_script',
        'unpause_scripts', 'send_to_script', 'send_scripts', 'send_script', 'unique_send_to_script')),
    ('running?', fn('running?')),
    ('before_dying/undo_before_dying', fn('before_dying', 'undo_before_dying')),
    ('variable (script args)', fn('variable')),
    ('engine flags: no_pause_all/no_kill_all/hide_me/silence_me/...',
     fn('no_pause_all', 'no_kill_all', 'hide_me', 'hide_script', 'silence_me', 'echo_on', 'echo_off', 'toggle_echo',
        'die_with_me', 'i_stand_alone', 'setpriority', 'priority?', 'quiet_exit', 'toggle_unique', 'report_errors',
        'want_downstream', 'want_script_output', 'idle?')),
    ('goto (label jump)', fn('goto')),
    # shared $globals are added in classify()
]
G['G9'] = [
    ('UserVars/Vars', const_recv('UserVars', 'Vars')),
    ('Settings/CharSettings/GameSettings', const_recv('Settings', 'CharSettings', 'GameSettings')),
    ('Lich.db / DB_Store', re.compile(r'(?<![\w.:])Lich\.db\b|\bDB_Store\b')),
    ('Lich.<name> (=Vars via method_missing)', None),   # computed
    ('SessionVars', const_recv('SessionVars')),
]
G['G10'] = [
    ('DownstreamHook', const_recv('DownstreamHook')),
    ('UpstreamHook', const_recv('UpstreamHook')),
    ('upstream_get/toggle_upstream/want_upstream', re.compile(r'(?<![\w:@$])(?:upstream_get\??|upstream_waitfor|toggle_upstream|want_upstream)(?![\w?])')),
]
G['G11'] = [
    ('Gtk/Gdk/GLib/Pango/Cairo', re.compile(r'(?<![\w.@$])(?:Gtk|Gdk|GdkPixbuf|GLib|Pango|Cairo)(?=\s*(?:\.|::|\[))')),
    ('Lich.msgbox', re.compile(r'(?<![\w.:])Lich\.msgbox\b')),
]
G['G12'] = [
    ('file writes', None),          # computed
    ('network', re.compile(r'\bNet::(?:HTTP|FTP|SMTP|POP3|IMAP)|\b(?:TCPSocket|TCPServer|UDPSocket|UNIXSocket|Socket\.(?:new|tcp)|OpenSSL::SSL|URI\.open|WEBrick|DRb|HTTParty|Faraday|RestClient|Excon)\b|(?<![\w.])open-uri')),
    ('subprocess', re.compile(r'(?<![\w.:@$])(?:system|spawn|exec)(?=\s*\(|[ \t]+(?:__S|__R|[\w$@"]))(?![\w?!])|(?<![\w.:@$])fork\s*(?:\{|do\b)|\b(?:Open3|IO\.popen|Process\.spawn|Kernel\.system|Win32API|WIN32OLE|Fiddle|Win32::)\b')),
    ('sqlite (direct)', re.compile(r'\bSQLite3\b|\bSequel\b')),
]
G['G13'] = [
    ('$_SERVER*/$_CLIENT* buffers', re.compile(r'\$_(?:SERVER|CLIENT|DETACHABLE|LASTUPSTREAM|IDLETIMESTAMP)[A-Z_]*')),
    ('XMLData other fields', None),   # computed
    ('want_downstream_xml', re.compile(r'(?<![\w:@$])want_downstream_xml(?![\w?])')),
    ('status_tags', re.compile(r'(?<![\w:@$])(?:status_tags|toggle_status)(?![\w?])')),
    ('strip_xml/sf_to_wiz/fb_to_sf', fn('strip_xml', 'sf_to_wiz', 'fb_to_sf')),
    ('Lich internals (Lich.x defined by Lich, Lich::X other than Messaging/Util/API classes)', None),  # computed
    ('Game.* / Frontend internals', None),  # computed
    ('Lich internal globals', re.compile(r'\$(?:safe_pause_lock|pause_all_lock|psinet|infomon_debug|creature_debug|setupfiles|fill_hands_actions|fill_left_hand_actions|fill_right_hand_actions|speech_highlight_start|speech_highlight_end|link_highlight_start|link_highlight_end|strip_xml_multiline|sftowiz_multiline)\b')),
]
G['G14'] = [
    ('eval of strings / bindings', re.compile(r'(?<![\w.:@$])eval(?=\s*\(|[ \t]+(?:__S|[\w$@"]))(?![\w?!])|(?:\.|(?<![\w.:]))(?:instance_eval|class_eval|module_eval)\s*\(|\bTOPLEVEL_BINDING\b')),
    ('instance_eval/class_eval blocks, define_method, instance_exec', re.compile(r'\b(?:instance_eval|class_eval|module_eval|instance_exec|class_exec|define_method|define_singleton_method)\b')),
    ('send/__send__/public_send', None),  # computed
]
# informational groups (not in the tiers)
INFO = {
    'T_threads': re.compile(r'\bThread\.(?:new|start|fork)\b'),
    'I_file_read': re.compile(r'\bFile\.(?:read|readlines|foreach|exist\?|exists\?|file\?|size|mtime|directory\?)|\bDir\.(?:glob|entries|\[|exist\?|children|each_child)|\bYAML\.(?:load_file|safe_load_file)|\bJSON\.load_file'),
    'I_quiet_command_xml': re.compile(r'\bLich::Util\.quiet_command_xml\b'),
    'I_frontend_branch': re.compile(r'\$frontend\b|(?<![\w.:])Frontend\.'),
    'I_ivar_const_get': re.compile(r'\b(?:instance_variable_get|instance_variable_set|const_get|const_set|class_variable_get|class_variable_set)\b'),
    'I_method_missing': re.compile(r'\bdef\s+(?:self\.)?method_missing\b'),
}

RUBY_GLOBALS = set('''$0 $1 $2 $3 $4 $5 $6 $7 $8 $9 $~ $! $@ $& $+ $` $' $* $$ $? $: $; $, $. $/ $\\ $< $> $_ $" $= $-w $-v $-d
$stdout $stderr $stdin $DEBUG $VERBOSE $LOAD_PATH $LOADED_FEATURES $PROGRAM_NAME $SAFE $FILENAME $KCODE $-0 $-a $-i $-l $-p'''.split())
ENV_GLOBALS = {'$frontend', '$lich_char', '$clean_lich_char', '$lich_char_regex', '$data_dir', '$script_dir', '$lich_dir',
               '$temp_dir', '$log_dir', '$backup_dir', '$map_dir', '$version', '$LICH_VERSION', '$stormfront',
               '$fake_stormfront', '$platinum', '$login_time', '$SEND_CHARACTER', '$cmd_prefix', '$room_count',
               '$last_dir', '$nav_seen', '$last_logoff', '$ORDINALS', '$box_regex', '$wine_bin', '$wine_prefix', '$zip'}
G15 = [
    ('Lich-set globals ($frontend, $lich_char, $data_dir, ...)', None),  # computed
    ('LICH_VERSION / XMLData.game / Frontend queries', re.compile(r'\bLICH_VERSION\b|\bXMLData\.game\b|(?<![\w.:])Frontend\.(?:client|supports_[a-z_]+\?|display_name)')),
]

LICH_DEFINED = set('''break_game_host_port class_eval class_variable_get cleanup_debug_logs configure_sqlite_connection core_updated_with_lich_version db_maint_due? db_maint_last_at db_maint_lock_path db_maint_set! db_mutex db_vacuum_if_due! debug_messaging deprecated display_exits display_expgains display_lichid display_room_links display_room_mono display_roomid_location display_stringprocs display_uid find_hosts_file fix_game_host_port get_simu_launcher hide_uid_flag hosts_file init_db inventory_boxes link_to_sal link_to_sge log max_debug_logs method_missing modify_hosts module_eval mutex_lock mutex_unlock open_sequel_sqlite open_sqlite_db restore_hosts seek set_inventory_boxes show_deprecated_log sqlite_busy_timeout_ms track_autosort_state track_dark_mode track_layout_state track_persistent_launcher_mode unlink_from_sal unlink_from_sge win screen_reader? respond_to? version'''.split())
API_CLASSES = set('''GameObj Char Stats Skills Spell Spells Effects Society Bounty Experience Wounds Scars Injured CMan Feat Armor
Shield Weapon Warcry PSMS Ascension Enhancive Resources Currency Group Infomon Gift ReadyList StowList SpellRanks Spellsong
CritRanks Stance Overwatch Armaments Mana Claim Disk Creature CreatureInstance Room Map Script Settings CharSettings
GameSettings UserVars Vars DownstreamHook UpstreamHook Watchfor Stash SessionVars DB_Store Messaging Util SK Frontend XMLData'''.split())

NORM_PREFIX = re.compile(r'(?:::)?\b(?:Lich::)?(?:Common|Gemstone|Games::Gemstone|Games::DragonRealms|DragonRealms)::(?=[A-Z])')
LICH_NS = re.compile(r'(?<![\w.:])Lich((?:::[A-Z]\w*)+)')
LICH_DOT = re.compile(r'(?<![\w.:])Lich\.([a-z_]\w*[?!=]?)')
XML_FIELD = re.compile(r'\bXMLData\.([a-z_]\w*[?!=]?)')
GAME_DOT = re.compile(r'(?<![\w.:])(Game|Frontend)\.([a-z_]\w*[?!=]?)')
SEND = re.compile(r'(?:\.|(?<![\w.:@$]))(?:send|__send__|public_send)\s*(?:\(\s*|\s+)(:[\w?!=]+|__S(\d+)__|[a-z_@][\w]*)')
GLOBAL = re.compile(r'\$[A-Za-z_]\w*')
DEF_NAMES = re.compile(r'\bdef\s+(?:self\.|[A-Z]\w*\.)?([a-z_]\w*[?!]?)')
NESTED_CLASS = re.compile(r'^[ \t]+(?:class|module)\s+([A-Z]\w*)\s*(?:$|<|;)', re.M)
ASSIGNED_CODE = re.compile(r'(?<![.\w@$:])([a-z_][A-Za-z0-9_]*)\s*(?:=(?![=~>])|\+=|-=|\|\|=)')
BLOCKPARAM_CODE = re.compile(r'\|([^|\n]*)\|')
NORM_PREFIX2 = re.compile(r'\bLich::(?=(?:Claim|Stash|Resources|Currency|Group|Bounty|Society|Stats|Skills|Spell|Spells|Effects|Char|GameObj|CMan|Feat|Armor|Shield|Weapon|Warcry|PSMS|Experience|Wounds|Scars|Injured|Infomon|Gift|ReadyList|StowList|Stance|Overwatch|Disk|Room|Map|Script|Settings|CharSettings|GameSettings|UserVars|Vars|DownstreamHook|UpstreamHook|Watchfor|SessionVars|DB_Store)\b)')
FILE_WRITE_MODE = re.compile(r'\b(?:File|IO)\.(?:open|new)\s*\(?\s*[^,\n]+,\s*__S(\d+)__|(?<![\w.:])open\s*\(?\s*[^,\n]+,\s*__S(\d+)__|\bCSV\.open\s*\(?\s*[^,\n]+,\s*__S(\d+)__')
FILE_WRITE_DIRECT = re.compile(r'\b(?:File|IO)\.(?:write|binwrite|delete|unlink|rename|chmod|truncate|symlink|link)\b|\bFileUtils\.|\bDir\.(?:mkdir|rmdir|delete|unlink)\b|\bFile::(?:WRONLY|CREAT|APPEND|RDWR)\b|\bTempfile\b')

# DragonRealms test: the census's DR vocabulary (DR-only Lich modules), the DR dependency loader,
# a DR game header, or a DR-only game guard.
DR_TEST = re.compile(r'(?<![\w.:])(?:DRC|DRCI|DRCT|DRCC|DRCA|DRCM|DRCH|DRCS|DRCMM|DRStats|DRSkill|DRRoom|DRSpells|DRInfomon|DRExpMonitor)\.|\bcustom_require\b')
DR_GUARD = re.compile(r'(?:exit|return|abort)\s+unless\s+XMLData\.game\s*=~\s*/\^?DR|XMLData\.game\s*!~\s*/\^?DR/[^\n]{0,40}\b(?:exit|abort|return)\b|(?:exit|abort)[^\n]{0,20}\bif\s+XMLData\.game\s*!~\s*/\^?DR')


def classify_hooks(code, strings, defs_body, regexes=()):
    """Return list of (kind, verdict, detail) for every Downstream/UpstreamHook.add in code."""
    out = []
    for m in re.finditer(r'(?<![\w])(DownstreamHook|UpstreamHook)\.add\s*(\(?)', code):
        kind = m.group(1)
        i = m.end()
        # skip first argument (name)
        depth = 0
        j = i
        while j < len(code):
            ch = code[j]
            if ch in '([{':
                depth += 1
            elif ch in ')]}':
                if depth == 0:
                    break
                depth -= 1
            elif ch == ',' and depth == 0:
                break
            elif ch == '\n' and depth == 0 and not m.group(2):
                break
            j += 1
        if j >= len(code) or code[j] != ',':
            out.append((kind, 'unresolved', 'no second argument', False))
            continue
        arg_start = j + 1
        rest = code[arg_start:arg_start + 400].lstrip()
        cands = []   # (body, param, is_def)
        detail = ''
        PROC = r'(?:proc|lambda|Proc\.new|->\s*(?:\(\s*\*?(\w+)[^)]*\)|(\w+))?)\s*(\{|do\b)'
        mm = re.match(PROC, rest)
        if mm:
            abs_start = arg_start + (len(code[arg_start:arg_start + 400]) - len(rest)) + mm.start(3)
            cands.append((extract_block(code, abs_start), mm.group(1) or mm.group(2), False))
            detail = 'inline'
        else:
            mv = re.match(r'(?:self\.|[A-Z]\w*\.)?method\s*\(\s*(?::(\w+[?!]?)|__S(\d+)__)\s*\)', rest)
            if mv:
                name = mv.group(1) or strings[int(mv.group(2))]
                if name in defs_body:
                    cands.append((defs_body[name], None, True))
                detail = f'method({name})'
            else:
                mi = re.match(r'(@@?\w+|\$\w+|[A-Za-z_]\w*(?:\.[a-z_]\w*[?!]?)?)', rest)
                if mi:
                    name = mi.group(1)
                    for ma in re.finditer(r'(?<![\w.])' + re.escape(name) + r'\s*=\s*' + PROC, code):
                        cands.append((extract_block(code, ma.start(3)), ma.group(1) or ma.group(2), False))
                    last = name.split('.')[-1]
                    if not cands and last in defs_body:
                        db = defs_body[last]
                        mp = re.search(PROC, db)
                        if mp:
                            cands.append((extract_block(db, mp.start(3)), mp.group(1) or mp.group(2), False))
                    detail = f'var {name}' if cands else f'unresolved {name}'
        if not cands:
            out.append((kind, 'unresolved', detail, False))
            continue
        verdicts = [judge(b, defs_body, param=pa, is_def=d) for b, pa, d in cands]
        rw = [v for v in verdicts if v.startswith('rewrite')]
        bodies = chr(10).join(b for b, _, _ in cands)
        xml = any(re.search(r'<[a-zA-Z/!?]', regexes[int(x)]) for x in re.findall(r'__R(\d+)__', bodies) if int(x) < len(regexes)) or bool(re.search(r'strip_xml|sf_to_wiz', bodies))
        out.append((kind, rw[0] if rw else verdicts[0], detail, xml))
    return out


OPENER = re.compile(r'(?<![\w.:@$])(?:do|def|class|module|begin|case|if|unless|while|until|for)(?![\w?!:])|(?<![\w.:@$])end(?![\w?!:])')


def extract_block(code, start):
    """start points at '{' or 'do'. Returns the block text (without delimiters)."""
    if code[start] == '{':
        depth = 0
        for k in range(start, len(code)):
            if code[k] == '{':
                depth += 1
            elif code[k] == '}':
                depth -= 1
                if depth == 0:
                    return code[start + 1:k]
        return code[start + 1:]
    return extract_keyword_block(code, start, 2)


def extract_keyword_block(code, start, skip):
    depth = 0
    line_start_loop = False
    for m in OPENER.finditer(code, start):
        w = m.group(0)
        if w == 'end':
            depth -= 1
            if depth == 0:
                return code[start + skip:m.start()]
            continue
        if w in ('if', 'unless', 'while', 'until'):
            # modifier form? look back on the line
            ls = code.rfind('\n', 0, m.start()) + 1
            before = code[ls:m.start()].strip()
            if before and not re.search(r'(?:=|\(|\|\||&&|\breturn|;|\bthen|\bdo|\belse|\bnot|<<|\?|:|,|\[|\{)$', before):
                continue
            if w in ('while', 'until'):
                line_start_loop = True
        if w == 'for':
            line_start_loop = True
        if w == 'do' and line_start_loop:
            ls = code.rfind('\n', 0, m.start()) + 1
            if re.search(r'\b(?:while|until|for)\b', code[ls:m.start()]):
                line_start_loop = False
                continue
        depth += 1
    return code[start + skip:]


def def_bodies(code):
    bodies = {}
    for m in re.finditer(r'(?<![\w.])def\s+(?:self\.|[A-Z]\w*\.)?([a-z_]\w*[?!]?)', code):
        bodies.setdefault(m.group(1), extract_keyword_block(code, m.start(), 3))
    return bodies


NIL_RETURN = re.compile(r'(?:\breturn|\bnext|\bbreak)(?:\s+|\s*\(\s*)nil\b|(?:^|[;{]|\bthen|\belse|\bdo|\s\?|\s:(?!:))[ \t]*nil(?:[ \t]*(?:$|[;}]|\bend\b|\bif\b|\bunless\b)|(?=\s+:(?!:)))', re.M)
BRANCH_KW = re.compile(r'(?:elsif|else|when|in)\b')


def judge(body, defs_body, depth=0, param=None, is_def=False):
    """Classify one hook body: 'read-only', 'rewrite (<signals>)' or 'unclear'.

    rewrite signals: nil returned anywhere (squelch); the line mutated in place or reassigned;
    return/next with another value; a last expression that is not the line; `line if cond`;
    an if/case whose final `else` returns the line but another top-level branch ends in
    something else (an implicit other value); a delegate method that itself rewrites.
    read-only: none of those, and the last expression is the line (or a delegate that is read-only).
    """
    if param is None:
        if is_def:
            pm = (re.match(r'\s*(?:self\.|[A-Z]\w*\.)?[a-z_]\w*[?!]?\s*\(\s*\*?(\w+)', body)
                  or re.match(r'\s*(?:self\.|[A-Z]\w*\.)?[a-z_]\w*[?!]?[ \t]+\*?(\w+)', body))
            if pm:
                # drop the signature line from the body
                body = body[body.find('\n') + 1:] if '\n' in body else ''
        else:
            pm = re.match(r'\s*\|\s*\*?(\w+)', body)
            if pm:
                body = body[pm.end():]
                body = body[body.find('|') + 1:]
        param = pm.group(1) if pm else None
    signals = []
    if NIL_RETURN.search(body):
        signals.append('nil')
    if param:
        p = re.escape(param)
        if re.search(r'\b' + p + r'\s*(?:\.(?:gsub!|sub!|replace|insert|prepend|concat|squeeze!|tr!|tr_s!|chomp!|chop!|strip!|lstrip!|rstrip!|upcase!|downcase!|capitalize!|swapcase!|slice!|delete!|clear|encode!|force_encoding|unicode_normalize!|scrub!)|<<|\[[^\]]*\]\s*=(?!=))', body):
            signals.append('mutates')
        if re.search(r'(?<![\w.])' + p + r'\s*(?:=(?![=~])|\+=)', body):
            signals.append('reassigns')
        for rm in re.finditer(r'\b(?:return|next)\b[ \t]*([^\n;]*)', body):
            val = rm.group(1).strip()
            val = re.split(r'(?:^|\s+)(?:if|unless)\b', val)[0].strip()
            if val and val != param and val != 'nil':
                signals.append('returns other')
                break
    # statements with their keyword nesting depth; ';' separates statements too
    seq = []      # (depth, text); 'end' markers kept so a branch ending in a nested block is 'unknown'
    d = 0
    for raw in body.split('\n'):
        for part in raw.split(';'):
            t = part.strip()
            if not t:
                continue
            if re.fullmatch(r'(?:end|\}|\)|\s)+', t):
                d -= len(re.findall(r'end|\}', t))
                seq.append((d, '<end>'))
                continue
            if BRANCH_KW.match(t):
                seq.append((d - 1, t))
                continue
            seq.append((d, t))
            if re.match(r'(?:if|unless|case|while|until|begin|for)\b', t) or re.search(r'(?:=|\()\s*(?:if|unless|case|begin)\b', t) \
                    or re.search(r'\bdo\s*(?:\|[^|]*\|)?$', t):
                d += 1
            d += t.count('{') - t.count('}') - len(re.findall(r'(?<![\w.:])end(?![\w?!:])', t))
    stmts = [(dd, t) for dd, t in seq if t != '<end>']
    last = stmts[-1][1] if stmts else ''
    ro_confirmed = bool(param) and (last == param or re.fullmatch(r'(?:return|next)\s*\(?\s*' + re.escape(param) + r'\s*\)?', last) is not None)
    if ro_confirmed and len(seq) >= 2:
        k = max(i for i, (dd, t) in enumerate(seq) if t == last)
        ldep = seq[k][0]
        if ldep > 0:
            # the line is returned from inside a block: find what encloses it
            j = k - 1
            while j >= 0 and not (seq[j][0] == ldep - 1 and seq[j][1] != '<end>'):
                j -= 1
            opener = seq[j][1] if j >= 0 else ''
            if re.match(r'(?:if|unless|elsif|when|in)\b|.*(?:=|\()\s*(?:if|unless|case)\b', opener):
                signals.append('branch')          # no final else: the other path returns nil
            elif opener == 'else':
                dep = ldep - 1
                ok = lambda pt: (pt == param or re.fullmatch(r'(?:return|next)\s*\(?\s*' + re.escape(param) + r'\s*\)?', pt)
                                 or re.search(r'\bthen\s+' + re.escape(param) + r'$', pt) or 'nil' in pt or pt == '<end>')
                m2 = j
                while m2 > 0:
                    d2, t2 = seq[m2]
                    if d2 == dep and re.match(r'(?:if|unless|case)\b|.*(?:=|\()\s*(?:if|unless|case)\b', t2):
                        break
                    if d2 == dep and (BRANCH_KW.match(t2) or t2 == 'else'):
                        pd, pt = seq[m2 - 1]
                        if not (pd == dep and re.match(r'(?:if|unless|case|when)\b', pt)) and not ok(pt):
                            signals.append('branch')
                            break
                    m2 -= 1
    delegated_ro = False
    if stmts and not ro_confirmed:
        if param and re.match(re.escape(param) + r'\s+(?:if|unless)\b', last):
            signals.append('conditional')     # `line if cond` returns nil otherwise
        elif re.fullmatch(r'[\w.@:]+(?:\(.*\))?', last) and last != param and not re.fullmatch(r'(?:end|else|elsif|when|rescue|ensure|begin)', last):
            # last expression is something else (a call or a variable)
            callee = re.match(r'(?:[\w@:]+\.)*([a-z_]\w*[?!]?)', last)
            if callee and callee.group(1) in defs_body and depth < 2 and param and param in last:
                sub = judge(defs_body[callee.group(1)], defs_body, depth + 1, is_def=True)
                if sub.startswith('rewrite'):
                    signals.append('delegate rewrites')
                elif sub == 'read-only':
                    delegated_ro = True
            else:
                signals.append('last')
    if signals:
        return 'rewrite (' + ','.join(sorted(set(signals))) + ')'
    return 'read-only' if (ro_confirmed or delegated_ro) else 'unclear'


# ----------------------------------------------------------------------------------------------
LICH_BUILTINS = {'k', 'kill', 'ka', 'killall', 'p', 'pause', 'pa', 'pauseall', 'u', 'unpause', 'ua', 'unpauseall', 'e',
                 'eq', 'es', 'exec', 'execq', 'l', 'list', 'la', 'listall', 'force', 'send', 'sn', 's', 'trust', 'distrust',
                 'set', 'help', 'magic', 'hide', 'unhide', 'lich5-update', 'banks', 'display', '?', 'favs', 'fav',
                 'lichs', 'lnet_off', 'sb', 'sp', 'spell', 'gs'}
SCRIPT_NAME = r'[A-Za-z][\w\-]*'


def target_from_cmd(s):
    """';go2 bank' or '#{$lich_char}go2 bank' -> ('start'|'control', name) or None."""
    s2 = re.sub(r'^#\{\s*\$(?:lich_char|clean_lich_char)\s*\}', ';', s.strip())
    if not s2.startswith(';'):
        return None
    m = re.match(r';(' + SCRIPT_NAME + r')(?:\s+(' + SCRIPT_NAME + r'))?', s2)
    if not m:
        return ('dynamic', None) if s2.startswith(';#{') else None
    w1 = m.group(1).lower()
    if w1 in ('k', 'kill', 'p', 'pause', 'u', 'unpause', 'send', 'sn'):
        return ('control', m.group(2).lower()) if m.group(2) else None
    if w1 == 'force':
        return ('start', m.group(2).lower()) if m.group(2) else None
    if w1 in LICH_BUILTINS:
        return None
    return ('start', w1)


def norm_target(s):
    s = s.strip().lower()
    s = re.sub(r'\.lic$', '', s)
    if '#{' in s or not re.fullmatch(r'[a-z0-9][\w\-]*', s):
        return None
    return s


def deps(code, strings, caller):
    edges = []  # (kind, target)
    dynamic = 0

    def add(kind, t):
        nonlocal dynamic
        t2 = norm_target(t) if t is not None else None
        if t2 is None:
            dynamic += 1
        elif t2 != caller:
            edges.append((kind, t2))

    starts = r'(?:Script\.(?:run|start|start_script)|(?<![\w.:])(?:start_script|force_start_script))'
    for m in re.finditer(starts + r'\s*\(?\s*(__S(\d+)__|[\w@$]+)', code):
        if m.group(2):
            add('start', strings[int(m.group(2))])
        else:
            dynamic += 1
    for m in re.finditer(r'(?<![\w.:])start_scripts(?:_if_available)?\s*\(?\s*((?:__S\d+__\s*,\s*)*__S\d+__)', code):
        for k in re.findall(r'__S(\d+)__', m.group(1)):
            for w in strings[int(k)].split():
                add('start', w)
    for m in re.finditer(r'(?:Script\.exists\?)\s*\(?\s*__S(\d+)__', code):
        add('exists', strings[int(m.group(1))])
    for m in re.finditer(r'(?:Script\.running\?|(?<![\w.:])running\?)\s*\(?\s*((?:__S\d+__\s*,\s*)*__S\d+__)', code):
        for k in re.findall(r'__S(\d+)__', m.group(1)):
            add('running', strings[int(k)])
    for m in re.finditer(r'(?:Script\.(?:kill|pause|unpause)|(?<![\w.:])(?:kill_scripts?|stop_scripts?|pause_scripts?|unpause_scripts?|send_to_script|unique_send_to_script|send_scripts?))\s*\(?\s*((?:__S\d+__\s*,\s*)*__S\d+__)', code):
        ks = re.findall(r'__S(\d+)__', m.group(1))
        name = code[m.start():m.start() + 30]
        if 'send' in name:
            ks = ks[:1]
        for k in ks:
            add('control', strings[int(k)])
    for m in re.finditer(r'Script\.running\.(?:find|any\?|select|detect|each|count|none\?)[^\n]{0,80}?\.name(?:\.downcase)?\s*(?:==|=~|\.include\?\s*\(?)\s*(__S(\d+)__|__R(\d+)__)', code):
        if m.group(2):
            add('running', strings[int(m.group(2))])
    for m in re.finditer(r'(?:(?<![\w.:])(?:fput|put|forceput|multifput|do_client)|Game\._?puts)\s*\(?\s*((?:__S\d+__\s*,\s*)*__S\d+__)', code):
        for k in re.findall(r'__S(\d+)__', m.group(1)):
            t = target_from_cmd(strings[int(k)])
            if t:
                if t[0] == 'dynamic':
                    dynamic += 1
                else:
                    add(t[0], t[1])
    return edges, dynamic


def analyse(path):
    raw = read(path).replace('\r\n', '\n')
    nlines = raw.count('\n') + (0 if raw.endswith('\n') or not raw else 1)
    lx = lex(raw)
    code = lx.code
    full = code + '\n' + '\n'.join(lx.interp)
    norm = NORM_PREFIX2.sub('', NORM_PREFIX.sub('', full))
    caller = os.path.basename(path)[:-4].lower()
    defs = set(DEF_NAMES.findall(norm))
    # a nested class/module with an API name is the script's own; a top-level one reopens Lich's
    local_consts = set(NESTED_CLASS.findall(norm))
    locals_ = set(ASSIGNED_CODE.findall(norm))
    for bp in BLOCKPARAM_CODE.findall(norm):
        locals_.update(re.findall(r'[a-z_]\w*', bp))
    hits = defaultdict(set)

    def check(g, label, rx):
        for m in rx.finditer(norm):
            tok = m.group(0).strip()
            base = re.match(r'[\w?!]+', tok.lstrip('$.'))
            name = base.group(0) if base else tok
            if name in defs or name in local_consts:
                continue
            if name in locals_:
                # the name is also a local variable here: count only a call with arguments
                rest = norm[m.end():m.end() + 40]
                if not re.match(r'\s*\(|[ \t]+(?:__[SR]\d|[A-Za-z_$@:"\'\-\d\[])', rest) or \
                        re.match(r'[ \t]+(?:if|unless|and|or|then|do|while|until|rescue|in)\b', rest):
                    continue
            hits[g].add(label)
            return

    for g, rows in G.items():
        for label, rx in rows:
            if rx is not None:
                check(g, label, rx)
    for label, rx in G15:
        if rx is not None:
            check('G15', label, rx)
    info = {k: bool(rx.search(norm)) for k, rx in INFO.items()}

    # Lich namespace
    for m in LICH_NS.finditer(full):
        parts = [p for p in m.group(1).split('::') if p]
        parts = [p for p in parts if p not in ('Common', 'Gemstone', 'Games', 'DragonRealms', 'Util') or p == 'Util']
        head = parts[0] if parts else ''
        if head in ('Messaging', 'Util') or head in API_CLASSES:
            continue
        hits['G13'].add('Lich internals (Lich.x defined by Lich, Lich::X other than Messaging/Util/API classes)')
    for m in LICH_DOT.finditer(full):
        name = m.group(1)
        if name == 'db':
            continue
        if name == 'msgbox':
            continue
        if name.rstrip('=') in LICH_DEFINED or name in LICH_DEFINED:
            hits['G13'].add('Lich internals (Lich.x defined by Lich, Lich::X other than Messaging/Util/API classes)')
        else:
            hits['G9'].add('Lich.<name> (=Vars via method_missing)')
    for m in XML_FIELD.finditer(full):
        f = m.group(1)
        if re.fullmatch(ROOM_FIELDS, f) or re.fullmatch(CHAR_FIELDS, f) or f in ('game', 'server_time', 'server_time_offset', 'roundtime_end', 'cast_roundtime_end', 'last_pulse', 'respond_to?', 'nil?'):
            continue
        hits['G13'].add('XMLData other fields')
    for m in GAME_DOT.finditer(norm):
        k, meth = m.group(1), m.group(2)
        if k == 'Game' and meth in ('puts', '_puts', 'respond_to?'):
            continue
        if k == 'Frontend' and (meth in ('client', 'display_name', 'respond_to?') or re.fullmatch(r'supports_\w+\?', meth)):
            continue
        hits['G13'].add('Game.* / Frontend internals')
    for m in SEND.finditer(norm):
        arg = m.group(1)
        if arg.startswith('__S'):
            if not re.fullmatch(r'[a-z_]\w*[?!=]?', lx.strings[int(m.group(2))]):
                continue
        elif not arg.startswith(':') and not re.match(r'[a-z_@]', arg):
            continue
        # socket/buffer sends with a variable argument are ambiguous; require symbol/method-like string, or a receiver-less send
        pre = norm[max(0, m.start() - 20):m.start()]
        if not arg.startswith(':') and not arg.startswith('__S') and re.search(r'(?:socket|sock|server|client|_SERVER_|_CLIENT_|conn|tcp)\w*$', pre, re.I):
            continue
        hits['G14'].add('send/__send__/public_send')
        break
    # file writes
    for m in FILE_WRITE_MODE.finditer(norm):
        k = m.group(1) or m.group(2) or m.group(3)
        mode = lx.strings[int(k)] if k is not None and int(k) < len(lx.strings) else ''
        if re.match(r'\s*[wa]|.*\+', mode):
            hits['G12'].add('file writes')
            break
    if FILE_WRITE_DIRECT.search(norm):
        hits['G12'].add('file writes')
    if lx.backticks:
        hits['G12'].add('subprocess')
    # requires
    reqs = set()
    for m in re.finditer(r'(?<![\w.])require\s*\(?\s*__S(\d+)__', code):
        reqs.add(lx.strings[int(m.group(1))].strip())
    for r in reqs:
        if re.match(r'(?:net/|socket|open-uri|openssl|drb|webrick|uri$)', r) and r != 'uri':
            hits['G12'].add('network')
        if r in ('sqlite3', 'sequel'):
            hits['G12'].add('sqlite (direct)')
        if r in ('open3',):
            hits['G12'].add('subprocess')
        if re.match(r'gtk|gdk|glib|cairo|pango', r):
            hits['G11'].add('Gtk/Gdk/GLib/Pango/Cairo')
    # globals
    gl = set(GLOBAL.findall(full))
    env_used = {g for g in gl if g in ENV_GLOBALS}
    if env_used:
        hits['G15'].add('Lich-set globals ($frontend, $lich_char, $data_dir, ...)')
    user_globals = {g for g in gl if g not in RUBY_GLOBALS and g not in ENV_GLOBALS and not g.startswith('$_')
                    and not re.fullmatch(r'\$(?:safe_pause_lock|pause_all_lock|psinet|infomon_debug|creature_debug|setupfiles|fill_hands_actions|fill_left_hand_actions|fill_right_hand_actions|speech_highlight_start|speech_highlight_end|link_highlight_start|link_highlight_end|strip_xml_multiline|sftowiz_multiline)', g)}
    # hooks
    hooks = []
    if hits.get('G10'):
        ncode = NORM_PREFIX2.sub('', NORM_PREFIX.sub('', code))
        hooks = classify_hooks(ncode, lx.strings, def_bodies(ncode), lx.regexes)
    edges, dyn = deps(NORM_PREFIX2.sub('', NORM_PREFIX.sub('', code)) + '\n' + '\n'.join(lx.interp), lx.strings, caller)
    if any(t == 'go2' and k == 'start' for k, t in edges):
        hits['G7'].add('go2 as a script')
    dr = bool(DR_TEST.search(norm)) or bool(DR_GUARD.search(raw)) or bool(re.search(r'^\s*#?\s*game\s*:\s*(?:dr|dragonrealms)\b', raw[:3000], re.I | re.M))
    return {
        'file': os.path.basename(path),
        'lines': nlines,
        'groups': {g: sorted(v) for g, v in hits.items() if v},
        'user_globals': sorted(user_globals),
        'hooks': hooks,
        'edges': sorted(set(edges)),
        'dynamic_starts': dyn,
        'requires': sorted(reqs),
        'info': info,
        'dr': dr,
        'lex_errors': lx.errors,
    }


def main():
    res = {}
    for c, pat in COLLS.items():
        files = sorted(glob.glob(pat))
        res[c] = [analyse(f) for f in files]
        print(c, len(files), file=sys.stderr)
    # shared user globals: a $name used in 2+ files of A u B
    cnt = Counter()
    for c in res:
        for r in res[c]:
            cnt.update(set(r['user_globals']))
    for c in res:
        for r in res[c]:
            shared = [g for g in r['user_globals'] if cnt[g] >= 2]
            r['shared_globals'] = shared
            if shared:
                r['groups'].setdefault('G8', []).append('shared $globals')
    json.dump(res, open(os.path.join(OUT, 'census.json'), 'w'), indent=1)


if __name__ == '__main__':
    main()
