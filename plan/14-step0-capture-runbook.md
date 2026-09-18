# Step 0 — Capture a Lich login

**Zero Rust. One evening.** Settles three of the four wire unknowns blocking the login
implementation (`plan/10` §12.1). Do this before writing any protocol code.

Tooling is ready (verified 2026-09-18): **TShark 4.6.0**, Npcap installed, and
`C:\Program Files\Wireshark` added to the **user PATH** — `tshark` works in any new shell.

---

## What this answers

| # | Question | How this capture answers it | Blocks |
|---|---|---|---|
| **S1** | Does Lich send SNI? | read the `ClientHello` | **the highest-value unknown** — rustls sends SNI by default; if `eaccess.play.net`'s load balancer routes on it, Cena reaches a different backend than Lich |
| **S2** | What TLS version is negotiated? | read the `ServerHello` | the `rustls` vs `native-tls` decision |
| **S4** | Per-response terminators; does the `K` response end with `\n`? | needs **plaintext**, see §3 | framed reads; a Rust port that mishandles this diverges only for long passwords |
| S3 | Out-of-range password bytes | **not answerable by capture.** Ruby raises before sending, so Lich has never emitted one. `10` §12.1 stands; Cena replicates Ruby's failure rather than wrapping. | — |

---

## 1. Capture

```powershell
tshark -D                      # list interfaces; note the number for your NIC
```

Then, capturing only EAccess traffic so the file stays small and contains nothing else:

```powershell
tshark -i <N> -f "port 7910 or port 7900" -w E:\Cena\capture\eaccess.pcapng
```

Start the capture, run a normal Lich login, stop with `Ctrl+C` once the game connects.

> **Filter on PORT, not host.** A host filter resolves the name once at start time, and
> `eaccess.play.net` is an AWS load balancer that resolved to two addresses
> (`3.22.54.28`, `18.118.231.211`) on 2026-09-18 — a name-based filter can miss the connection
> entirely and produce a 0-packet file. EAccess uses **:7910 (TLS)** and **:7900 (cleartext
> fallback)**, so a port filter catches it regardless of which IP answers, and still records
> nothing else.
>
> **Check the interface before capturing.** `Find-NetRoute -RemoteIPAddress 3.22.54.28` names
> the adapter traffic will actually use; a VPN going up or down changes it. A capture on the
> wrong interface yields 0 packets, and `tshark -r <file> | Measure-Object -Line` is how you
> confirm that before reading anything into a blank result. The file still contains your encrypted login — treat it as sensitive and do not
> commit it. `capture/` is gitignored.

## 2. Read the answers — S1 and S2

**S1, SNI:**

```powershell
tshark -r E:\Cena\capture\eaccess.pcapng `
  -Y "tls.handshake.type == 1" `
  -T fields -e tls.handshake.extensions_server_name
```

- **Blank** → Lich sends no SNI, confirming the spec's reading of `eaccess.rb` (it never sets
  `ssl_socket.hostname`). **Then Cena must suppress SNI too**, or accept that it may land on a
  different backend. With `rustls` that means a custom `ServerName` path or `dangerous()`
  config; `native-tls`/OpenSSL makes it easier.
- **`eaccess.play.net`** → the spec's reading is wrong and Rust's default is correct. Amend
  `10` §2 and delete the concern.

**S2, negotiated TLS version and cipher:**

```powershell
tshark -r E:\Cena\capture\eaccess.pcapng `
  -Y "tls.handshake.type == 2" `
  -T fields -e tls.handshake.version -e tls.handshake.ciphersuite
```

If it negotiates below TLS 1.2, `rustls` will not talk to it at all and the decision is made
for us.

## 3. S4 — terminators, which needs plaintext

The capture is encrypted, so the handshake bytes are not directly readable. Two ways, in order
of preference:

**(a) Instrument Lich (simplest, most reliable).** In `reference/lich-5/lib/common/authentication/eaccess.rb`,
`EAccess.read(conn)` is the single funnel for every response. Add a temporary debug line that
writes `response.inspect` to a file — `.inspect` escapes `\n`, `\r` and `\t` visibly, which is
exactly what S4 needs. Run one login, read the file, revert the change.

This yields the terminator for **every** response (`K`, `A`, `M`, `F`, `G`, `C`, `L`), not just `K`.

**(b) Keylog + Wireshark decryption**, if you would rather not touch Lich:

```powershell
$env:SSLKEYLOGFILE = "E:\Cena\capture\keys.log"
```
…then point Wireshark at it (*Preferences → Protocols → TLS → (Pre)-Master-Secret log*). Ruby's
OpenSSL does not honor `SSLKEYLOGFILE` by default, so **(a) is the realistic route.**

## 4. Record the results

Amend `plan/10` §12.1, changing each of S1, S2, S4 from **UNVERIFIED** to **VERIFIED** with the
observed value, per the evidence rules (`05` §−2: cite the measurement, not the memory).

**Done when:** S1, S2 and S4 are VERIFIED or amended, and S3 is explicitly recorded as
unobservable-by-design.

## 5. Then

- **Step 1** — the login spike (`plan/10` §11).
- **Step 2** — the Milestone 1 slice (`plan/12` §7, narrowed by §9c).

---

## Safety

- The capture contains a real login. **Do not commit it** (`capture/` is gitignored) and delete
  it once the four answers are recorded.
- Use the free-to-play test account rather than a main account.
- If you instrument `eaccess.rb`, **revert it** — that tree is a read-only reference clone and a
  stray debug write would contaminate later greps.
