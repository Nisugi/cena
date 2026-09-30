# 59 — The inventory manager

PROPOSED 2026-09-30. The author, reading `plan/58`: *"this makes me realize we need to get a
few things in first. So I think inventory manager should be our first add, let's make a solid
plan for that."*

The inventory manager is **everything the character carries, as one tree**:
- every worn container, every bag inside it, the hands, what is at the feet, and the reserve;
- each item's weight, and each container's capacity and how full it is;
- closed and locked containers marked;
- the room's containers and floor.

A player can find an item wherever it is, read an item's full detail, and move items with the
move confirmed.

Saga has one (*"a live, structured view of everything worn, held, and on the ground as
container cards you can open and nest (in/on/behind/under). Dragging an item between
containers has Saga plan and issue the actual game commands for you"*,
`reference/wiki_clean/Saga.txt:25`). VellumFE has one: the Containers window and four
commands (`reference/VellumFE/book/src/widgets/containers-window.md`,
`book/src/features/inventory-tools.md`).

Measured 2026-09-30 against `reference/VellumFE` at `c1f7953` and Hydra's `main`.

---

## 1. How the game answers

It only answers a client that logs in with the **WRAYTH banner**
(`reference/VellumFE/src/network.rs:679-683`). Hydra already sends it:
`CLIENT_BANNER = "/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML"`
(`crates/cena-platform/src/eaccess/wire.rs:37`), from `connect_game`
(`crates/cena-platform/src/eaccess/game.rs:77-82`), and the Lich relay sends the same
(`crates/cena-agent/src/lich.rs:73`). Without it the game says nothing at all.

**The request** is `_inventory manager <token>`
(`reference/VellumFE/src/core/inventory_service.rs:159`).

**The answer** is one `<inventoryManager id=token room=…>` element, holding an `<i>` per item
(`reference/VellumFE/src/parser/tests.rs:2652`, from a 2026-08-12 log):

```xml
<inventoryManager id='imtest1' room='2005'><i id='148848453' loc='worn,player' name="a patchwork,dwarf skin,backpack" long="a $_patchwork dwarf skin backpack$_ bound by interwoven briar vines" weight='5' in_max='2000'/><i id='148848479' loc='in,148848453' name="an,aquamarine,wand" weight='1'/>…</inventoryManager>
```

- `loc` is a relation and the item's parent:
  - to the character: `worn`, `righthand`, `lefthand`, `atfeet`, `reserved`;
  - to a container, by its id: `in`, `on`, `behind`, `underneath`;
  - `room` alone for the floor.
- `in_max` and `on_max` are packed: `v/10` pounds, `v%10` items, where 0 means no limit.
- `weight='-1'` marks a fixture.

**Paging.** A long tree comes in pages. A page ends with `<continuation root last/>`, and the
next page is asked for with `_inventory manager <token> continue <room> <root> <last>`
(`inventory_service.rs:278-280`). A dead cursor answers with `state='stale'`
(`parser/tests.rs:2746`). Nothing on the wire marks the end: the tree is whole when no
cursor is left unasked.

**An item's detail** is asked for with `_inventory viewitem <token> <id>`. When the item is
in a locker, `via <selector>` is added (`inventory_service.rs:358-374`). The answer is an
`<inventoryViewItem>` element, 7 to 50 lines long, with four `<result>` sections: look,
inspect, analyze and recall (`plan/15-wrayth-protocol.md` §6.11: 13 answers, 52 sections).

### 1a. What Hydra already has

The whole **receiving** side is built:
- **The parser** (`crates/cena-protocol/src/parser/inventory.rs`, `parser/view_item.rs`).
- **Typed frames**: `InventoryResponse`, `InventoryItem`, `Continuation`, `ItemView`,
  `ItemDetail` (`frame/payload.rs:288-487`).
  - An item's detail **keeps its links**, where VellumFE flattens them away.
