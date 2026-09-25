# A transcription of crates/cena-model/src/state/bounty.rs matchers (HEAD 3566f8d),
# used only to test sample game lines found in the survey pile's comments.
import re, pathlib

HMM = r"(?:Hmm, I've got a task here from .*?(?P<town>[A-Z].*?)\..*?)?"
LOCATION = r"(?:on|in|near) (?:the\s+)?(?P<area>[^.]+?)(?:\s+(?:near|between|under) (?P<town2>[^.]+))?"
GUARD = r"(?:one of the guardsmen just inside the (?P<town3>Ta'Illistim) City Gate|one of the guardsmen just inside the Sapphire Gate|one of the guardsmen just inside the gate|one of the (?P<town4>.*) (?:gate|tunnel) guards|one of the (?P<town5>Icemule Trace) gate guards or the halfing Belle at the Pinefar Trading Post|Quin Telaren of (?P<town6>Wehnimer's Landing)|the dwarven militia sergeant near the (?P<town7>Kharam-Dzu) town gates|the sentry just outside town|the sentry just outside (?P<town8>Kraken's Fall)|the purser of (?P<town9>River's Rest)|the tavernkeeper at Rawknuckle's Common House|the captain of the (?P<town10>Contempt)|the elderly guard in the East Guardtower)"
CONC = r"is working on a concoction that requires (?:an?|some|several) (?P<herb>[^.]+?) found [oi]n (?:the\s+)?(?P<area>[^.]+?)(?:\s+(?:near|under|between) [^.]+)?\.  These samples must be in pristine condition\.  You have been tasked to retrieve (?P<number>\d+) (?:more\s+)?samples?\."
MAYBE = r"(?:The taskmaster told you:  \x22)"

def ren(p, pairs):
    for a, b in pairs:
        p = p.replace(a, b)
    return p

