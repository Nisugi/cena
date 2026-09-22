# 27e — Hands, inventory, containers and money

What you are holding, what you are wearing, what holds what, and what a name
resolves to.

Read [`27`](27-model-api.md) first.

---

## Hands — `state/hands.rs`

`Hand` is **three answers, not two**: `Unknown`, `Empty`, `Holding { id, noun,
name }`. A hand nobody has reported is not an empty hand.

> **A Rule 2.2a loss, found by building on top rather than by audit.** The hands
> dropped the `exist` id the wire sends: `Frame::LeftHand` preserved it and
> `GameState::apply` matched `{ item, .. }`. Nobody noticed until
> `hand_holding(id)` could not be written.

**Reconnect: kept.** *"Nothing empties them because a socket dropped"*, and the
burst sends real contents — `<left exist=…>plain gift`, `<right>Empty` — not
placeholders.

## Inventory — `state/inventory.rs`

Containers and their contents, from `Frame::Container`, `ContainerItem`,
`ClearContainer`, `DeleteContainer`, `InventoryManager`, `InventoryViewItem`.

**Reconnect: cleared.** Container CONTENTS are not re-sent, and Lich drops them
for the same reason (`inventory.rb:1014-1045`) — *"no stale container mirror
survives"*.

## The worn snapshot — `state/inventory_snapshot.rs`

What `Component{inv}` last showed: the worn-items list.

**Reconnect: kept.** A logged-off character gains and loses nothing, it is not
in the login burst, and it is point-in-time by contract either way.

> This is where M1's first live session showed its seam: worn inventory arrived
> as `a` + `pebbled grey leather doublet` **split at a link boundary**, printing
> down the screen. A frame boundary is not a line boundary — the fix is
> `LineAssembler`, and `ends_line` is what says where a line really ends.

## Containers — `state/containers.rs`

| Field | Wire source |
|---|---|
| `stow`, `stow_checked` | `stow list` |
| `ready`, `ready_checked` | `ready list` |
| `store` | |

**Reconnect: kept, and the failure avoided is silent.** *Neither list is
re-sent by the login burst* — only `stow list` and `ready list` teach them — so
clearing here would leave a behavior with no stow container and **no event ever
coming to restore one**.

> **`Containers::checked` reports whether the list was ever taught**, not
> whether the items still exist. Lich's `stash.rb` re-checks against
> `GameObj.inv` and clears the flag if one has gone — that is a *cache-coherence*
> check and it belongs to whoever owns inventory, not to a record of what the
> game said.

### The three-way split this port found

> **`stash.rb`, `bank.rb` and `fog.rb` each turn out to be two files wearing one
> name.** MEASURED for stash: **13 pure functions against 12 with 45
> send/wait/retry calls between them.**
>
> The **reading** half answers questions about the character and is built. The
> **sending** half issues commands and waits — it needs the authority token and
> roundtime, would put `fput` in a crate with no socket, and is **M6**.
> `plan/20` §0b tables what remains so it is recorded rather than forgotten.

## Bank — `state/bank.rs`

What `bank account` last reported.

**Reconnect: kept.** Silver on deposit is not connection state: nobody spends it
while you are logged off, and the figure is re-read only by a `bank account`
command, never by the burst. Clearing would leave a behavior believing the
account empty with no event coming to correct it.

> **The bank's indentation guard was one of four tests that passed a mutation
> because the input never reached the code.** A green mutation run says *look at
> the input*, not just the assertion.

## Currency — `character/currency.rs`

What `wealth` reported. Coins on you, as distinct from the bank above.

## Names to things — `state/nouns.rs`, `resolve.rs`, `gameobj.rs`

`plan/12` §2e: *"every interactable carries a stable id... it is what makes 'the
doublet' resolvable to a thing"*.

The **capture** of `exist=`/`noun=` already happened in steps 2 and 4 — every
`RoomItem` in the room and in every container carries both. What `nouns.rs`
adds is the **lookup**: `Found`, `Where`, and a search across room, hands and
containers.

> **A link nested inside a clickable `<d>` reached no consumer at all.**
> MEASURED: **322 occurrences across 62 of 208 live logs**, and **zero** in the
> committed fixtures — which is why the golden corpus did not catch it. A
> fixture set proves what it contains.

## Disk — `state/disk.rs`

Your disk, by its nouns (`DISK_NOUNS`).

---

## Reconnect, in one table

| | Reconnect | Why |
|---|---|---|
| `left_hand`, `right_hand` | **kept** | the burst sends real contents, not placeholders |
| `inventory` | **cleared** | container contents are not re-sent |
| `inventory_snapshot` | **kept** | a logged-off character gains and loses nothing |
| `containers` | **kept** | only `stow list`/`ready list` teach them; nothing would restore one |
| `bank` | **kept** | nobody spends your silver while you are gone |

**The contrast with `cooldowns` is the point** (see [`27g`](27g-model-api-features.md)):
the four keeps above are facts about *you* that nothing changes while you are
away. A cooldown is a stamp on *someone else*, taken against a clock this
session was keeping, and both assumptions break at once.