- **The model** (`crates/cena-model/src/state/inventory_snapshot.rs`, 222 lines):
  - the tree, the room it was taken in, and cursors not yet asked;
  - the details asked for;
  - kept across a reconnect (`state/reconnect.rs:385-396`).
- **Tests** that measured the author's September logs exactly
  (`snapshots=36 items=5364 views=13 sections=52`), with 21 of 21 mutants killed
  (`plan/15` §6.11).

**Nothing sends the request.** A grep of `crates/` for `_inventory` finds only doc comments.
`plan/15` §6.11 says so: *"The request/continuation service is NOT built"*. Travel's routines
**read** the snapshot when one happens to be there: `travel/facts.rs:135-142`,
`travel/kept.rs:50-72`, `routines/cutter.rs:238-349`, `day_pass.rs:536-557`,
`shopping.rs:83-97`, `trinket.rs:162-171`. `state/resolve.rs:252-262` does the same.

The model also has three shortcomings the manager would expose:
- `apply_snapshot` **replaces** the tree on every answer (`inventory_snapshot.rs:61-83`), so a
  second page would erase the first;
- `find` matches an exact noun only (`:122`);
- `total_weight` has neither Saga's 0.1-pound floor nor its rule for weightless containers
  (`:131-137`), and reads `99990` as 9,999 pounds where `invmgr.lic` reads it as no limit
  (`inventory/12-lich-repo-mirror/4-loot-trade.md:317`).

On the GUI side:
- the **Containers** widget draws only the passive feed, the containers whose windows the
  game opened (`cena-gui/src/widget/lists.rs:79-160`);
- **item drag** exists: `_drag`, Alt by default (`cena-gui/src/carry.rs`). Nothing checks a
  move landed (`inventory/15-vellum-gaps.md:195`).

---

## 2. What VellumFE built, and what to keep of it

### 2a. The service (`inventory_service.rs`, 796 lines with its tests)

VellumFE's service is a pure state machine: it hands back the lines to send and never sends
them itself (`:17-20`).
- Up to 4 page requests at once, each with its own token and a 10-second limit.
- A room that changes between pages, a `stale` answer, or a timeout: the load starts again
  from scratch, twice at most (`:25-36`, `:172-310`).
- One refresh at a time. A second is refused while one is loading (`:140-146`).
- Repeated items and cursors are dropped.
- Its tests are exact and port directly (`:446-796`).

**One defect not to copy.** An answer whose token is not in flight is published *over* the
snapshot (`src/core/messages/element.rs:2410-2489`). That includes a late page from a timed-out request, or from
a load that restarted. A lone page with no cursor is then published as the whole inventory.

### 2b. The four commands

| Command | What it does |
|---|---|
| `.invsync` | takes the snapshot (`commands.rs:2907-2918`), announcing `snapshot complete: N items (room R)` |
| `.find <words>` | searches the snapshot, never the game, by name and long description. Prints each match's place outermost first, `in your backpack > blue velvet pouch (closed)`, and its id (`commands.rs:2052-2106`, `state.rs:1475-1541`) |
| `.viewitem <id>` | the item's four sections, shown in the `inspect` stream and the window's Item tab, never the story (`commands.rs:2883-2900`, `travel_ticks.rs:606-654`) |
| `.drag <id> left\|right\|drop\|wear\|feet` and `.drag <id> in\|on\|behind\|underneath <id>` | a move **checked against the hands**: *confirmed* when a hand shows it, *sent* for a container-to-container move that never touches a hand, *failed* after 8 seconds. It refuses a full hand or a second move. A locker is named by its `in_selector` (`src/core/item_mover.rs`, 374 lines) |

### 2c. The Containers window (`frontend/gui/app/widgets/containers.rs`, 646 lines)

- **Header:** Refresh and an item count, marked *incomplete* when it is.
- **Tabs:** Containers, Worn, Room and Item. Seven groups: hands, worn containers, worn,
  at the feet, reserved, the room's containers, and the floor (`:121-135`).
