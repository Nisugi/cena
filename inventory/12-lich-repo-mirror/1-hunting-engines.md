# 1-hunting-engines: lich_repo_mirror survey

Scope: 31 scripts, 20 substantive; 13 read deeply (plus 4 variants by `diff`), 6 at capture-line depth
through checked extractions (Ashborne's coordination layer read deeply, the rest of it at capture depth),
11 in the tail; Cena HEAD 3566f8d at the start and a6146e6 at the end
(`git -C E:/Cena rev-parse --short HEAD`; of the cited files only `hunt/drive.rs` changed, and its cite is re-read), 2026-09-25.

## Top findings

1. **Ashborne's file bus is a complete requirements list for group hunting.** 26 shared files (special
   ask 1, §A) plus a remote-command channel (§C) carry member telemetry, follow and ready gates, an
   assist target, loot election, a danger broadcast with rescue, spell and interrupt claims, job
   election, a phased town round and a repeat-cycle handoff. In Hydra most of it becomes a read of
   another session's `GameState`; what remains is 14 group decisions (§D, R1-R14), none built.
   Two are cheap and matter before groups: the room claim should treat **every character this Hydra
   runs** as "with me" (R13; Ashborne's own-character tie-break, `Ashborne.lic:7114-7131`,
   `:22196-22215`), and a follower whose leader vanished defends the room 30 s, then withdraws
   (R14, `Ashborne.lic:20985-21050`), which is the author's recorded reconnect design (`plan/30` §4) in
   running code. HIGH, after solo M6.
2. **`invalid_targets` is imported with the wrong meaning.** bigshot: "but don't count these" toward
   the flee count (`bigshot.lic:3484`, `:8579-8583`), still attacked. Cena: `ignore`, never attacked
   (`hunt/import.rs:391`, `profile.rs:82-83`, `engine.rs:528-544`). huntpro's and Ashborne's
   "ignore" lists mean "leave the room" (Cena's `flee.from`), and both flee at `>=` where bigshot and
   Cena flee at `>` (`huntpro.lic:2041-2051`, `Ashborne.lic:9545`). **CONFLICT, HIGH (M6 import).**
3. **A contested room stops Cena's hunt, and it loots there anyway.** `engage` needs the claim every
   tick (`engine.rs:430`), `wander` will not leave while creatures remain (`:571-573`), `loot` has no
   claim check (`:286-330`). bigshot claims on entry and keeps it while fighting (`bigshot.lic:9392`),
   gates looting on it (`:7808`) and counts a stranger's floating disk as contesting the room (`:7091-7099`); every
   engine here walks on. INFERRED from the tick order; PARTIAL/CONFLICT, HIGH (M6).
4. **Lich has already collected most of these engines' emergency lines, and Cena chose not to port
   them.** `reference/lich-5/lib/gemstone/combat/defs/messages.rb` (240 lines, 2026-09-16) gathers
   bigshot's, ecleanse's and eohunter's hook regexes into 8 families and 21 events: self-disarm (6
   forms), hazards and hive traps, ambushers and "You bolt", rooted and "cannot move your legs",
   item limit, bless shrugged and expired, arrow stuck, aiming, bonded weapon return, 703 and 1614 marks,
   **Swift Justice charges** (`plan/33`'s `justice`, held as "later" for want of this classifier) and
   Arcane Reflex, and the weapon-reaction prompt. `common/spell.rb:21-60` holds the prepare and cast
   replies, including sanctuary ("no need for spells of war"), cutthroat and "can't think clearly".
   `crates/cena-model/src/state/combat.rs:90-93` records `messages.rb` as not ported, "a later pass if
   a consumer wants it"; the M6 hunt is that consumer. **GAP, HIGH (M6).**
5. **Rest triggers Cena's profile cannot express**: spirit ≤ 10%, any wound or scar rank ≥ N (per
   part in gshunting4all), stunned/webbed/bound, poisoned/diseased, "your attack has no effect" three
   times (wrong equipment), and a cycle count to stop an overnight run. Special ask 2 has the lines.
   **GAP/PARTIAL, HIGH (M6).**
6. **Environmental flight is missing**: hazard objects (cloud, void, web, vine, sandstorm, tempest,
   swarm, pod), gas clouds forming (`Ashborne.lic:17317-17345`), ground about to erupt, sanctuary
   refusals, and two hazard "creatures" Cena would attack (wasp nest, shimmering fungus, `creatures.tsv:484`,
   `:602`). bigshot has `flee_clouds/vines/webs/voids` settings (`bigshot.lic:3491-3494`) that the importer
   drops. **GAP, HIGH (M6).**
7. **Cena holds per-creature message data it never reads.** `state/creature_message.rs:245` `classify`
   has no caller outside two test files, so 281 `spell_prep` lines (an interrupt signal Ashborne builds
   with a cruder verb list, `Ashborne.lic:16460-16474`) and the Weapon Fire triggers
   (`creature_messages.tsv:3132-3133`) reach nothing. PARTIAL, HIGH (M6).
8. **Bounty → hunting ground is the data Cena lacks for M8.** Cena reads every bounty sentence
   generically (`state/bounty.rs`), better than the 1,124 regexes of `Ashborne.lic:749-1877` or
   intel_huntpro's 282-row table, but has only creature → area-name and uid range
   (`creature_areas.tsv`). The engines carry the ground: Ashborne 531 zones with start rooms and
   21,839 room ids (`:5097-5629`), huntpro 254 areas, huntplan 1,307 spawn lists over 8,810 rooms,
   huntingareas 38. Also missing: "You are / will be eligible for new task assignment"
   (`intel_huntpro.lic:1201-1206`) and the bounty creature's arrival line
   (`bountyhunter-support.lic:178`). **PARTIAL/GAP, HIGH (M8).**
9. **A Lich bug was ported faithfully.** `crates/cena-model/src/movement.rs:107` has
   `^[A-z\s-] is unable to follow you\.$` from `reference/lich-5/lib/common/move.rb:290`; the class has no
   `+`, so no real name matches (VERIFIED with Python `re`). Ashborne's is
   `^([A-Za-z][A-Za-z'\-]*) is unable to follow you\.` (`Ashborne.lic:10912`). **CONFLICT, MEDIUM**
   (HIGH once groups move together).
10. **Healing in the field, for M6d**: another character's rank-1 wounds read from `appraise`
    (`Ashborne.lic:18157-18201`), Empath self-heal at wound ≥ 1, the Empath-first town pass behind a
    phase barrier (special ask 1, #16-#19). **GAP, HIGH (M6d).**
11. **huntplan's creature data mostly agrees with Cena's**: 517 of 548 names at the same level, 7
    level conflicts, 3 creatures with messages but no `creatures.tsv` row (shadowy spectre, shan
    sorcerer, infernal lich); ~21 immunity facts Cena's 18 immunity rows lack (lesser vruul ×6, storm
    giant 710, wood wight 1002 ...). The `mana` report (max mana, pulse off node) and a bard's
    `song status` renewal cost are not captured. MEDIUM.
12. **`bigshit2.lic` is a prank, not a bigshot fork to learn from.** On random rests it silences all game
    output, posts on public channels and strips the character in town (`bigshit2.lic:5306-5395`,
    `:389-390`). Nothing to port; worth a warning, and an argument for `plan/35` that a hook which can
    swallow lines can blind the player.

## Special ask 1: Ashborne's shared files (the requirements list for in-process group hunting)

**Source.** `Ashborne.lic` v0.8.18 (2026-09-19), author NecroDeus, no license line (header :1-7). Each
character runs its own Lich and its own copy of Ashborne; they coordinate through files in
`$lich_dir/data/ashborne/` (`ashborne_data_file` :6780) plus a remote-command channel (below).
Ashborne says so itself: *"Ashborne control traffic uses Borg and local files, not group whispers
or ;chat."* (:8375). VERIFIED: every file name below came from
`grep -oE "_(ashborne|intel_ashborne|huntpro|uberbeta)_[a-zA-Z0-9_#{}.]*\.(txt|yml|yaml|log|json)" Ashborne.lic | sort | uniq -c`
(30 distinct patterns) plus `grep -nE "_file(\(...\))? *\+ *\"" Ashborne.lic` for the three derived
names (`.empath`, `.phase.<p>`, `.tmp`). Every writer and reader was read at the cited line.

**`<L>`** is the bus key, `group_bus_leader_name` :14132: the leader's own name when
`$ashborne_group_ai == "1"` (leader), else `$ashborne_captain` (follower, `"2"`). So each group has
its own set of files and two groups on one machine do not collide. `$ashborne_group_ai == "0"` is solo.
**Every record carries the session id and an epoch, and a reader discards it when either is wrong.**

### A. The coordination files