specs = [
    ("None", r"^You are not currently assigned a task"),
    ("BanditAssignment", HMM + r"It appears they have a bandit problem they'd like you to solve"),
    ("CreatureAssignment", HMM + r"It appears they have a creature problem they'd like you to solve|" + MAYBE + r"I've got an urgent mission for you\.  We have a creature problem we'd like you to solve\.  Go report to the (?P<town11>[A-Z].*?) to find out|" + MAYBE + r"I've a favor to ask of you\.  We have a creature problem we'd like you to solve: you know, by killing\.  Go report to the (?P<town12>[A-Z].*?)"),
    ("GemAssignment", HMM + r"The local gem dealer, (?P<npc_name>[^,]+), has an order to fill and wants our help|All right\.  I've a mission for you\.  Our guest, the trader (?P<npc_name2>[^,]+)"),
    ("HeirloomAssignment", HMM + r"It appears they need your help in tracking down some kind of lost heirloom|" + MAYBE + r"?It's time for you to earn your keep around here\.  I'd like you track down a lost heirloom\."),
    ("HerbAssignment", HMM + r"The local [^,]+?, (?P<npc_name>[^,]+), has asked for our aid\.  Head over there and see what you can do\.  Be sure to ASK about BOUNTIES\.|" + MAYBE + r"I've got a mission for you\.  Our [^,]+?, (?P<npc_name2>[^,]+), has asked for our aid\.  Head over there and see what you can do\."),
    ("RescueAssignment", HMM + r"It appears that a local resident urgently needs our help in some matter"),
    ("SkinAssignment", HMM + r"The local furrier (?P<npc_name>.+) has an order to fill and wants our help|" + MAYBE + r"?You look like you need work\.  The flesh merchant (?P<npc_name2>.+), down in the hold, has an order to fill and wants our help\."),
    ("Taskmaster", r"^You have succeeded in your task and can return to the Adventurer's Guild"),
    ("HeirloomFound", r"^You have located (?:an?|some) (?P<item>.+) and should bring (?:it back|your find) to " + GUARD + r"\.$"),
    ("Guard", r"^You succeeded in your task and should report back to " + GUARD + r"\.$"),
    ("DangerousSpawned", r"^You have been tasked to hunt down and kill a particularly dangerous (?P<creature>[^.]+) that has established a territory " + LOCATION + r"\.  You have provoked (?:his|her|its) attention and now you must(?: return to where you left (?:him|her|it) and)? kill (?:him|her|it)!$"),
    ("RescueSpawned", r"^You have made contact with the child you are to rescue and you must get (?:him|her) back alive to " + GUARD + r"\.$"),
    ("Bandit", r"^You have been tasked to(?: help (?P<assist>\w+))? suppress (?P<creature>bandit) activity " + LOCATION + r"\.  You need to kill (?P<number>\d+) (?:more\s+)?of them to complete your task\.$"),
    ("Dangerous", r"^You have been tasked to hunt down and kill a (?:particularly )?dangerous (?P<creature>[^.]+) that has established a territory " + LOCATION + r"\.  You can get its attention by killing other creatures of the same type in its territory\.$"),
    ("Escort", MAYBE + r"?I've got a special mission for you\.  A certain client has hired us to provide a protective escort on (?:his|her) upcoming journey\.  Go to (?P<start>[^.]+) and WAIT for (?:him|her) to meet you there\.  You must guarantee (?:his|her) safety to (?P<destination>[^.]+) as soon as you can, being ready for any dangers that the two of you may face\.  Good luck!\x22?$"),
    ("Gem", r"^The gem dealer in (?:(?P<town>[^,]+), (?P<npc_name>[^,]+), )?has received orders from multiple customers requesting (?:an?|some) (?P<gem>[^.]+)\.  You have been tasked to retrieve (?P<number>\d+) (?:more\s+)?of them\.  You can SELL them to the gem dealer as you find them\.$"),
    ("Heirloom", r"^You have been tasked to recover (?:an?|some) (?P<item>[^.]+) that an unfortunate citizen lost after being attacked by an? (?P<creature>[^.]+?) " + LOCATION + r"\.  The heirloom can be identified by the initials \w+ engraved upon it\.  [^.]*?(?P<action>LOOT|SEARCH)[^.]+\.$"),
    ("Herb", r"^The .+? in (?P<town>[^,]+?), (?P<npc_name>[^,]+), " + CONC + r"$|^The .+, (?P<npc_name2>[^,]+), aboard the (?P<town2b>\w+?) in .*? " + ren(CONC, [("(?P<herb>", "(?P<herb2>"), ("(?P<area>", "(?P<area2>"), ("(?P<number>", "(?P<number2>")]) + r"$"),
    ("Rescue", r"^You have been tasked to rescue the young (?:runaway|kidnapped) (?:son|daughter) of a local citizen\.  A local divinist has had visions of the child fleeing from an? (?P<creature>[^.]+?) " + LOCATION + r"\.  Find the area where the child was last seen and clear out the creatures that have been tormenting (?:him|her) in order to bring (?:him|her) out of hiding\.$"),
    ("Skin", r"^You have been tasked to retrieve (?P<number>\d+) (?P<skin>[^.]+?)s? of at least (?P<quality>[^.]+) quality for (?P<npc_name>.+) in (?P<town>[^.]+?)\.  You can SKIN them off the corpse of an? (?P<creature>[^.]+) or purchase them from another adventurer\.  You can SELL the skins to the furrier as you collect them\.\x22?$"),
    ("Cull", r"^You have been tasked to help (?P<assist2>\w+) rescue a missing child by suppressing (?P<creature2>[^.]+) activity " + ren(LOCATION, [("(?P<area>", "(?P<area2>"), ("(?P<town2>", "(?P<town2b>")]) + r" during the rescue attempt\.  You need to kill (?P<number2>\d+) (?:more\s+)?of them to complete your task\.$|^You have been tasked to help (?P<assist3>\w+) retrieve an heirloom by suppressing (?P<creature3>[^.]+) activity " + ren(LOCATION, [("(?P<area>", "(?P<area3>"), ("(?P<town2>", "(?P<town2c>")]) + r" during the retrieval effort\.  You need to kill (?P<number3>\d+) (?:more\s+)?of them to complete your task\.$|^You have been tasked to help (?P<assist4>\w+) kill a dangerous creature by suppressing (?P<creature4>[^.]+) activity " + ren(LOCATION, [("(?P<area>", "(?P<area4>"), ("(?P<town2>", "(?P<town2d>")]) + r" during the hunt\.  You need to kill (?P<number4>\d+) (?:more\s+)?of them to complete your task\.$|^You have been tasked to(?: help (?P<assist>\w+))? suppress (?P<creature>[^.]+) activity " + LOCATION + r"\.  You need to kill (?P<number>\d+) (?:more\s+)?of them to complete your task\.$"),
    ("Failed", r"^You have failed in your task|^The child you were tasked to rescue is gone and your task is failed\.  Report this failure to the Adventurer's Guild\."),
]
MATCHERS = [(k, re.compile(p)) for k, p in specs]

def classify(text):
    text = text.strip()
    for k, rx in MATCHERS:
        m = rx.search(text)
        if m:
            caps = {n.rstrip("0123456789b"): v for n, v in m.groupdict().items() if v}
            return k, caps
    return None, {}

if __name__ == "__main__":
    for raw in pathlib.Path("samples_raw.txt").read_text(encoding="utf-8").splitlines():
        src, _, rest = raw.partition(":")
        ln, _, body = rest.partition(":")
        body = body.strip().lstrip("#").strip()
        if body.startswith("teststring = "):
            body = body[len("teststring = "):].strip().strip('"')
        body = body.rstrip()
        k, caps = classify(body)
        print(f"{src}:{ln}\t{k}\t{caps}\t{body[:110]}")