- **Container rows:**
  - start collapsed, and stay as left across a refresh;
  - carry 🔒 when locked and ▣ when closed;
  - show three columns: items inside, weight, and capacity;
  - colour the weight amber at 70% full and red at 90% (`:45-60`).
- **Weight:** Saga's rule (`state.rs:1358-1429`):
  - an item's own weight, 0.1 pound when the game says 0;
  - the contents of a container whose `in_encum` is 0 left out;
  - unknown if anything inside is unknown.
- **Clicking an item** opens it in the Item tab.
- **Right-click:** Inspect, Look, To right hand, To left hand, Drop, Wear; on a container,
  Focus and Open or Close.
- **Focus:** one container's whole contents, flat and sorted.
- **Room items** are hidden when the character has since moved rooms (`:152-157`).
- **Refresh is by hand only**, by the owner's decision (`:3-5`). `plan/28-gui-inventory.md:1053`
  marked this as *"a decision to re-confirm"*.

**VellumFE also probes.** After each snapshot it asks the detail of up to 40 containers, one
per prompt, only to learn whether each is closed (`inventory_service.rs:315-364`).

**What VellumFE lacks:**
- no drag in the tree;
- `on_max` surfaces never shown;
- the per-item count limit decoded and never shown;
- links in the detail flattened;
- the tab and focus kept once for all windows (`containers.rs:308-310`).

---

## 3. Hydra's shape

### 3a. The load lives in the model, pure; the session drives it