| # | File (`data/ashborne/`) | Fields, in order | Writer | Readers (what reads it, and the rule) | Freshness | Decision it serves |
|---|---|---|---|---|---|---|
| 1 | `<L>_ashborne_group_session.txt` | `session_id` (`<epoch>-<6 digits>`) `\|style` (combat style 1-9) `\|members` (comma list) | leader, `begin_group_session` :14800 (also clears danger and rest hub) | all: `group_session_data` :14775, `group_session_names` :14790, followers take their session id from it `current_group_session_id` :14818; the next-cycle launcher :19950 | none (it is the epoch) | which group run is current; the roster every other file is checked against. The stale-heartbeat check ignores the first 12 s of a session :20156 |
| 2 | `<L>_ashborne_state_<member>.txt` | 21: `name\|room\|epoch\|health%\|mana%\|stamina%\|flags\|rt\|castrt\|group_ai(0/1/2)\|area\|style\|session\|startup_ready\|encumbrance%\|encumbrance_text\|mind%\|max(wound,scar)\|wound_rank\|scar_rank\|sitting`. `flags` (:14833) = dead, stunned, webbed, bound, disarmed, recovering, swallowed, weapon_fire, shadow_crossing, prone, bleeding, loot_full, looting | each member, `publish_group_state` :14874, every ≥1 s, or ≥0.5 s when changed | `read_group_state` :14918, used by: follower ready at start :15409-15416; looter eligibility :15060; balanced looter :15084; move gate :17953; stand wait :10939; rest-until blockers :20239; stun recovery :20489; adjacent rescue :15990; rank-1 cleanup :18273; interrupt responder readiness :8021; stale heartbeat :20149; disarm pause :14954; captain swallowed :21000; town empath pass :18419; anti-poach exclusion :15885 | 5 s (loot), 10 s (move, rest), 12 s (heartbeat), 15 s (town) | **the member telemetry**: every group decision reads it |
| 3 | `<L>_ashborne_group_room.txt` | `room_id` | leader, `publish_group_leader_room` :20735, on change | follower `group_follow_sync` :20806: leader not visible → `go2` there if the route is short (a distant route is refused), same room → rejoin | none | follower catch-up |
| 4 | `<L>_ashborne_group_ready.txt` | `ready(0/1)\|room\|epoch` | leader :20753; `1` when no configured follower is missing from the room (:11460, :29286, :32413), `0` at :29145 | follower `leader_group_ready_for_followers?` :20774 → `group_follower_captain_present?` :21067 → `group_follower_should_hold?` :21076 | 30 s | followers do not engage until the leader has declared the room ready |
| 5 | `<L>_ashborne_group_target.txt` (flock) | `session\|room\|target_id\|target_name\|epoch` | leader `publish_group_combat_target` :14352 (2 s dedupe; skips dead, gone and hazards); cleared on room change :14290, :32418; manual-leader mode captures the leader's own attack line :14308-14349 | follower `shared_group_combat_target` :14391 → `sync_group_combat_target` :14415 sends `target #id` | 20 s, same room and session | **assist**: all hit the leader's target |
| 6 | `<L>_ashborne_group_looter.txt` | `name\|epoch\|session` | `assign_group_looter` :15040, from `coordinate_group_looter` :15133 (leader, or anyone passing the turn on) | every member, `group_looter?` :15224 via `current_group_looter_record` :15022 | 30 s | **one looter per corpse set**. Modes (`$ashborne_loot_rotation_mode`): `round_robin` by fewest confirmed loots :15100, `balanced` by lightest encumbrance with a hand-off meter :15084. 10 s turn timeout passes the corpse on :15182. All `loot_full` → danger "returning to town to sell" :15170. A member over `$ashborne_loot_emergency`% → danger :15142 |
| 7 | `<L>_ashborne_loot_receipt_<member>.txt` (flock, 0600) | `session\|epoch\|name\|room\|corpse_ids\|confirmed_count` | the looter: ids after a clean loot :9769 via :11033; `+1` on each `You search (the\|a\|an) X.` line :11069-11074 | `confirmed_loot_count` :11057 ← round robin :15108 | session | fair loot turns |
| 8 | `<L>_ashborne_group_danger.txt` | `name\|reason\|room\|epoch` | anyone, `publish_group_danger` :20103; a `stunned` report never overwrites another reason :20105 | `current_group_danger` :20121 (16 call sites); `apply_group_danger_abort` :20268: any reason but `stunned` sends **everyone** home (`$ashborne_action = 99`); leader rescue :15990, :16115; dead handling :16137-16175; stun recovery :20489 | 90 s | **group emergency broadcast**. Reasons and triggers in §B |
| 9 | `<L>_ashborne_group_spells.txt` (**not** locked) | lines `combat_key\|spell_key\|caster\|epoch`; `combat_key` = `room:sorted target ids` :20628 | anyone, `group_spell_claim` :20685 | anyone, `group_spell_claimed?` :20678 (room scope compares the room only) | 45 s | **one caster per fight** for an opener, AoE, control or rescue. Keys (grep of literal claims): `610`, `shield_throw`, `1614_open`, `1615`, `1117`, `1120`, `335`, `917`, `1207`, `1008`, `1017`, `118_open`, `undead_hold_301`, `emergency_213`, `rescue_<name>`, `dead_rescue_130_<name>`. `group_noise_ready?` :20702 waits up to 4 s for another member's opener, then 1.5 s |
| 10 | `<L>_ashborne_group_move_<member>.txt` | `safe(0/1)\|room\|epoch` | each member :17916/:17937; safe = no flags, no rt/castrt, outside the 12 s "vulnerable" window (`mark_group_vulnerable` :17897), no targets | leader `group_member_move_status` :17953 ← `group_wait_for_followers_to_move` :17998 (30 s). Falls back to #2, then to "visible in clear shared room despite stale telemetry" :17971 | 10 s | the leader does not walk away from a busy follower |
| 11 | `<L>_ashborne_anti_caster_event.txt` (flock) | `event_id(room:target:epoch)\|room\|target_id\|target_name\|epoch` | any member whose DownstreamHook sees a creature begin a cast (:16480, pattern :16474) | `current_anti_caster_event` :17697 | 8 s (`ANTI_CASTER_EVENT_TTL` :5028), same room | shared detection of a casting creature |
| 12 | `<L>_ashborne_anti_caster_claim.txt` (flock) | `event_id\|responder\|status(claimed/sent/failed)\|ability\|epoch\|failed_names` | `process_anti_caster_event` :17790 | the same | claim held 3 s; failures excluded | **one interrupter per cast**, ranked by ability priority +40 physical +30 for the elected interrupt job :8045-8061 |
| 13 | `<L>_ashborne_group_plan.yml` | `version, generated_at, session_id, leader, hunt_mode, members, assignments{job → primary, primary_score, backup, backup_score}`; jobs in election order `guardian interrupt healer control dispel scout damage` :7825-7865 | leader `publish_group_plan` :7867 (tmp + rename) | `current_group_plan` :7883: warcamp mode :7218, interrupt bonus :8047, rank-1 healer :18259, town empath :18367, `;ashborne plan` :7916 | 3600 s | **job election** from members' capability files |
| 14 | `<member>_ashborne_capabilities.yml` (**not** `<L>`-keyed) | `version(2), scanned_at, character, profession, level, role_preference, resource_reserves{mana 40, stamina 20, health 70}, equipment{right, left, using_shield}, skills{10}, known_spells, known_combat_abilities{cman, shield, armor, weapon, feat}, anti_caster_abilities[{family, id, name, priority, purpose, target_type}], role_scores` :7463-7535 | owner, `;ashborne setup capabilities` (sends `skill full`, `cman/shield/armor/weapon/feat info` :7468) | leader for the plan :7609; responder ranking :8045; `known_visible_empath` :18991; mana-share partner (SMC ≥ 24) :19559; rank-1 healer :19521; **its existence marks a character the same player runs** :7114 | 45 days (:5027) | who can do what; role scores :7391-7424 |
| 15 | `<member>_ashborne_ability_rules.yml` | per ability `{enabled, min_targets, max_targets, only_creatures, never_creatures}` + `crowd_control{enabled, min_targets 3, max_targets}` :7054-7093 | owner (GUI) `save_ability_rules` :7170 | `ability_allowed?` :7258, `rule_matches_room?` :7185; existence also marks a local character :7119 | mtime | per-ability room rules |
| 16 | `<L>_ashborne_town_service_request.txt` | `session\|epoch\|heal\|sell\|reunion_room\|waggle\|token(<session>-town-<cycle>)\|healer` | leader `publish_town_service_request` :14601 (deletes the rest hub first); called :6432, :19811, :20380 | followers `current_town_service_request` :14694 | 900 s | **the town order**: heal? sell? spell-up? where to meet; who heals first |
| 17 | `<L>_ashborne_town_service_<member>.txt` | `session\|epoch\|name\|room\|status`; status ∈ `ready`, `blocked`, `blocked:<token>:<reason>`, `healed:<token>`, `returned:<token>` | follower `publish_town_service_receipt` :14716 (also the Intel worker :2366-2381, keyed by `$ashborne_captain`) | leader `follower_town_service_receipt_rc13` :19102, `wait_for_town_service_group_rc13` :19134 (watchdog `$ashborne_service_watchdog`, default 180 s), phase barrier :19230 | 900 s | per-member town progress; a `blocked` member stops the repeat queue :19150 |
| 18 | `<L>_ashborne_town_service_request.txt.phase.<healed\|returned>` | `token\|phase` | leader, when every receipt shows the phase :19258 | followers :19277 | token | **phase barrier**: nobody sells before everyone is healed; nobody leaves before everyone is back |
| 19 | `<L>_ashborne_town_service_request.txt.empath` | `token\|done` | the group Empath when its pass ends :18488, or a waiting member once the healer has been missing 45 s :18440 | all :18421-18423 (wait ≤180 s) | token | **Empath heals the group before herbs** (`$ashborne_town_empath_first`, default 1) |
| 20 | `<L>_ashborne_town_checkin_<member>.txt` | `session\|token\|epoch\|name\|phase` | leader, every 10 s to a follower whose ack is missing :14460, :19263 | follower `answer_town_service_checkin` :14487 re-sends its receipt, and `join <captain>` for `returned` | 30 s | nudge a lagging follower |
| 21 | `<L>_ashborne_group_rest_hub.txt` | `session\|token\|epoch\|room` | leader `publish_group_rest_hub` :14626 (called :19678) | followers `current_group_rest_hub` :14680; runtime phase "resting, awaiting leader" :14866 | 900 s | where the group sits out the rest after town |
| 22 | `<L>_ashborne_rank1_heal_request.txt` | `session\|request_id\|epoch\|room\|healer\|members` | leader `run_group_rank1_wound_cleanup` :18273, between fights, for members whose wound rank is exactly 1 | the named Empath `process_follower_group_rank1_heal_request` :19510 | 30 s | **field healing** between fights (`$ashborne` setting `rank1heal`, default 0 :8361) |
| 23 | `<L>_ashborne_rank1_heal_<healer>.txt` | `session\|request_id\|epoch\|name\|room\|status`; status ∈ `complete`, `partial-reserve-or-safety`, `blocked-capability`, `blocked-error` | the Empath :14562 | leader, waits ≤18 s :18314-18325 | 30 s | heal done, move on |
| 24 | `<L>_ashborne_shutdown_request.txt` | `session\|epoch\|heal\|sell\|waggle` | leader :14731 (called :21272, :21277) | followers :19460: heal, sell, ewaggle as asked, kill, write #25, exit | 900 s | orderly group stop |
| 25 | `<member>_ashborne_cycle_stopped.txt` | `session\|epoch\|name` | follower :14766 (and the Intel worker :2347-2360) | the leader's next-cycle launcher :19959 waits for every member (≤600 s old, 5-minute deadline) before `;ashborne <style> <area> cycle=N hunts=M` | 600 s | **repeat hunts**: the next cycle starts only when every follower has stopped |
| 26 | `<member>_ashborne_runtime.txt` | 13: `epoch\|name\|room\|phase\|detail\|cycle\|cycles\|health%\|mana%\|mind%\|session\|phase_seconds\|timings` | each member `publish_runtime_status` :10832 (phases :14855: danger, looting, return, travel, combat, loot-wait, resting, hunting, town-checkpoint, town-watchdog) | `;ashborne status` globs every `*_ashborne_runtime.txt` :10879; leader `follower_activity_explanation` :10812 (≤15 s) explains a missing ack and decides a follower needs a restart :10824 | 15 s | **live status of every character**, and stale-follower recovery |

**Local, not coordination:** `<member>_ashborne_spell_metrics.yml` :1922 (per spell and creature: damage,
kills, control, hits, wards, no_effect, from observed casts :1936-1957, "adaptive spell learning");
`<member>_ashborne_support_<ts>.yml` :6993 (diagnostic dump, embeds the group plan :7025);
`<member>_ashborne_runtime_errors.log` :6796 and `_intel_ashborne_runtime_errors.log` :2472 (capped at
256 KiB, keeps 200 lines). Imports: `data/uberbeta/*_uberbeta_*` seeds a missing file once :516-522;
`data/huntpro/<name>_huntpro_capabilities.yml` :7045 and HuntPro's settings `import_huntpro_settings` and `import_missing_huntpro_group_settings` :7616-7700.

### B. The emergency reasons written to the danger file (#8)

MEASURED: `grep -n "publish_group_danger(" Ashborne.lic` = 31 call sites. From the per-tick check
`ashborne_group_status_check` :28608-28746 unless noted: **dead** (:28620, :15643, :16058); **low mana**
≤10% when mana sharing is on (:28634); **low spirit** ≤10% (:28646); **encumbered** ≥
`$ashborne_value_encumbrance` when loot rotation is off (:28655); **stunned** (:20486, a local-recovery
reason, never a retreat); **webbed** (:28666); **bound** (:28671); **poisoned** when 114 is not
castable (:28682); **diseased** when 113 is not castable (:28693); **low health** ≤50% with no Empath
`cure` and no herbs (:28712); **major injury**, wound or scar rank ≥2 with no herbs (:28733, :28742);
**stranded in gas cloud** (:17402); **`<name>` needs medical or emergency recovery after stun** (:20515);
**leader unavailable with hostiles; withdrawing** (:21047, a follower 30 s after losing its leader);
**individual travel failed** (:8813); **medical retreat during individual travel** (:21434); **noise
lockdown unavailable** (:8223); encumbrance emergency and all looters full (:15142, :15170); and the
free-text `$ashborne_return_why` of critical mana ≤5% (:7754), warcamp swarm (:14254) and others (:6217,
:6429, :9559, :9844, :16818, :20038, :20313). The leader also treats a **stopped heartbeat** as danger:
a visible follower whose state is >12 s old and who has not been seen attacking in 12 s (:20149-20176)
sends the group home (:20294-20301).

### C. The command channel (not files)

`send_remote_group_command` :15443 sends a Lich command to another character's Lich through
`thunder` (`;thunder <name> <cmd>`), Borg (`$commands.push("5 <rand> <name> <cmd>")`) or `queen`
(`$ashborne_multi_account_tool`, default `borg` :15578). Commands sent: `;kill ashborne` then
`;ashborne auto group` (launch or restart a follower, :15474-15480, :15554-15557); `;e …run_individual_trip(<room>)`
(:8881, each member travels alone and meets there, 0.8.5); `;e …rejoin_captain_group` (:8900);
`;e …finish_shadow_valley_crossing` (:8949, :8982); stand (:10929); `;e …recover_shadow_valley_regroup`
and `force_individual_regroup` (:15532, :15545); `;e …group_follow_sync(true)` (:15562); `;go2 <room>`
(:15587); `;queen rally` (:15591). In game: `hold <name>` (:15437), `join <captain>` (:14499, :19365, :19434),
`disband` and `group open` (:19407-19408, :19813-19814), `drag <name> <dir>` and `pull <name>` (:16073, :16083, :15951).
Observed game text used as signals: `<Name> is unable to follow you.` (:10912) and a follower's own
attack lines as a heartbeat (:17196).

### D. What this means for Hydra: the requirements, and what Cena has

All 26 files exist because the characters are in **different processes**. In Hydra they are
sessions in one `cena_host::Host` (`crates/cena-host/src/table.rs:103`), so most files become a
read of another session's state, not a protocol. What is left is **what a group decides**. Searched
Cena for any cross-session behavior: `grep -rn "leader\|follower\|looter\|assist" crates/cena-behavior/src crates/cena-host/src`
finds none, and `plan/30` §4 records groups as "recorded now, built after solo".

| Req | From files | What Hydra needs | Cena today | Verdict, value |
|---|---|---|---|---|
| R1 a group run | #1, #24, #25 | a group object: leader, members, an epoch; start, stop and repeat all together | `Host` holds sessions and stops them all (`stop_all`, `table.rs`); no group | **GAP**, HIGH after solo M6 (`plan/30` §4) |
| R2 member telemetry | #2, #10, #26 | read any member's vitals, mind, encumbrance, rt/castrt, statuses, worst wound/scar, sitting, room, and its **hunt phase** | the model has every game fact: vitals `state/vitals.rs`, encumbrance % and text `state/character.rs:226-230`, rt and cast rt `state.rs:146-167`, statuses incl. `bound`, `sitting`, `webbed` `status.rs:187-233`, wounds `state/character/body.rs`; hunt phase `hunt/said.rs`. Not published across sessions | **PARTIAL**: facts HAVE, the behavior flags (disarmed, recovering, swallowed, looting, loot_full, ready) are not a shared record. HIGH |
| R3 follow and gate | #3, #4, #10 | followers go to the leader; followers hold until the leader says the room is ready; the leader does not move while a member is busy | none | **GAP**, HIGH |
| R4 assist | #5 | one shared target; followers `target #id` it | per session `state/targeting.rs` (current target) | **GAP** (the sharing), HIGH |
| R5 loot election | #6, #7 | one looter per corpse set; round robin by confirmed count, or lightest encumbrance; 10 s turn timeout; stop when all full | per session loot planner `loot/plan.rs`; the ledger records searches `state/ledger/hunt.rs` | **GAP**, HIGH |
| R6 danger broadcast and rescue | #8, §B | any member's emergency sends the group home; leader drags a stunned or webbed member out of an adjacent room; a dead member stops the hunt | solo survival only: dead stops, down stands (`hunt/engine.rs:244-258`) | **GAP**, HIGH |
| R7 claims | #9, #11, #12 | one caster per opener, AoE, rescue; one interrupter per creature cast | none | **GAP**, MEDIUM |
| R8 creature-casting detection | #11 | know a creature is casting | bestiary `spell_prep` messages, 281 rows (`cut -f2 crates/cena-model/data/creature_messages.tsv \| sort \| uniq -c`), matched by `state/creature_message.rs` | **PARTIAL**: per-creature texts HAVE; Ashborne's generic verb pattern (:16474) is not ported. MEDIUM |
| R9 capabilities and jobs | #13, #14, #15 | who is healer, interrupter, guardian; role scores from skills, spells, abilities | skills `state/character/skills.rs`, known spells `state/known_spells.rs`, PSM `state/character/psm.rs`, profession | **PARTIAL**: inputs HAVE, election GAP. MEDIUM |
| R10 town barrier | #16-#21 | a group town round: heal (Empath first), then sell, then regroup, each phase a barrier, one rest room | solo selling round `town/plan.rs`; no heal behavior (M6d) | **GAP**, HIGH once groups and M6d exist |
| R11 field healing | #22, #23 | the Empath cleans rank-1 wounds between fights, within a mana reserve | none (M6d not built) | **GAP**, HIGH for M6d |
| R12 status board | #26 | every character's phase, room, HP/mana/mind, hunt n of m, time per phase | the hub's card per character (`plan/29` §6, `crates/cena-web`) | **PARTIAL**: no hunt phase or per-phase timings on the card. MEDIUM, M7 (an agent reads the same) |
| R13 room claim with a group | #2 (:15885), #14 (:7114, :22196) | a grouped member, or **any character this Hydra runs**, is not a stranger in the claim; two of one player's hunters that meet: the lowest name keeps the room (`local_collision_holder?` :7125) | `claim_room(&room, with_me)` takes the roster (`state/claim.rs:181`); the engine passes the game group's nouns (`hunt/engine.rs:547-555`) | **PARTIAL**: the hosted characters are not in the roster. HIGH (it is the one line that stops two of the author's own characters leaving each other's rooms) |
| R14 leader loss | follower 30 s rule :20985-21050 | a follower whose leader vanished defends the room for 30 s, loots only if uncontested, then withdraws | none | **GAP**, HIGH: it is the author's recorded reconnect design (`plan/30` §4) in running code |

## Special ask 2: decision rules these engines have that bigshot's 87 words and Cena's profile lack

**The baseline.** Cena's profile is `crates/cena-behavior/src/hunt/profile.rs:70-96` (rooms, stance
×3, rest {fried, overkill, encumbered, mana_below, until {experience, mana, spirit, stamina}, when
{bleeding, health_at_most, cannot_cast, cannot_use_ranged}, commands}, prepare, signs, ignore, flee
{count, from}, loot {script, delay, defensive, box_in_hand}, wander {wait}, targets, routines,
sequences); five guards are built (`hunt/guard.rs:59-65`) and 87 bigshot words are evaluated in
`plan/33`. bigshot's own settings (not guard words) are `bigshot.lic:3423-3558`; the importer reads
the keys `plan/30` §4 lists and names the rest as not imported (`hunt/import.rs:40-47`). A "bigshot"
column entry of **setting** means bigshot has it outside the guard vocabulary and Cena's importer drops
it. Sources: this file's own reading, and three scratch extractions in `tools/1-hunting-engines/`
(`ashborne-captures.md`, `huntpro-family.md`, `bigshit2-gshunting4all.md`), each row of which was
cited to a line; the rows below were spot-checked against the scripts.

**Two findings about Cena's existing rules come first, because they are bugs rather than gaps:**

1. **`invalid_targets` is imported with the wrong meaning. CONFLICT, HIGH (M6 import).** In bigshot it
   is "flee if enemy count is > N **but don't count these**" (`bigshot.lic:3483-3484`), read only in
   `should_flee?` (`:8579-8583`); `valid_target?` (`:8600-8646`) never consults it, so bigshot still
   attacks them. Cena maps it to `ignore` (`hunt/import.rs:391`), documented as "Creatures never
   attacked" (`profile.rs:82-83`), and `fightable` removes them from both targeting and the flee count
   (`engine.rs:528-544`). A bigshot profile that lists a harmless creature so it does not trigger a
   flee will, in Hydra, never be fought. huntpro's and Ashborne's "ignore" lists mean a third thing,
   **leave the room** (`huntpro.lic:2028-2038`, `Ashborne.lic:9528-9537`), which is Cena's `flee.from`.
2. **A contested room stops the hunt dead. PARTIAL, HIGH (M6).** `engage` returns nothing unless the
   room is claimed (`engine.rs:430`); `wander` refuses to move while `fightable` is non-empty
   (`engine.rs:571-573`); `flee` fires only on count or name (`engine.rs:263-280`). So with a stranger
   and a live creature in the room every tick is `Said::Nothing`, and the `loot` arm (`engine.rs:286-330`)
   has no claim check, so it loots corpses in someone else's room. Every engine here leaves instead
   (`huntpro.lic:645-655`, `Ashborne.lic:9081-9086` and `leave_occupied_room` :22187, `shunt.lic:570`).
   INFERRED by reading the tick order (`engine.rs:207-228`), not run. Related, MEDIUM: Cena re-asks the
   claim every tick, while Lich decides it on room arrival (`reference/lich-5/lib/gemstone/claim.rb:87-101`,
   noted at `claim.rs:180-185`) and huntpro and gshunting4all keep fighting once engaged
   (`huntpro.lic:703-727`, `gshunting4all.lic:13135-13175`); a player walking in mid-fight would leave
   Cena's hunter standing while it is attacked (INFERRED: assumes `room players` updates mid-room).

### Rest and return