**The model** (`cena-model/src/state/inventory_snapshot/`, split from today's file) gains
`Loading`. Its state machine is VellumFE's (§2a), with the defect fixed:
- **An answer with a token Hydra is waiting on** is merged into the load. The load is
  published whole, once, when no cursor is left.
- **An answer with a token Hydra is not waiting on:**
  - applied as today when nothing is loading, because the player typed it;
  - dropped while a load runs.
- **A late page is always dropped**; it never replaces the tree.

This fixes the page problem in §1a where it lives. `apply_snapshot` never sees a single page.

**The session's actor** drives it the way it drives Hydra's own `health` for the nerves
(`cena-session/src/actor/nerves.rs:22-49`, the glossary's *Hydra's own asking*):
- It sends each line the load hands back as `Origin::Hydra`, echoed nowhere.
- At each prompt it ticks the load's clock.
- `SessionHandle` gains two calls: *refresh* and *view item*.
- An `Event` tells every viewer a snapshot or a detail arrived.

Keeping the machine in the model means the GUI, the web page (`plan/58` step 7), the agent,
and the Ruby bridge all read one tree. None of them sends its own request.

The answers are elements, not prose, so nothing reaches the story (the parser emits no text
for them; `plan/15` §6.11). Nothing needs hiding.

### 3b. The questions a tree answers, in the model

Beside the snapshot, each ported with VellumFE's tests:
- **Weight:** Saga's rule, with `99990` read as no limit.
- **Contents:** the count of everything inside, however deep.
- **Where an item is:** `in your backpack > blue velvet pouch (closed)`.
- **Finding:** by any words of the name or long description.
- **Grouping:** the seven groups.
- **The locker selector:** `via`, for an item inside a locker.
- **Staleness:** whether the room has changed since the snapshot.

The travel routines keep reading `on_person` and `contents_of` as they do today, and find
the tree there more often.

### 3c. Moves, checked against the hands

VellumFE's move checker (`item_mover.rs`) goes into the session:
- The **one** way the manager, `.drag`, and the GUI's existing drag (`carry.rs`) move an item.
  Today's drag sends and forgets.
- Each outcome is told in Hydra's window: confirmed, sent or failed.
- A confirmed move **patches the tree**: the item's parent and relation change, so the window
  is right without a refresh. A move only *sent* is marked unconfirmed until the next refresh.
- Moving onto a container:
  - into it when it has only `in_max`;
  - with a choice of *in*, *on*, *behind* or *under* when it has more;
  - a locker named by its `in_selector`.

---

## 4. Steps

Each step is a commit on branch `inventory`, with its tests.

0. **Measure, with your leave** (§5 item 6). From the September logs that already hold 36
   snapshots:
   - Does a prompt follow `_inventory manager` and `_inventory viewitem`? This decides how
     the session's queue waits on them.
   - Does the snapshot's `flags` carry `closed` for every closed container? This decides
     whether probing is needed.
   - How long does an answer take?
1. **The model.**
   - `Loading`, the state machine with VellumFE's service tests (`inventory_service.rs:446-796`) and the defect's own test.
   - The page merge.
   - §3b's questions, with VellumFE's weight, place and find tests.
   - `inventory_snapshot.rs` split into a folder under the cap.
2. **The session.**
   - The actor sends, ticks and publishes.
   - `SessionHandle::refresh_inventory` and `view_item`.
   - The snapshot taken when §5 item 1 says.
   - Tested against `AnsweringSource` with a paged answer, a stale cursor, a timeout, and a
     room change.
3. **The commands**, named as §5 item 2 decides.
   - The help lines, the glossary rows (*inventory snapshot*, *item detail*, *checked move*),
     and the architecture page.
4. **Checked moves** (§3c).
   - The session's move checker, `.drag`, and the GUI's drag moved onto it.
   - A confirmed move patching the tree.
5. **The window.**
   - A new **Inventory** widget: the Containers widget stays, drawing the passive feed. It
     gets §2c's header, tabs, groups, columns, colours, Focus, right-click menu, and room
     staleness.
   - `on_max` shown as a second capacity where a container has one.
   - The Item tab's detail keeps its links, clickable as the story's are.
   - Rows are drag sources and drop targets (§3c).
   - Tab and focus kept per window.
   - kittest images of each tab.
6. **The records.**
   - `plan/15` §6.11 marked built.
   - `plan/27a-model-api-spine.md:72-73` and `plan/27e-model-api-inventory.md:27-37`
     corrected (both still call the snapshot *"the worn-items list"*).
   - `inventory/15-vellum-gaps.md`'s two rows (`:194-195`) updated.

The web page's nested inventory (`plan/58` step 7) then only draws what step 1 already
answers.

---

## 5. Questions for the author

1. **When is the snapshot taken?**
   - (a) By hand only, as VellumFE does. That was your decision there (`containers.rs:3-5`).
   - (b) **Once, quietly, after each login and reconnect, and by hand after that.**
   - (c) Also again after each hunt's rest or selling round.

   Recommended: (b), with §3c's confirmed moves keeping the tree right between refreshes.
   The travel routines and `resolve.rs` then always have a tree to read.
2. **Command names.**
   - VellumFE's four as they are: `.invsync`, `.find`, `.viewitem`, `.drag`.
   - Or one family: `.inv`, `.inv find`, `.inv view`, `.inv move`.

   Recommended: VellumFE's, for the muscle memory. `.find` does not collide with the Find bar,
   which is a key (`plan/52` step 6), not a command.
3. **Probing for closed containers.** VellumFE asks the detail of up to 40 containers after
   each snapshot, one per prompt. Recommended: none, if step 0 shows `flags` already carries
   `closed`. An item's detail, and Open or Close from the menu, keep a flag right when it
   changes.
4. **Drag in the tree** (Saga has it, VellumFE does not). Recommended: yes. `carry.rs` is
   already there, and §3c makes it checked.
5. **Where an item's detail shows.**
   - VellumFE sends it to an `inspect` stream and the Item tab.
   - Recommended: the Item tab. `.viewitem` without an Inventory window open says it in
     Hydra's window instead, sections and links intact.
6. **May I read the log archive for step 0?** The measurement reads the author's September
   logs, the same 36 snapshots `plan/15` §6.11 counted. The rule is to ask first.