| Rule | Engines (script:line) | bigshot | Cena | Verdict, value |
|---|---|---|---|---|
| **Rest when spirit ≤ 10%** | `huntpro.lic:15506-15509`, `Ashborne.lic:28495-28498` (group: :28646) | no (`v N` is a step guard) | `rest.until.spirit` only (`profile.rs:161-164`); no trigger in `rest_reason` (`rest.rs:142-171`) | **GAP, HIGH** (a spirit-draining hunt kills) |
| Rest on stamina | `huntpro.lic:15496-15504` (≤ 10), `chuntpro.lic:2221-2228`; Ashborne deliberately never, it throttles instead: conserve at reserve+15, bursts need reserve+45 (`Ashborne.lic:7760-7818`) | no | `until.stamina` only | PARTIAL, MEDIUM |
| **Rest on any wound or scar rank ≥ N**; per-part thresholds (head 2, arms 3 ...) and a summed "combo" score | `huntpro.lic:15532-15575`, `Ashborne.lic:28510-28551`, `:20011-20028`; `gshunting4all.lic:580-598`, `:12024-12060` | only by `wounded_eval` Ruby | `rest.when` has bleeding, health, `cannot_cast`, `cannot_use_ranged` (`profile.rs:175-185`); the ranks exist (`state/character/body.rs:139-141` `worst`) | **PARTIAL, HIGH** (M6, and M6d decides when healing is due) |
| **Stunned, webbed or bound: leave the room, or rest** (Ashborne: one-room move once mobile and the group is healthy) | `huntpro.lic:15578-15604`, `Ashborne.lic:20484-20551`, `:28569-28577`; gshunting4all defers its escape until the hold breaks, `:9748-9756` | no | survival only withholds `stand` (`engine.rs:253`); the other arms carry on | **GAP, HIGH** |
| Poisoned or diseased: cure (114, 113) if castable, else rest | `huntpro.lic:15606-15626`, `Ashborne.lic:28579-28598` | step guards `poison`, `disease` | statuses HAVE (`status.rs:231-233`); no trigger; `plan/33` §2b keeps the words | PARTIAL, MEDIUM (M6d) |
| **Two health tiers**: a normal rest, and a lower "abandon the hunt now" tier (Ashborne 60%, gsh4a 35% checked by a 0.3 s thread mid-roundtime) | `Ashborne.lic:19992-20044`; `gshunting4all.lic:6094`, `:12308-12333` | fog settings are the *how*, not a tier | one tier (`rest.when.health_at_most`) | PARTIAL, MEDIUM |
| **An escape ladder** on the way out: Spirit Guide 130, Voln `symbol of return`, Sunfist `sigil of escape`, CoL `sign of darkness`, a safe room; keep enough mana for 130 | `gshunting4all.lic:9744-9915`; `huntpro.lic:18873-18905`, `:15369-15376` (`130_fog`) | **setting**: `fog_return`, `custom_fog`, `fog_optional`, `fog_rift` (`bigshot.lic:3430-3433`) | fog model (`state/fog.rs`); sending half deferred (`plan/20` §0b); costs in `societies/*.rs` | PARTIAL, MEDIUM |
| Critical mana: return at once, mid-fight (5%) | `Ashborne.lic:7744-7758` | no | `rest.mana_below` waits for the tick | PARTIAL, LOW |
| **Wrong equipment**: three "your attack has no effect" in 20 s: go home (need blessed or enchanted) | `Ashborne.lic:2208-2247`, `intel_huntpro.lic:1096`, `chuntpro.lic:5777` (23 sites), `bigshot-bless.lic:11` | bigshot rests on ammo with no effect (`bigshot.lic:6398-6400`) and has a `bless` setting | none | **GAP, HIGH** (undead and noncorporeal hunts) |
| Hunt time limit | `intel_huntpro.lic:114-126`, `Ashborne.lic:2461`, `smarthunt.lic:122` | no | none | GAP, LOW |
| **Number of cycles**, then stop | `Ashborne.lic:600` (`hunt_cycles`, default 1), `:18959-18983`, `stopbigshot.lic`, `killbigshot.lic`; huntpro always stops after one trip (`huntpro.lic:5620-5754`) | only by a resting script | the hunt cycles forever (`rest.rs:67-87`) | GAP, MEDIUM (an overnight run needs an end) |
| Dread: rest at creeping ≥ 20 or crushing ≥ 15 | `gshunting4all.lic:7785-7789`, `:12289-12300` | **setting**: `creeping_dread`, `crushing_dread` (`bigshot.lic:3442-3443`) | named as not imported (`hunt/import.rs:45-47`) | PARTIAL, MEDIUM |
| Overexerted debuff: escape | `gshunting4all.lic:8421-8423` | no | effects hold it by name | PARTIAL, LOW |
| Group rest-until: every member's mind ≤ N and mana ≥ 90 | `Ashborne.lic:20239-20265` | no | solo `until` only | GAP, groups |

### Flee, targets, the room

| Rule | Engines (script:line) | bigshot | Cena | Verdict, value |
|---|---|---|---|---|
| **Room hazard objects** (cloud, void, web, vine but not dreamvine, sandstorm, tempest, swarm, pod, sealed fissure): dispel with 119 (912 for a wizard) or leave; a switch per kind | `huntpro.lic:6049-6292`, `:15793-15802`; `Ashborne.lic:18079-18097`, `:21907-22160`; `chuntpro.lic:2305-2480`; `shunt.lic:406` | **setting**: `flee_clouds`, `flee_vines`, `flee_webs`, `flee_voids` (`bigshot.lic:3491-3494`) | room objects are parsed, used only as "not loot" (`loot/worth.rs:41`); `flee.from` reads creatures only (`engine.rs:270`) | **GAP, HIGH** |
| **Gas cloud forming over me**: escape, avoid the room 20 s, no looting while it is pending | `Ashborne.lic:16594-16602`, `:17315-17345`, `:9627` | no | none (the cloud abilities are bestiary data, `creature_attacks.tsv`) | **GAP, HIGH** |
| Ground about to erupt (boil, frost, stalagmites, debris): leave | `intel_huntpro.lic:1112`, `Ashborne.lic:16539-16544` | **setting**: `flee_message` (any environmental line, `bigshot.lic:3486`) | the attack lines classify (`combat_attacks.tsv:55-59`); no reaction | PARTIAL, HIGH |
| **Hazard "creatures" never targeted** (wasp nest, shimmering fungus) | `huntpro.lic:2016-2026`, `Ashborne.lic:17458-17463` | no | both are ordinary rows (`creatures.tsv:484`, `:602`) and pass `valid_target` (`state/creatures/instance.rs:542-564`); `any = true` attacks them | PARTIAL, HIGH |
| Flee-count exclusions: summoned minions (grik, imp, haze, rouk, brume, mist, smoke, darkling, shadowling ...), severed troll parts, companions | `bigshot.lic:8584-8586` (the baseline itself) | yes, built in | Cena's flee counts every `fightable` creature (`engine.rs:268`); `valid_target` excludes appendages and animates only | PARTIAL, MEDIUM (Cena flees earlier than bigshot in summoner rooms) |
| **Flee count: `>=` or `>`** | `huntpro.lic:2041-2051`, `Ashborne.lic:9545`: `>=` over all targets (their own help text says "more than") | `>` (`bigshot.lic:3483`, `:8590-8592`) | `>` over fightable only (`engine.rs:268-269`) | CONFLICT with huntpro/Ashborne, MEDIUM for anyone importing their values; matches bigshot |
| Flee groups: several of one creature count as one | `shunt.lic:981`, `:409-411` | no | none | GAP, LOW |
| Profession- or level-gated flee (casters avoid Vvrael witches, warlocks, destroyers and greater constructs; flee a ghost wolf below level 13); **a boon adjective** (rune-covered, tattooed, sparkling, shining) | `huntpro.lic:735-1995`; `chuntpro.lic:5878-5920`; `Ashborne.lic:3607-4178` (`CASTER_REJECTION_RULES`, 570 names) | **setting**: `boon_flee_from`, `boons_flee` (`bigshot.lic:3490`, used at `:8572`) | `flee.from` matches a whole name or noun (`engine.rs:642-644`); the boon adjectives are known (`state/creatures/instance.rs:38`) but no rule reads them | PARTIAL, MEDIUM |
| **Prefer a helpless target** (sleeping, frozen, stunned, lying down) when choosing | `huntpro.lic:506-512`, `:596-625`; `Ashborne.lic:9119-9124` | no (per-step guards only) | `choose_target` ranks by list order only (`engine.rs:495-523`); `helpless` is a *step* guard in `plan/33` §3 | GAP, MEDIUM (a ranking rule, which no guard can express) |
| Priority by role (warchief, shaman first; then casters, healers, archers) | `Ashborne.lic:14262-14286` | no | list order | PARTIAL, LOW |
| **Interrupt a casting creature** (one member, the best-ranked ability) | `Ashborne.lic:16460-16509`, `:17790-17850` | no | 281 `spell_prep` lines in `creature_messages.tsv`; the matcher `state/creature_message.rs:245` has no caller outside tests | GAP (rule), PARTIAL (signal), MEDIUM |
| Creature level against mine: CC only where the area is not trivial; keep wander out of rooms more than 2 levels above the target | `huntpro.lic:2498`; `huntplan.lic:3193` | no | no guard compares levels (`plan/33`) | GAP, LOW |
| **Leave a contested room**, and do not loot there | see the finding above | yes: `bigclaim?` (`bigshot.lic:7091-7099`) is `Claim.mine?` **plus no stranger's floating disk** (unless `ignore_disks`); looting needs it (`need_to_loot?`, `:7808`); a room is claimed on entry and kept while fighting (`bigclaim? \|\| !new_room`, `:9392`) | stays idle; loots; a stranger's disk does not contest the room (`state/claim.rs` reads players and the hiding sign only; disks are modelled in `state/disk.rs`) | PARTIAL, HIGH |
| **Two of this player's own hunters meet**: the lowest name keeps the room, the other moves on | `Ashborne.lic:7114-7131`, `:22196-22215` | no | `claim_room`'s roster is the game group (`engine.rs:547-555`) | **GAP, HIGH** for Hydra, whose whole point is several characters in one process |
| Skip a room with a hidden someone | `shunt.lic:570`, `Ashborne.lic:2869` | yes (claim) | `claim.rs:120` | HAVE |

### Stance, emergencies, groups and class rules

| Rule | Engines (script:line) | bigshot | Cena | Verdict, value |
|---|---|---|---|---|
| **Kneeling to fire a crossbow** (styles 7 and 8) | `huntpro.lic:3526`, `:3536`, `:449-452` | step guard `k` (`plan/33` §2a: `self_kneeling`) | survival stands whenever kneeling (`engine.rs:249-257`), so a crossbow routine that kneels is undone the next tick | CONFLICT, MEDIUM |
| Do not try to leave offensive while Frenzy (216), Song of Rage (1016) or Zealot (1617) is up | `chuntpro.lic:2282-2303` | no | the stance setter would resend a stance the game refuses (`stance.rs:85`) | GAP, MEDIUM |
| Stay defensive until something attacks me | `chuntpro.lic:5742-5756` | no | attacks on me are classified (`state/combat/attack.rs`); no rule | GAP, LOW |
| **Self-disarm: recover the item**; hold the group while a member recovers; Minor Sanctuary (213) if two or more hostiles remain | `Ashborne.lic:16855-16864`, `:17099-17119`, `:16957-17030`; `bountyhunter-support.lic:95-145` | no | none; Lich has the lines (`combat/defs/messages.rb:89-97`) | **GAP, HIGH** |
| **Weapon Fire**: drop the burning weapon, no looting until it cools, then `gird` | `Ashborne.lic:16646-16795` | no | data only (`creature_messages.tsv:3132-3133`), matcher unwired | PARTIAL, HIGH |
| **Sanctuary room**: the game refuses the attack; leave | `intel_huntpro.lic:544-559`, `Ashborne.lic:2878-2896`, `bountyhunter-support.lic:174` | no | none; `plan/33` §2b's `nomagic` covers only the map tag | **GAP, HIGH** (the engage arm would resend forever) |
| Confused: `run`, pause | `intel_huntpro.lic:649-661`, `Ashborne.lic:2976-2988` | no | no status | GAP, MEDIUM |
| Swallowed (Belly of the Beast): stow, attack the stomach wall with a dagger | `huntpro.lic:15271-15287`, `Ashborne.lic:15760-15772` | no | none | GAP, MEDIUM |
| A spell misfires or diverges: do not recast it for 180-270 s | `intel_huntpro.lic:622-690` | no | Maintain retries a sign every 60 s whatever the reason (`engine.rs:57`, `:361-367`) | GAP, MEDIUM |
| Loot safety: no loot during a cloud, weapon fire or medical danger; wait over a corpse that may re-form | `Ashborne.lic:9627-9645` | no | loot runs whenever a corpse is present (`engine.rs:286-330`) | PARTIAL, HIGH |
| **Healing in the field**: Empath self-heal at wound ≥ 1, `cure` at health ≤ 50, herbs, whisper the group Empath | `huntpro.lic:15473-15545`, `:15521-15529`; `Ashborne.lic:28469-28551` | no | none (M6d) | GAP, HIGH (M6d) |
| Group rules | special ask 1, §D (R1-R14); huntpro's in-game forms: `hold` each member and wait (`huntpro.lic:16107-16343`), a follower attacks when it sees the leader attack (`:19050-19148`), mana sharing (`:15348-15357`), 108 on a stunned member and `pull` a prone one (`:16361-16398`) | **setting**: `independent_travel`, `independent_return`, `group_deader`, `ma_looter`, `never_loot`, `random_loot`, `final_loot` (`bigshot.lic:3540-3546`), all dropped | none | GAP, after solo M6 |
| Per target, at most once per N s (Mana Leech: once per creature per 90 s); **up to N times per target** (Bind then kill: 3) | `Ashborne.lic:12538-12552`, `:11735-11780` | `once`, `room`, `repeatdelay` | `plan/33` §2g has `once`, `once_here`, `every N`, none per target and counted | PARTIAL, LOW |
| A guard on the creature's **blood or bones** (Empath: Bone Shatter needs bones, blood spells need blood) | `Ashborne.lic:12696`, `:13062` | no | `creatures.tsv` has `blood` and `bones` columns; no guard in `plan/33` | GAP (new word), MEDIUM |
| A guard on **my own** stun (a Warrior berserks when stunned with stamina ≥ 50) | `huntpro.lic:15578-15594`, `Ashborne.lic:28553-28566` | no | `plan/33` §2b's self words are hidden, disease, poison, outside, splashy, alone, nomagic | GAP (new word), MEDIUM |
| A **spell immunity** guard (skip 501 on a creature immune to it) | `huntplan.lic:1939-1985`, and the per-creature spell overrides (`Ashborne.lic:11789-12202`, `huntpro.lic:3576-4163`) | no | `creature_lists.tsv` holds 18 immunity rows; no guard reads them | GAP (new word plus data), MEDIUM |

**In one line:** beyond bigshot's words and Cena's profile, these engines add **rest triggers**
(spirit, stamina, wound rank, stun, web, bind, poison, disease, a second health tier, wrong
equipment, a cycle count), **environmental flight** (hazard objects, gas clouds, ground traps,
sanctuaries, hazard creatures), **target ranking** (helpless first) and **claim behaviour** (leave, do
not loot, let one's own other character have the room), plus three guard words `plan/33` does not
have: blood/bones, my own stun, and spell immunity.

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| `huntplan.lic:68-611` (Mertyn, v1.0, no license line) | `creature_list`: creature → level, spawn ids | 543 rows, 548 names, 12 with `nil` level (`huntplan_levels.py` in `tools/1-hunting-engines/`) | names, level, spawn_ids | `crates/cena-model/data/creatures.tsv` (627 rows), `level` column 4 | **HAVE** for 517 names at the same level. **CONFLICT** on 7: shelfae guard 8 vs Cena 7, centaur ranger 23 vs 25, wind wraith 61 vs 63, direbear 64 vs 65, monstrous direwolf 67 vs 68, ilvari sprite 73 vs 72, ithzir herald 93 vs 92 (Cena's are from Lich's templates; neither is authoritative, the wiki would settle it). **PARTIAL**: wharf rat has level 1 here, blank in Cena (`creatures.tsv`, `wharf_rat` row). **GAP**: `shadowy spectre` (14), `shan sorcerer` (58), `infernal lich` (110) are in `creature_messages.tsv` but have no `creatures.tsv` row; `gnoll priestess` (20) is nowhere. The other 8 misses are naming variants (glittering crystal crab / crystal crab, shan bard / shan bardess, arachne priestess / arachne priest) | LOW; MEDIUM for M8 bounty lookups |
| `huntplan.lic:614-1924` | `spawn_index`: spawn id → Lich room ids | 1,307 keys, 8,810 distinct rooms (`awk 'NR>614 && NR<1925' huntplan.lic \| grep -oE "\[[0-9,]+\]" \| tr -d '[]' \| tr ',' '\n' \| sort -u \| wc -l`) | spawn_id, [room_id] | `crates/cena-model/data/creature_areas.tsv` (1,394 rows: creature, area name, uid range) | **PARTIAL**: Cena places a creature by area name and uid range; huntplan by explicit room ids, which is what a hunting-ground builder needs (`build_spawn_locales` :2455, `build_wander_rids` :2565, and `safe_room_level` = max(char level, creature level) + 2 :3193 to keep wander out of rooms whose creatures are too high). The spawn-id provenance is not stated (INFERRED: a room uid or a spawn-region id) | MEDIUM, M8 bounty (huntplan is a bounty hunt planner) |
| `huntplan.lic:1939-1985` | creature immunities and weaknesses, observed | 25 immunities, 2 weaknesses, plus "trolls are weak to fire" (:1985) (`grep -c "\.immunities\.push"`) | creature, spell number or element | `crates/cena-model/data/creature_lists.tsv`, list `immunities`: 18 rows | **GAP** for ~21: lesser vruul (501, steam, fire, acid, cold, lightning), storm giant 710, stone troll fire, wood wight 1002, Illoke mystic 909, myklian cold, earth elemental 501 and 909, greater earth elemental 501, treekin druid 501, treekin warrior 501, krag dweller 501, fire rat steam, frost giant weak to fire. **HAVE**: fire rat fire, fire cat fire, krag dweller fire; dark vortece 501 is covered by Cena's broader `magic`. Three are soft ("516 yields very little mana" :1941-1943) | MEDIUM: a caster routine choosing a spell (M6 guards) |
| `huntingareas.lic:9-47` (Salasin, v1.0.4) | bigshot hunting grounds | 38 rows (`grep -c "^\s*\[\['" huntingareas.lic`) | [level+flag: creature], town room, hunting room, boundary rooms, flags `:no_boss_bounties`, `:no_rescue_bounties`, `:interior_rooms`, `:dont_get_spellups` | none: Cena's profile holds one hunting room and its boundaries (`hunt/profile.rs:101-110`); no catalogue | **GAP**. Note the source has a data bug: row 2 is `2300, 3214 [3205]` (missing comma), which Ruby reads as `3214[3205]`, a bit test that yields `0` | MEDIUM, M8 bounty; LOW otherwise |
| `hw_hunter_attack.lic:358-402` (Alastir) | Hinterwilds creatures: armed or unarmed, and which maneuver per profession | 13 creatures | name, armed?, cman list | `creature_attacks.tsv` physical attacks name weapons (e.g. `brawny_gigas_shield-maiden ... handaxe`) | **PARTIAL**: "armed" is derivable from the weapon attacks; not a column | LOW |
| `Ashborne.lic:5097-5629` (NecroDeus, no license line) | `HUNTPLAN_ZONE_CATALOG`: hunting zones | 531 zones (452 Living, 79 Undead), levels 1-110, 21,839 room ids (count by the scratch extraction, `ashborne-captures.md` D2) | slug → [level, Living/Undead, start room, [rooms], [creature names]] | `creature_areas.tsv` (area name, uid range), `creatures.tsv` level and `undead` | **PARTIAL**: no curated start room and room list per creature (INFERRED: the uid ranges can yield rooms through `cena-map`, unmeasured) | HIGH, M8 (bounty: where to hunt), and M6 profile creation |
| `Ashborne.lic:749-1877` | `BOUNTY_RULES_0/1`: bounty sentence → hunt | 1,124 regex rows (743 creature, 255 skin, 26 skin-with-area) | [regex, kind, [short name, names, area code or skin, type, stage]] | `state/bounty.rs:374`, `:408`, `:422` parse any bounty generically, with `creature` and `area` captures | **PARTIAL**: reading HAVE and better (one regex, not 1,124); the area → ground mapping GAP | HIGH, M8 |
| `intel_huntpro.lic:1480-13247` (huntpro, no license line) | bounty creature/place table | 282 creature-in-place rows (× rescue and cull forms), 281 skin rows, 118 area codes, 108 skin names | sentence → creature, target list, area code, type, skin | as above | **PARTIAL** (same) | HIGH, M8 |
| `huntpro.lic:11859-14140` | hunting-area catalogue | 254 areas (176 Living, 78 Undead), 11 with a spawn delay | code → level, type, entry room, spawn delay | as above | **PARTIAL** | MEDIUM, M8 |
| `Ashborne.lic:3607-4178`; `huntpro.lic:735-1296` | creatures a pure caster should avoid | 570 names (Ashborne; 418 avoid, 152 wizard may fight); 568 (huntpro) | name → [index, wizard may fight] | `flee.from` per profile; Vvrael rows in `creatures.tsv` | **PARTIAL** (mechanism HAVE, knowledge GAP) | MEDIUM |
| `Ashborne.lic:6749-6770` | anti-caster abilities | 14 spells + 3 skills | [spell, name, priority 64-100, purpose, any/undead] | none | **GAP** | MEDIUM (interrupts) |
| `Ashborne.lic:5059-5089` | crowd-control spells with default minimum targets | 29 | spell → [name, min targets] | `mob N` in `plan/33` §2g | covered by plan/33; the defaults GAP | LOW |
| `Ashborne.lic:5090-5092` | Grimswarm warcamp rooms, entry rooms, shroud-sensitive spells | 16, 2, 28 | room ids, spell numbers | none | **GAP** | MEDIUM (a warcamp hunt) |
| `Ashborne.lic:17458-17460`; `huntpro.lic:2016-2026` | never-target "creatures" | 2 (`wasp nest`, `shimmering fungus`) | name | ordinary rows, `creatures.tsv:484`, `:602` | **PARTIAL** | HIGH (M6) |
| `Ashborne.lic:18107-18126` | rank-1 wound healing: appraisal part → Lich part; part → spell and cost tier | 14 parts, 4 tiers | part, [wound spell, cost, scar spell, cost] | none (UNVERIFIED oddity: 1107-1110 in `spells.tsv` are Adrenal Surge, Empathy, Empathic Focus, Empathic Assault, so the "scar spell" numbers read as tiers) | **GAP** | MEDIUM, M6d |
| `huntpro.lic:1300-1995` | boon "boss" traps | 4 prefixes × 76 bases | adjective + base | boon adjectives `state/creatures/instance.rs:38`, `gameobj-data.tsv:86` | **PARTIAL** (known, no rule) | MEDIUM |
| `gshunting4all.lic:1310-1364`, `:9674-9679` (Falvicar, no license line) | `SOCIETY_DATA`: society abilities with ranks; the "society heal" | 28 abilities | name, cmd, buff, desc, rank, spell_id | `state/societies/voln.rs`, `sunfist.rs`, `col.rs`, each checked against the wiki | **CONFLICT, do not port**: ranks wrong nearly everywhere (Voln Protection 3 against Cena's 6, `voln.rs:236-240`; all twelve CoL ranks), and the heal sends `sign of wracking` (5 spirit, restores mana) for CoL and `sign of healing` (a CoL sign) for Sunfist (VERIFIED by reading `:9674-9679`) | MEDIUM, M6d (port society heals from `societies/*.rs`) |
| `gshunting4all.lic:3604-3653` | wizard's offensive spell list | 39 | num, label, cmd | `spells.tsv` | **CONFLICT, do not port**: e.g. "1628 Judgment" (Judgment is 1630), "1630 Divine Wrath" (335) | LOW |
| `gshunting4all.lic:605-919` | `SCRIPT_TEMPLATES`: scripted weapons and items and their activated abilities | 31 item types, 85 ability rows, each type citing its gswiki page | type: max tier, detect pattern, noun hint; ability: verb, cooldown s, when, min creatures, min tier, daily limit, min health, light/dark | only the flares (`combat_effects.tsv:104` `valence` ...) | **GAP** | LOW (a data table and `send` steps would do) |
| `gshunting4all.lic:555-564` | `MINIBOSS_NAMES` | 42 | name fragment | `creatures.tsv` `boss`, `boss_type`: 71 bosses, 38 `miniboss`; 37 of the 42 match | **PARTIAL**: dogmatist, vesperti, massive pyrothag, forest trali shaman and arachne priest are not flagged as bosses in Cena (check the wiki) | LOW |
| `gshunting4all.lic:7377-7404`, `:1197-1208` | gemstone activated properties; EG/TG/TK abilities | 23; 4 | name, cooldown, when, desc | none | **GAP** | LOW |
| `hw_hunter_attack.lic:27-45`, `bountyhunter-bandits.lic:32-44` | per-profession self-buff lists (Bard, Cleric, Empath, Wizard) | 4 lists, 10-14 spells each | spell numbers | `hunt/profile.rs:79-81` `signs` (per profile) | **N/A**: one player's choices, not game facts | — |

## Captures Cena lacks or differs on (special ask 3)
| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| `Ashborne.lic:10912` | a group member did not follow you | `^([A-Za-z][A-Za-z'\-]*) is unable to follow you\.` | `crates/cena-model/src/movement.rs:107` has `^[A-z\s-] is unable to follow you\.$`, ported from `reference/lich-5/lib/common/move.rb:290`; `state/movement.rs:274` matches the fragment `unable to follow you` | **CONFLICT**: the ported regex has no `+`, so it matches only a one-letter name. VERIFIED with Python `re`: `Nerten is unable to follow you.` → no match, `X is unable ...` → match. The fragment table in `state/movement.rs` does catch it, so it is a latent bug in one of two tables, inherited from Lich | MEDIUM; HIGH once groups move together |
| `Ashborne.lic:16460-16474` | a creature begins to cast | `(?:An?\|The) <name> (?:prepares?\|gestures?\|chants?\|incants?\|invokes?\|begins to (?:prepare\|chant\|incant\|gesture)\|raises? .+? (?:hand\|arm)s?)` | bestiary `spell_prep` messages, 281 rows, `crates/cena-model/data/creature_messages.tsv`, matched by `state/creature_message.rs` | **PARTIAL**: per-creature texts HAVE; the generic verb catch-all is not ported | MEDIUM (interrupts, M6 routines) |
| `Ashborne.lic:17196` | a named player is attacking (used as a follower heartbeat) | `\A<Name>(?:'s)?\s+(?:appears to be focusing\|focuses\|utters\|channels\|gestures\|hurls\|shoots\|swings\|attacks\|ambushes\|attempts to\|launches\|invokes\|incants\|casts)\b` | the combat parser reads third-person attacks, `state/combat/attack.rs`, `combat_attacks.tsv` role `group=third_person` | **HAVE** (the fact); in Hydra a member's liveness is its session, not a line | — |
| `Ashborne.lic:11069-11074` | you searched a corpse | `\AYou search (?:the\|a\|an) .+\.[ \t]*\z` | `crates/cena-model/src/state/ledger/hunt.rs:37` `^You search the .+\.$` | **PARTIAL**: Cena takes only `the`; Ashborne also `a`/`an` (whether the game prints those is UNVERIFIED) | LOW |
| `group_ajar.lic:119` | someone tried to join you while your group is closed | `^([A-Z][a-z]+) tried to join your group, but your group status is closed\.` | `crates/cena-model/src/state/group.rs:65-118` (`GroupEvent`, 14 variants); Lich `group.rb:309`, `:351` have only `<name>'s group status is closed` | **GAP** (both repos) | MEDIUM, groups after M6 (auto-group a hosted character) |
| `group_ajar.lic:124` | someone tried to add you, or hold your hand, while your group is closed | `^([A-Z][a-z]+) tried to (?:add you to (?:his\|her) group\|hold your hand), but your group status is closed\.` | as above | **GAP** | MEDIUM, as above |
| `shunt.lic:406`; Ashborne gas cloud (see the Ashborne captures) | a room hazard: flee | loot nouns `cloud\|void\|sandstorm`, name `sealed fissure` | `grep -rni "hazard\|gas cloud\|sandstorm\|sealed fissure" crates` finds nothing in the model or the hunt | **GAP** | HIGH, M6: a hunt standing in a cloud |
| `shunt.lic:464` | the Rift moved you | `Suddenly you feel sick\|Suddenly, you feel a sensation\|The air whooshes by you` | none (`grep -rn "air whooshes" crates reference/lich-5/lib`: nothing) | **GAP** | LOW (Rift hunts) |
| `shunt.lic:570` | a hidden someone in the room | `obvious signs of someone hiding` | `crates/cena-model/src/state/claim.rs:120` `HIDING` | **HAVE** | — |
| `shunt.lic:334`, `universalkill-nylis2.lic:36-70` (and `-nylis`, `-daiyon`, `-champ`, `champkill`) | an `aim` that cannot land | `already missing that`, `does not have a head`, `cannot aim that high`, `find an opening for your strike` | `grep -rn "aim that high\|already missing that\|does not have a head" crates`: nothing. Cena knows amputations from crits (`state/creatures/body.rs`) | **GAP** (the replies) | LOW-MEDIUM: an `aim` step in a routine |
| `universalkill-nylis2.lic:36` | blocked by a thorny barrier | `thorny barrier surrounding` | `combat_results.tsv:82-84`, `state/overwatch.rs:165` | **HAVE** | — |
| `bountyhunter-support.lic:174` | a sanctuary: no fighting or no attack spells here | `Be at peace my child, there is no need to fight here.` / `... no need for spells of war in here.` | none in `crates` (Python substring search, Method); Lich has the spell half, `reference/lich-5/lib/common/spell.rb:41` (`results_regex`); `plan/33` §2b proposes `nomagic` from the map tag | **GAP** in Cena (Lich has `spells of war`; `no need to fight here` is in neither) | MEDIUM, M6 (a hunt that tries to fight in a sanctuary) |
| `bountyhunter-support.lic:174` | cut throat, silenced | `The searing pain in your throat makes that impossible.`; `The pall of silence settles more heavily over you.` | `state/afflictions.rs:199` has `The pall of silence settles more heavily over you.`; cutthroat is `afflictions.rs:57` | **PARTIAL**: silence HAVE; the "searing pain in your throat" refusal (a cast refused while cutthroat) is not in `crates`; Lich has it in `common/spell.rb:27` (`prepare_regex`, 12 prepare replies, and `results_regex` :35-60), none of which Cena ports (`Your spell is ready`, `fizzles ineffectually`, `already have a spell readied`: 0 hits in `crates`) | LOW |
| `bountyhunter-support.lic:176` | the creature absorbed your spell | `pulses brightly as (?:she\|he) absorbs the magic!` | `creature_messages.tsv` (bestiary text); `tests/combat_fsm_hunt_log_raw.rs` | **HAVE** as creature text | — |
| `bountyhunter-support.lic:178` (commented) | the bounty creature arrives | `Out of the corner of your eye, you see (.*) approaching.  (?:She\|He\|It) must be the creature that you've been tasked to kill!` | `state/bounty.rs`, `state/bounty_status.rs`: `grep -rn "must be the creature"` nothing | **GAP** | HIGH, M8 bounty (dangerous-creature and escort tasks) |
| `bountyhunter-support.lic:180-190` (commented) | debuffs that stop casting or acting | `The spirits distract your every action.`; `Your thoughts scatter as you struggle to prepare any magical incantations.`; `You suddenly feel angered beyond all reason, causing you to scream out in rage!`; `You can't think clearly enough to prepare a spell!` | none of the four in `crates`; Lich has `You can't think clearly enough to prepare a spell!` (`common/spell.rb:24`) | **GAP** (one of four in Lich) | MEDIUM, M6 (a guard that a cast will fail) |
| `bountyhunter-support.lic:188` (commented), `:170` | you were disarmed; your hands are empty | `is knocked from your grasp and out of sight!`; `You drop your (.*) into the shadows!`; `You cannot attack with\|You swing a closed fist` | `state/combat/status.rs:30` `Disarmed` is the **creature's** (`combat_effects.tsv:199`, "The X's Y falls to the ground."); your own: nothing. **Lich has it**: `reference/lich-5/lib/gemstone/combat/defs/messages.rb:89-97`, family `:disarm`, six `disarm_seen` patterns with the weapon's noun (knocked from your grasp, wrenched off bony protrusions, telekinetic, webbing) | **GAP** in Cena: `crates/cena-model/src/state/combat.rs:90-93` records `messages.rb` as NOT ported, "a later pass if a consumer wants it" | HIGH, M6: Ashborne builds a whole recovery on it (disarm flag, :14839) |
| `bountyhunter-support.lic:106`, `bountyhunter-bandits.lic:17-25` | recovering a dropped or hurled weapon | `You spy (.*) and recover it!`; `You know (.*) is around here somewhere, but you don't see it.`; `A (.*) rises out of the shadows and flies back to your waiting hand!`; `In order to recover your hurled weapon, you'll need to have a free hand.` | `travel/routines/sword_gorge.rs` has one `recover` reply for its own errand; the model: nothing. Lich has the bonded-weapon return, `combat/defs/messages.rb:142` (`bond_return`) | **GAP** | MEDIUM, M6 (the recovery half of disarm) |
| `bountyhunter-support.lic:194` (commented) | your weapon was turned into a creature | `form twists and mutates, sprouting scales and cold eyes as it transforms into (.*)!` (veiled sentinel) | nothing in `crates`; Lich `combat/defs/messages.rb:96` (`sanctum_transform`) | **GAP** | LOW |
| `bigshot-bless.lic:11` | a blessing has lapsed (the attack does nothing) | `has no effect` (bigshot's own reading is `but it has no effect`, `bigshot.lic:6398`, "Ammo had no effect (need blessed or magical)") | `combat_results.tsv:214-217` (`unaffected`) covers spell immunity, not this | **GAP** (exact sentence UNVERIFIED) | MEDIUM, M6 (undead and noncorporeal hunts) |
| `huntplan.lic:3526-3528` | the `mana` command: maximum mana, mana per pulse off node | `Maximum Mana Points:[^\d]+(\d+)[^\d]+(\d+)`; `Mana gained off node:[^\d]+(\d+)[^\d]+(\d+)` | `grep -rn "Maximum Mana Points\|Mana gained off node" crates reference/lich-5/lib`: nothing | **GAP** | MEDIUM, M6 rest: huntplan derives `rest_till_mana` as "within one pulse of full" |
| `huntplan.lic:3628` | a bard's song renewal cost | `Your current renewal cost is (\d+) mana.` (`song status`) | `state/character/spellsong.rs:34` defers `renew_cost` | **GAP** | MEDIUM, M6 Maintain for bards |
| `hw_hunter_follower.lic:29-38`, `:94-99` | bounty kill counts, from `bounty` and from the guild clerk | `You need to kill ([\d,]+) (?:more )?of them to complete your task.`; `Killing (.*) of them should do nicely.  Report back to me when you are done.` | `state/bounty.rs:368`, `:422` (the `bounty` report) | **HAVE** the report; **GAP** the clerk's spoken assignment (LOW) | — |
| `hw_hunter_attack.lic:100-114`, `shunt.lic:939-949` | UCS positioning and tier-up | `You have (decent\|good\|excellent) positioning`; `Strike leaves foe vulnerable to a followup (.*) attack!` | `state/combat/ucs.rs:99-139`, `combat_effects.tsv:349` | **HAVE** | — |
| `hw_hunter_attack.lic:140-178` | the stance changed | `You are now using N% of your combat skill to defend yourself.`; `You are now in an offensive stance.` | `crates/cena-behavior/src/stance.rs:72` `landed` reads the stance bar, not prose (`plan/12` §3a) | **HAVE** (by the model, not the sentence) | — |
| `easykill.lic:19`, `bountyhunter-bandits.lic:122`, `fastkill.lic:22` | the target is gone or dead | `is quite dead already`; `You currently have no valid target.`; `What were you referring to?` | `combat_results.tsv:220` (`already_dead`); no-target: the send gate refuses `TargetGone` before sending (`cena-session/src/command/verdict.rs:350-356`, `hunt/drive.rs:562`) | **HAVE** (dead); the no-target reply is not classified, the gate makes it rare | LOW |
| `Ashborne.lic:17317-17318`, `:16538` | a gas cloud is forming over me (and the night hound's shadow cloud) | `\A(?:You notice (?:a gas cloud begin to form above you\|the gas cloud begin to condense)\|Large sparks begin to form in the center of the gas cloud\|The static discharges are now audible from the gas cloud\|The gas cloud rumbles a bit)`; `\A(?:You notice (?:that )?the night hound(?:'s)? breath (?:hangs in the air\|congeal into a billowing cloud of shadows)\|The shadowy cloud seeps languidly\|You breathe in some of the gas)` | which creatures cast one is bestiary data (`creature_attacks.tsv`); no classifier | **GAP** | HIGH, M6 |
| `Ashborne.lic:17335` | the cloud is gone | `\A(?:The gas cloud dissipates rapidly\.\|The gas cloud begins to dissipate\.\|A quiet wind disperses the gas cloud\.)\z` | none | **GAP** | HIGH, M6 |
| `Ashborne.lic:16539-16544`, `intel_huntpro.lic:1112` | the ground under me is about to erupt | `The ground beneath your feet (?:begins to boil violently\|boils with renewed vigor\|suddenly frosts and rumbles violently\|rumbles with renewed vigor)`; `(?:Fiery\|Craggy) debris explodes from the ground beneath you`; `The earth cracks beneath you, releasing a column of frigid air`; `Icy stalagmites burst from the ground beneath you` | the attack lines classify (`combat_attacks.tsv:55-59`); the warning is not acted on | **PARTIAL** | HIGH, M6 |
| `Ashborne.lic:16648-16649`, `:16665` | Weapon Fire struck my weapon; it cooled | `\bYour (.+?) is struck with .+?\bcast\b`; `... leaps from (?:an?\|the\|your) (.+?) in your (?:right\|left) hand\b`; `<noun>.*returns to normal` | `creature_messages.tsv:3132-3133` (`weapon_fire` triggers); `state/creature_message.rs:245` `classify` has **no caller outside tests** (`grep -rn "creature_message::" crates`: two test files) | **PARTIAL** (data, unwired) | HIGH, M6 |
| `Ashborne.lic:2213`, `:2238-2247`; `intel_huntpro.lic:1096`; `chuntpro.lic:5777` | my attack does nothing: need a blessed or enchanted weapon | `but it has no effect\|your attack has no effect` | `combat_results.tsv:214-219` are spell immunity; the weapon line: 0 hits | **GAP** | HIGH, M6 |
| `intel_huntpro.lic:544-559`, `Ashborne.lic:2878-2896`, `gshunting4all.lic:11863` | sanctuary (also the Minor Sanctuary 213 landing) | `there is no need to fight`, `no need for spells of war`, `Be at peace my child\, there`, `sense of peace and calm settles over the` | Lich `common/spell.rb:41`; `bigshot.lic:5818` | **GAP** in Cena | HIGH, M6 |
| `intel_huntpro.lic:649-661`, `Ashborne.lic:2976-2988` | I am confused; it passed | `You are confused\!\|Your thoughts become clouded and confused`; `The fog of confusion recedes\.` | "You are confused!" only as a triton trigger in `creature_messages.tsv`; no status | **GAP** | MEDIUM |
| `intel_huntpro.lic:622-690` | a spell misfired or diverged; it may be cast again | `Your spell misfires`; `bright white light briefly appears\, then fades again`; `you feel able to cast Spirit Slayer again\.`; `you feel able to cast Wall of Force again\.` | none | **GAP** | MEDIUM (Maintain's retry) |
| `Ashborne.lic:2873`; `intel_huntpro.lic:539`, `:566`, `:571` | my spell was eaten, fizzled by noise, or the target is out of reach | `absorbs the energy of your spell`; `disruption of the noise in the area causes the spell to fizzle`; `Perhaps you should try throwing or shooting something at` | none | **GAP** | MEDIUM |
| `intel_huntpro.lic:1201`, `:1206`; `Ashborne.lic:3294`, `:3299` | bounty cooldown: eligible now, or later | `You are eligible for new task assignment`; `You will be eligible for new task assignment` | `state/bounty.rs`, `bounty_status.rs`: 0 hits; not in Lich core either | **GAP** | HIGH, M8 |
| `Ashborne.lic:18157-18201` | another character's rank-1 wounds, read from `appraise` | `(?:a )?bruised #{side} eye`, `minor cuts and bruises on (?:his\|her\|their) #{side} #{limb}`, `minor (?:bruises\|lacerations) about the head`, `strange case of muscle twitching` ... | own injuries only (`state/character/injured.rs`, `body.rs`) | **GAP** | HIGH, M6d (an Empath healing the group) |
| `Ashborne.lic:12696`, `:13062` | the target has no blood; no bones | `Finding no living blood upon which to draw, your questing magics pass harmlessly through`; `You concentrate intently .*finding no bones within to break\.` | `creatures.tsv` `blood` and `bones` columns; the replies: 0 hits | **PARTIAL** | MEDIUM |
| `huntpro.lic:15271-15287`, `Ashborne.lic:3214`, `:15763-15772` | swallowed by a roa'ter | room `Belly of the Beast`, object `stomach wall`, `Accept your fate`, `tossed about like a rag` | none | **GAP** | MEDIUM |
| `gshunting4all.lic:10362` | a weapon reaction is offered | `You could use this opportunity to (.+?)!` (Lich: `<d cmd='WEAPON (\w+\s#\d+)'>`) | 0 hits; Lich `combat/defs/messages.rb:161`, `bigshot.lic:2757` | **GAP** | MEDIUM (bigshot's `weapon_reaction` setting) |
| `gshunting4all.lic:12366`, `:12382` | disarmed (floating; knocked away) | `Your (.+?) tears free from your hands and floats`; `knocked from your grasp and out of sight` | 0 hits; Lich `combat/defs/messages.rb:90-95` | **GAP** | HIGH, M6 |
| `gshunting4all.lic:11851` | my legs will not move (stand refused) | `don't seem to be able to move your legs` | 0 hits; Lich `messages.rb` family `:hold` has `You don't seem to be able to move(?: your legs)? to do that\.` | **GAP** | MEDIUM (survival re-sends `stand`) |
| `gshunting4all.lic:12132-12141` | dread stacks | `creeping dread \((\d+)\)`, `crushing dread \((\d+)\)` | the Debuffs text is kept (`effects.rs`), no typed count. gsh4a's reset can never fire (it matches a tag against a tag-stripped line) | **PARTIAL** | MEDIUM |

## Script by script
| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| `Ashborne.lic` (NecroDeus, v0.8.18, 2026-09-19; no license line) | 32,527 | solo and multi-account group hunting coordinator, GUI and an in-process "Intel" line watcher; the newest of the huntpro family | 26 coordination files (special ask 1); 78 game-text facts, 9 DownstreamHooks and one ~120-test `get` loop (`Ashborne.lic:2533-3598`); 42 constants, 21 game-data tables; 152 settings (`SETTING_DEFAULTS` :4871-5024) | the group bus is the requirements list for Hydra's groups; hazards, disarm, weapon fire, sanctuary, wrong equipment, spirit and wound rest triggers are **GAP**s; bounty reading **HAVE** (Cena's is more general), bounty → ground **PARTIAL** |
| `huntpro.lic` ("2026 Sun", 2026-08-17; by Jara, `updater_huntpro.lic:23`, `huntpro.lic:10`; no license line) | 19,160 | one-trip hunter: `;huntpro <style 1-9> <area>`, fights until a rest trigger, walks home, optionally cleans up, **exits** (`huntpro.lic:5620-5754`) | 114 settings (`:23-136`); 56 result patterns and no DownstreamHook of its own; 254-area catalogue; 170 boundary escapes (`:14141-15270`) | rest triggers and hazard rules as above; `flee` and "ignore" semantics **CONFLICT** with Cena's |
| `intel_huntpro.lic` | 13,265 | not an engine: huntpro's game-line watcher, started every loop (`huntpro.lic:433-439`), steering it by globals | 1,272 `line =~`/`when` sites (978 are one bounty table) | sanctuary, wrong equipment, confusion, misfire/divergence, ground traps, bounty cooldown **GAP**; bounty kinds **HAVE** |
| `chuntpro.lic` ("Classic Huntpro", superseded at huntpro v11) | 9,198 | the older huntpro, GTK setup, built-in town round | 28 settings; 45 result patterns | only differences reported: stance lock under Frenzy/Rage/Zealot (**GAP**), level-gated flee, `stance_dance`, appraise-driven UAC |
| `bigshit2.lic` (Rinkidinkicus, v1.0.0, 2026-03-31; no license line) | 7,494 | claims to be "bigshot with adaptive command pipelining" (`:5-13`); it is bigshot 5.7.9 plus **a hidden prank**: on a random 10-40% of rests, `do_relief` (`:5306-5395`) blanks all game output with a DownstreamHook (`:389-390`), posts on the `help` and `ooc` channels, walks to town, removes armor and trousers, kneels and yells (VERIFIED by reading `:5306-5345`, `:5538-5552`). Its header copies bigshot's contributors line, which names Nisugi among others (`:16`) | adds no guard word, no setting and no game-text capture (the scratch diff: 45 guard words, all in bigshot's 87) | **N/A, and a warning**: do not recommend it. It is also the argument for `plan/35` (M7) that a hook able to return `nil` can blind the player; Hydra has no user hooks |
| `gshunting4all.lic` (Falvicar, v3.2.2; no license line) | 13,340 | a combat assistant, not a hunter: the player walks, it fights (`:16`); 23 background watchers; an escape ladder | escape rules by health, mana, per-part wounds, dread, encumbrance, overexertion, overflow; ~50 captures, many marked *guessed* by its own author (changelog `:43-44`); data tables for scripted items and society abilities | per-part wound thresholds, escape ladder, disarm recovery **GAP/PARTIAL**; its society table disagrees with the wiki and with Cena's `societies/*.rs`: do not port it |
| `huntplan.lic` (Mertyn, v1.0) | 3,788 | plans a hunt for one creature or the current bounty: writes bigshot's targets, boundaries, flee list and a per-profession routine (`hunting_style` :3495-3505), and ebounty's mapping | data: 543 creatures with level and spawn ids, 1,307 spawn → room lists, 27 immunity facts; captures: `mana` report, `song status` renewal; uses `Bounty.task`, `Spell[]`, `Skills`, `Room` | levels **HAVE** (517) with 7 **CONFLICT**s; spawn rooms **PARTIAL**; immunities mostly **GAP**; `mana` and renewal **GAP**. Two source bugs: `elsif hunting_style = 'warrior_melee'` (:3664, assignment); `safe_room_level` +2 rule :3193 is a good idea worth keeping |
| `hw_hunter_attack.lic` (Alastir) | 653 | Hinterwilds group attack routine per profession | captures: UCS, stance confirmations, attack results; data: 13 HW creatures armed/unarmed | UCS and stance **HAVE**; armed flag **PARTIAL** |
| `hw_hunter_follower.lic` (Alastir) | 145 | a follower driven by the leader's **emotes and game text** (`shifts his weight` → deposit, `yawns` → heal, draws weapon → follow) | captures: bounty counts, the leader's actions, ooze engulf, cannibal ambush | a different group protocol from Ashborne's: signals in the game text. Bounty **HAVE**; the rest is one player's convention (**N/A**) |
| `shunt.lic` (spiffyjr, 2021) | 1,014 | a hunt-only engine (no rest), fast | flee rules (`flee_at`, `flee_group`, `flee_ignore`, `flee_always`, hazards :405-419), aim fallback :334, creature-acting-on-you tracker :903-950, hidden-player skip :570, Rift teleport :464 | flee groups and ignores, hazards, aim replies **GAP**; hiding **HAVE**; UCS **HAVE** |
| `bountyhunter-bandits.lic` (Alastir) | 181 | kill bandits per profession | captures: hurl and `recover hurl` replies | recovery **GAP** |
| `bountyhunter-support.lic` (Alastir) | 203 | a watcher beside bigshot for bounty hunts | captures: disarm, sanctuary, silence, cast debuffs, bounty creature arrival, weapon-to-adder | mostly **GAP** (see captures); the bounty-creature arrival is HIGH for M8 |
| `smarthunt.lic` (Aethor, Yasutoshi, 2014, v1.4.4) | 1,177 | hunting wrapper for monks and Voln | settings only: hunt time limit (`sm_minutes_to_hunt` :122), mind start/stop (:126-127), herb health (:119), mana floor (:125), Voln symbols per hunt (:109-110) | settings compared in special ask 2 |
| `group_ajar.lic` (Tillmen, v0.1) | 153 | opens the group when dead or stunned (so a healer can join), auto-joins an allow list | captures: two "group status is closed" lines | **GAP** (group.rs lacks both) |
| `universalkill-nylis2.lic` (Nylis) | 89 | aim-and-ambush with body-part fallback | aim refusals | **GAP**. Variants: `universalkill-nylis.lic` (87, lacks `find an opening for your strike`), `universalkill-daiyon.lic` (87, maul, encumbrance check commented out), `universalkill-champ.lic` and `champkill.lic` (71 each, identical but for comments: aim legs first) |
| `easykill.lic` | 48 | attack the first aggressive NPC; skip grizzled/ancient unless the bounty is a dangerous creature (:9) | `is quite dead already` | **HAVE** |
| `hunt2.lic` | 41 | start `wander` for the bounty creature | bounty parse (`suppress (.+?) activity`, `SKIN them off the corpse of a`...) | **HAVE** (`state/bounty.rs`) |
| `bigshot-bless.lic` (Veska) | 15 | `sym bless` when an attack "has no effect" | one capture | **GAP** |
| `huntingareas.lic` (Salasin, v1.0.4) | 49 | a table of 38 bigshot hunting grounds | data only | **GAP** (see data tables) |

## Tail

- **Stop bigshot after one run** (the idea is a setting: hunt N cycles, then stop): `stopbigshot.lic` (18,
  Zachriel), `killbigshot.lic` (7). Cena's profile has no cycle count (`hunt/profile.rs:68-96`); Ashborne
  has `cycle=N hunts=M` (:19930-19940). Counted in special ask 2.
- **Wrappers and helpers, no game data:** `shunt-loop.lic` (27, loops shunt), `wanderb.lic` (1 line:
  `wander` with `UserVars.bounty`), `fastkill.lic` (22, `kill` until `You currently have no valid target`),
  `updater_huntpro.lic` (144, a file updater for huntpro), `huntmaster.lic` (18, a placeholder that echoes
  "todo").
- **Not hunting:** `grind.lic` (23, alchemy grinding; `ground as much as possible`, `appears to be as ground
  as` belong to pile 7's crafting).

## Method

**Coverage.** 31 scripts (`pile-1-hunting-engines.tsv`), 103,102 lines (`cat` of the 31 files `| wc -l`; the TSV's column sums to 103,118). 20 are substantive by
the brief's rule (capture + data ≥ 5). Read deeply by me: `Ashborne.lic`'s coordination layer (every
writer and reader of the 26 files, the remote-command channel, the danger, claim, leader-loss and
town-barrier code), `huntplan.lic`, `hw_hunter_attack.lic`, `hw_hunter_follower.lic`, `shunt.lic`,
`bountyhunter-bandits.lic`, `bountyhunter-support.lic`, `group_ajar.lic`, `universalkill-nylis2.lic`
(its three variants and `champkill.lic` by `diff`), `easykill.lic`, `hunt2.lic`, `smarthunt.lic`'s settings.
At capture-line depth, through three scratch extractions whose rows cite a line each and which I
spot-checked (below): the rest of `Ashborne.lic` (captures, tables, rules; its hooks and main loop were
read whole), `huntpro.lic`, `intel_huntpro.lic`, `chuntpro.lic`, `bigshit2.lic`, `gshunting4all.lic`. The
extractions are `tools/1-hunting-engines/ashborne-captures.md`, `huntpro-family.md` and
`bigshit2-gshunting4all.md`, each with its own Method; they hold more rows than this file carries.
Tail: 11 scripts, all opened.

**HEAD.** `3566f8d` at the start, `a6146e6` at the end (other sessions committed skinning work and
`plan/36`'s herb table). `git -C E:/Cena diff 3566f8d HEAD --stat -- crates/cena-behavior/src/hunt/ crates/cena-model/src/movement.rs crates/cena-model/src/state/claim.rs crates/cena-model/src/state/creature_message.rs crates/cena-model/src/state/combat.rs crates/cena-model/src/state/creatures/instance.rs crates/cena-model/data/creatures.tsv crates/cena-model/data/creature_lists.tsv crates/cena-model/src/state/bounty.rs`
shows only `hunt/desk.rs` and `hunt/drive.rs`; the one `drive.rs` cite was re-read (`:562`). New since
the start and relevant to the M6d rows: `crates/cena-model/data/herbs.tsv` and `state/doses.rs` (`plan/36`).

**Counts.**
- Shared-file names: `grep -oE "_(ashborne|intel_ashborne|huntpro|uberbeta)_[a-zA-Z0-9_#{}.]*\.(txt|yml|yaml|log|json)" Ashborne.lic | sort | uniq -c` (30 patterns); derived names `grep -nE "_file(\([^)]*\))? *\+ *\"" Ashborne.lic`; danger reasons `grep -n "publish_group_danger(" Ashborne.lic` (31 sites); spell-claim keys `grep -oE "group_spell_claim(ed)?\(\"[^\"]+\"" Ashborne.lic | sort | uniq -c`.
- huntplan levels: `tools/1-hunting-engines/huntplan_levels.py` (reads `huntplan.lic` and `creatures.tsv` as UTF-8; 543 rows, 548 names, 517 same level, 8 differ of which wharf rat is a blank in Cena, 12 names missing).
- huntplan immunities `grep -c "\.immunities\.push" huntplan.lic` (25), weaknesses (2); spawn rooms as in the data table row; huntingareas `grep -c "^\s*\[\['" huntingareas.lic` (38); Cena immunity rows `awk -F'\t' '$2=="immunities"' creature_lists.tsv` (18).
- Cena creature messages by kind: `cut -f2 crates/cena-model/data/creature_messages.tsv | sort | uniq -c` (281 `spell_prep`).

**Absence claims.** Every "none in `crates`" above was re-run with a Python substring search, not grep:
`tools/1-hunting-engines/absence_check.py` reads every `.rs` and `.tsv` under `E:/Cena/crates` (607 files,
`target/` excluded) and every `.rb` under `reference/lich-5/lib` (955 files), case-insensitive, and prints
the hits per fragment. It is why several rows say "Lich has it": `spells of war`, `searing pain in your
throat`, `think clearly enough to prepare` (`common/spell.rb`), `knocked from your grasp`, `sprouting
scales`, `flies back to your waiting hand` (`combat/defs/messages.rb`). One of the scratch extractions
found that `grep -iF` returns nothing at all under this machine's Git Bash grep 3.0 (a manufactured false
negative); no absence claim here rests on `-iF`. The concept searches (type and field names) are the
`grep -rn` commands quoted in the rows: e.g. `grep -rni "hazard\|gas cloud\|sandstorm\|sealed fissure" crates`,
`grep -rn "creature_message::" crates`, `grep -rn "invalid_targets\|ignore" crates/cena-behavior/src/hunt`.

**Spot checks of the extractions** (each line opened with `sed -n`): `huntpro.lic:15506` (spirit),
`:15578`, `:15596`, `:2028`, `:2034`, `:2041`; `Ashborne.lic:9528`, `:9545`, `:28495`, `:19992`, `:2461`,
`:18959`, `:3294`, `:2878`, `:16538`, `:17317`, `:18157`; `gshunting4all.lic:580`, `:6094`, `:7785`, `:9744`;
`chuntpro.lic:5777`, `:2282`; `intel_huntpro.lic:114`, `:544`, `:1112`, `:1201`; `bigshot.lic:2757`, `:5818`;
`bigshit2.lic:385-392`, `:5306-5345`, `:5536-5553`. Two cites were off and are corrected here
(`intel_huntpro.lic:1096`, not :1093; `hunt_cycles` is `Ashborne.lic:600`).

**Not done.** No log-archive search, nothing run against the game, nothing under `E:\Cena` edited.
Sample sentences for `has no effect` and the gas-cloud lines are the scripts' regexes, not observed wire
text.
