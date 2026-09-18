# 10 — EAccess / Login: Reimplementation Specification

**What this covers.** The complete authentication path for Cena: the SGE wire protocol on
`eaccess.play.net:7910`, the HTTPS web-login fallback, credential storage, and the handoff from a
successful auth to a live game socket — specified precisely enough to implement in Rust without
reading the Lich Ruby.

**What happens next** — everything that arrives on the game socket after the handoff — is
[`plan/15-wrayth-protocol.md`](15-wrayth-protocol.md). This document ends where that one begins:
at `<mode id="GAME"/>`.

**Risk assessment.** The cryptography and framing are now fully pinned and verified by execution
(§2, §3) and are safe to implement from; what remains genuinely uncertain is a small, enumerable
set of wire questions — SNI, TLS version floor, response terminators, and out-of-range password
bytes — every one of which the Phase 2 spike (§10) exists to settle before a line of production
login code is written. `01-architecture.md` called EAccess "the highest-risk reimplementation";
after this dig the risk is no longer *unknown protocol*, it is *four measurable unknowns*.

Source of record: `E:/Cena/reference/lich-5` at commit `236a9a2`. Citations are `file.rb:line`,
relative to that tree. Four independent digs produced this document and each was adversarially
verified; **where a verifier corrected an analyst, the corrected version is what appears below**,
flagged inline as `[verifier-corrected]`.

---

## 0. Corrections to the working assumptions

Three premises that were in circulation before this dig are wrong, and each changes the plan.

**0.1 — There is no cleartext `:7900` fallback in Lich.** PR #1569 ("fall back to cleartext SGE
auth on 7900 when the TLS port is unreachable") is **CLOSED, NOT MERGED** (`gh pr view 1569` →
`"state":"CLOSED"`, `"mergedAt":null`, closed 2026-09-08T21:47:37Z, "Closing in favor of #1570").
Code search confirms: `grep -rn "cleartext_socket\|secure_socket" lib/ spec/` returns nothing, and
`7900` appears nowhere in `lib/`. **Lich has exactly two auth paths: TLS SGE on 7910, and HTTPS
web-login.** See §4.2 for why 7900 was abandoned — the field evidence is the most useful part.

**0.2 — Certificate "verification" is trust-on-first-use with silent auto-re-pin**, not
verification. It is nominally `VERIFY_PEER`, but the bootstrap that decides what to pin runs with
`VERIFY_NONE`, and any later mismatch silently re-pins. §2.3.

**0.3 — The crypto home is not `entry_store.rb`.** The brief pointed at
`entry_store.rb` (1,131 lines) as the credential-crypto file. It is a YAML persistence, migration
and favorites layer. The actual cryptography lives in three files under `lib/common/gui/`:
`password_cipher.rb` (152), `master_password_manager.rb` (220),
`windows_credential_manager.rb` (241). §5.

---

## 1. The protocol at a glance

```
TCP connect eaccess.play.net:7910    (5s connect timeout)
TLS handshake                        (pinned self-signed cert, no SNI, no hostname check)
  → K                                get 32-byte hash key
  ← A  <account> <obf-password>       authenticate
  ← M                                 list games
  ← F  <game_code>                    entitlement check
  ← G  <game_code>                    (response discarded, but required)
  ← P  <game_code>                    (response discarded, but required)
  ← C                                 character list → resolve name → char code
  ← L  <char_code> STORM              → KEY=..., GAMEHOST=..., GAMEPORT=...
close socket                          (single-use; ensure-closed on every path)

rewrite GAMEHOST/GAMEPORT             (Lich.fix_game_host_port — mandatory)
TCP connect game host/port
  → <KEY>\n
  → /FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML\n
  → sleep 300ms; <c>\n ; sleep 300ms; <c>\n
live
```

Command order is **strict**. `G` and `P` responses are read and entirely discarded by Lich, but
they **must still be sent** — the server's state machine expects them. `F` resets game-selection
context, so `G`/`P`/`C` must be re-issued for a different game code.

---

## 2. Connection and TLS

### 2.1 Endpoint and timeouts

```ruby
# eaccess.rb:100, :145
def self.download_pem(hostname = "eaccess.play.net", port = 7910)
def self.socket(hostname = "eaccess.play.net", port = 7910)
```

| Constant | Value | Source |
|---|---|---|
| Host | `eaccess.play.net` | eaccess.rb:145 |
| Port | `7910` (TLS over TCP) | eaccess.rb:145 |
| `PACKET_SIZE` | `8192` | eaccess.rb:23 |
| `CONNECT_TIMEOUT` | `5` seconds — **TCP handshake only** | eaccess.rb:36 |
| Overall watchdog | `30` seconds default, `auth_with_timeout` | eaccess.rb:372 |

The endpoint resolved to `3.22.54.28` (AWS) during the 2026-09-08 outage — infrastructure has
moved off Simutronics' own address space.

The `CONNECT_TIMEOUT` comment encodes a production failure (eaccess.rb:29-36):

> Bounds the TCP connect to eaccess.play.net:7910 so a silently-dropped SYN (firewalled/blocked,
> no RST) fails in seconds instead of hanging on the OS connect timeout (commonly ~75s on Linux,
> driven by tcp_syn_retries) — observed live when the port is unreachable but not actively refused.

**There are no read timeouts anywhere.** `CONNECT_TIMEOUT` covers only the TCP handshake; the TLS
handshake and every protocol read are unbounded blocking calls, covered solely by the 30s
whole-exchange watchdog. **Cena must add an explicit per-read deadline that Lich lacks.**

### 2.2 TLS context — exact

```ruby
# eaccess.rb:145-174
socket                  = Socket.tcp(hostname, port, connect_timeout: CONNECT_TIMEOUT)
cert_store              = OpenSSL::X509::Store.new
ssl_context             = OpenSSL::SSL::SSLContext.new
ssl_context.cert_store  = cert_store
ssl_context.verify_mode = OpenSSL::SSL::VERIFY_PEER
cert_store.add_file(pem) if pem_exist?
ssl_socket              = OpenSSL::SSL::SSLSocket.new(socket, ssl_context)
ssl_socket.sync_close   = true
ssl_socket.connect
EAccess.verify_pem(ssl_socket)
```

Four properties matter, and all four are places Rust will differ by default:

- **No SNI.** `ssl_socket.hostname =` is never assigned. Ruby's OpenSSL binding omits the SNI
  extension unless you set it explicitly, so Lich's ClientHello carries **no server_name**.
- **No hostname verification.** `verify_hostname` is never set, `post_connection_check` is never
  called. `VERIFY_PEER` here means only "the chain validates against the store."
- **Empty root store.** `OpenSSL::X509::Store.new` does *not* call `set_default_paths`. The only
  trusted certificate in the universe for this connection is `simu.pem`.
- **No version floor or cipher list.** Ruby's `SSLContext::DEFAULT_PARAMS` disables SSLv2, SSLv3
  and compression only — TLS 1.0 and 1.1 remain permitted.

### 2.3 The pinning model — TOFU with silent auto-re-pin

Pin location: `File.join(DATA_DIR, "simu.pem")` (eaccess.rb:90-92).

Bootstrap (eaccess.rb:100-122) — **the hole**:

```ruby
ctx  = OpenSSL::SSL::SSLContext.new     # verify_mode defaults to VERIFY_NONE
sock = Socket.tcp(hostname, port, connect_timeout: CONNECT_TIMEOUT)
ssl  = OpenSSL::SSL::SSLSocket.new(sock, ctx)
ssl.connect
File.write(pem, ssl.peer_cert)
```

Verification (eaccess.rb:125-142):

```ruby
def self.verify_pem(conn)
  if !(conn.peer_cert.to_s == File.read(pem))
    Lich.log "warn: EAccess stage 'cert_pin_mismatch' -- ... re-pinning automatically. ..."
    download_pem
  else
    return true
  end
end
```

Stated plainly: the very first connection — the one that decides the trust anchor forever —
performs **no certificate verification at all**, and on *any* later mismatch the code re-pins from
another unverified connection. An attacker who MITMs one connection installs a persistent pin.
`docs/eaccess-failure-diagnostics.md` calls this "**Ambiguous by design today**."

**[verifier-corrected] `verify_pem` returns `nil` on mismatch, not `false`, and no caller checks
it** (eaccess.rb:127-141; callers at :172 and :202). The mismatch branch ends with `download_pem`,
whose value is `File.write`'s byte count. So a cert mismatch is **non-fatal by construction**: it
logs, re-pins, and **continues on the already-established connection without re-handshaking against
the new pin**. That is strictly worse than "auto-re-pin" — the current session proceeds against the
unverified cert.

`verify_pem` is called twice per auth (eaccess.rb:172 inside `socket`, again at :202 at the top of
`auth`), the second re-reading the file from disk. Redundant.

The comparison is **full PEM string equality** (`peer_cert.to_s == File.read(pem)`) — whitespace,
trailing-byte and line-ending sensitive. On Windows, any CRLF normalization of `simu.pem` causes a
permanent re-pin loop. **Cena should compare a SHA-256 of the DER, not PEM text** (§9.2).

---

## 3. Framing, reads, and the password hash

### 3.1 Framing — bare LF, one per command

Line-oriented, tab-delimited, `\n`-terminated. Every command is `conn.puts "<LETTER>[\t<arg>]*\n"`.

**Verified empirically:** every literal in the source already ends in `\n`, *and* `IO#puts` is used.
Ruby's `IO#puts` appends a newline **only if the string does not already end in one**:

```
io.puts "K\n"   →  "K\n"        (one newline, NOT "K\n\n")
```

So the wire bytes are exactly one `\n` per command. **Rust writes `b"K\n"`. Never `\r\n`.** A
mechanical port that writes the literal *and* adds a newline sends `K\n\n` and desynchronizes the
exchange. Commands affected: eaccess.rb:205, :222, :231, :241, :256, :259, :262, :273.

### 3.2 Reads — a single syscall, and a latent bug

```ruby
# eaccess.rb:353-355
def self.read(conn)
  conn.sysread(PACKET_SIZE)      # 8192
end
```

One `sysread` per protocol step, **no read-until-delimiter loop**. It assumes every response
arrives in a single TCP segment. `docs/eaccess-failure-diagnostics.md` admits this:

> **Not implemented (out of scope for this pass):** distinguishing a truncated/partial `sysread`
> from a genuine short response — `EAccess.read` remains a single `sysread(PACKET_SIZE)` call. If a
> response is ever found to legitimately span multiple TCP segments in practice, this would need a
> read-until-delimiter loop, not just better logging.

**Cena must implement framed reads** — and doing so requires knowing the real terminator for each
response, which nobody has documented. That is spike item S4 (§10).

### 3.3 The password hash — the one operation that must be exactly right

For each byte index `i` of the password:

```
out[i] = ((password[i] - 32) XOR key[i]) + 32
```

```ruby
# eaccess.rb:216-219
password = password.split('').map { |c| c.getbyte(0) }
hashkey  = hashkey.split('').map  { |c| c.getbyte(0) }
password.each_index { |i| password[i] = ((password[i] - 32) ^ hashkey[i]) + 32 }
password = password.map { |c| c.chr }.join
```

Properties, each verified by running the Ruby:

1. **The key is used POSITIONALLY, index `i` to index `i` — not cyclically, not repeating.**
   There is no key wraparound. eaccess.rb:218.
2. **No masking, no modulo, no `& 0xFF`.** Ruby's Integers are unbounded.
3. **Password longer than the 32-byte key → crash.** `hashkey[i]` is `nil` and Ruby raises
   **`TypeError: nil can't be coerced into Integer`** at the coercion in `(password[i] - 32) ^ nil`.
   **[verifier-corrected]** — it is *not* `NoMethodError`, which matters for log-grep diagnosis.
   Cena must bound password length against key length explicitly.
4. **Out-of-range results raise, in BOTH directions. [verifier-corrected]** `String#chr` raises
   `RangeError` above 255 *and* below 0. Brute-forcing all 65,536 byte pairs of
   `((p-32)^k)+32` gives a range of **-224 .. +287**, with `RangeError` on **14,336 pairs — 7,168
   from overflow and 7,168 from underflow**. Examples: `((255-32)^32)+32 = 287` → "287 out of char
   range"; `((0-32)^65)+32 = -63` → "-63 out of char range"; the minimum `p=0,k=224` → -224.
   An implementation that guards only `> 255` is still broken. The bound is `0..=255` on both ends.
5. **The result is raw bytes, not text.** `String#chr` yields ASCII-8BIT for values ≥128 and the
   interpolated `A` line stays binary.

**Worked test vector** (synthetic — no real credentials anywhere in this document):

```
password = "P@ssw0rd!"
key      = "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"   # the spec fixture, eaccess_spec.rb:278
result   = [145, 130, 48, 55, 50, 118, 53, 44, 104]
```

Both the analyst and the verifier produced these bytes independently by execution. **Use this as
Cena's unit-test vector.**

**PORTING HAZARD #1, and it is the worst one in the whole document.** The obfuscated password
**must be written as raw bytes (`&[u8]` / `Vec<u8>`), never assembled into a Rust `String`.** Values
above 127 appear routinely (145 and 130 in the vector above). Building the `A` line as a `String`
UTF-8-encodes 145 into `0xC2 0x91` and login fails. Assemble the whole `A` line as `Vec<u8>`.

**PORTING HAZARD #2 — non-ASCII passwords are silently mangled by Lich.** `password.split('')`
splits into **characters**, then `getbyte(0)` takes only the **first byte of each**. A multi-byte
UTF-8 password therefore loses all continuation bytes. Rust's natural `.bytes()` iteration would
*not* reproduce this. INFERRED (moderate confidence): Simutronics constrains passwords to printable
ASCII server-side, so this never fires in practice — but no charset constraint appears anywhere in
the source. Spike item S3 (§10).

---

## 4. The commands, one by one

Non-legacy path, `eaccess.rb:204-295`. Order: **K → A → M → F → G → P → C → L**.

### 4.1 `K` — hash key challenge

```ruby
# eaccess.rb:204-214
conn.puts "K\n"
key = EAccess.read(conn)
raise AuthenticationError, "MALFORMED_K_RESPONSE" if key.to_s.strip.empty?
```

- **Send:** `K\n`
- **Receive:** a 32-byte random key string (`docs/eaccess-protocol-analysis.md`: "Response: 32-byte
  random hash key string"). Spec fixture `'ABCDEFGHIJKLMNOPQRSTUVWXYZ012345'`, eaccess_spec.rb:278.
- **The raw response is used verbatim — it is NOT stripped.** `key.to_s.strip.empty?` is only an
  emptiness guard; `hashkey` retains whatever the server sent, and *that* is what gets byte-indexed.
- **Whether the K response carries a trailing newline is UNRESOLVED.** See §11.1 — this is the
  single most dangerous open question in the document. The code cannot distinguish, because
  positional indexing only ever reaches index `len(password)-1`.

### 4.2 `A` — authenticate

```ruby
# eaccess.rb:221-228
conn.puts "A\t#{account}\t#{password}\n"
response = EAccess.read(conn)
unless /KEY\t(?<key>.*)\t/.match(response)
  error_code = response.split(/\s+/).last
  raise AuthenticationError, error_code
end
```

- **Send:** `A\t<ACCOUNT>\t<OBFUSCATED_PASSWORD_BYTES>\n` — as raw bytes (§3.3).
- **Success response shape:** `A\t{ACCOUNT}\tKEY\t{SESSION_KEY}\t{ACCOUNT_HOLDER_NAME}`
- **The session key from `A` is captured and then entirely discarded.** The real key comes from `L`.
- **The success regex requires a TRAILING TAB after the key**, i.e. at least one more field after
  `KEY\t<sessionkey>`. Per the protocol doc that field is the account holder's real name.
  **[verifier-corrected: confirmed empirically, not theoretical]** —
  `/KEY\t(?<key>.*)\t/` matches `"A\tACCT\tKEY\tSESSIONKEY\tHolder\n"` but does **not** match
  `"A\tACCT\tKEY\tSESSIONKEY\n"`. If Simutronics ever drops the holder name, a successful auth is
  misread as a failure — and the failure path then reports `.split(/\s+/).last`, **which would be
  the session key itself, logged**. **Cena must parse positionally, not by this regex.**
- **Failure:** `error_code = response.split(/\s+/).last` — split on any whitespace run (tabs and
  newline), take the last token.
- **Error tokens** (`KNOWN_REJECTION_TOKENS`, eaccess.rb:43): `REJECT`, `NORECORD`, `INVALID`,
  `PASSWORD`. Matched by **substring containment** (`.include?`), not equality.

### 4.3 `M` — game list

```ruby
# eaccess.rb:230-236
conn.puts "M\n"
m_response = EAccess.read(conn)
raise StandardError, m_response unless m_response =~ /^M\t/
```

- **Send:** `M\n`; **receive:** `M\t{CODE1}\t{NAME1}\t{CODE2}\t{NAME2}...`
- **[verifier: load-bearing and under-stated]** On the non-legacy path the `response` variable is
  reassigned by `F` (:242) and again by `C` (:263), and it is **the `C` value that
  `resolve_char_code` consumes at :270**. The variable reuse *is* the contract. A porter who
  carefully preserves M's value in its own variable and passes the wrong one to character
  resolution gets `CHARACTER_NOT_FOUND` on every login.

### 4.4 `F` — entitlement check

```ruby
# eaccess.rb:241-252 (approx)
conn.puts "F\t#{game_code}\n"
response = EAccess.read(conn)
```

- **Success match (normal path):** `/NORMAL|PREMIUM|TRIAL|INTERNAL|FREE/` — **unanchored**.
- **`NEW_TO_GAME` is tolerated ONLY on the generator path.** Getting this wrong blocks character
  creation on every instance the account does not already hold — which is exactly what the
  generator exists for. eaccess.rb:249; `docs/eaccess-protocol-analysis.md:93`.
- **Legacy path differs:** a non-matching `F` **silently skips that game** rather than raising.

### 4.4a Game codes are CASE-SENSITIVE, and a wrong one fails four commands later

**VERIFIED 2026-09-18** by spike run against the live server.

`M` lists the codes uppercase (`DR`, `DRT`, `GS3`, `GST`, `GSX`, …). **A lowercase code is not
recognised — and the server does not say so at the point of use.** `F\tgs3` still returns a
well-formed `F\tPREMIUM`: the server answers about the **account's default instance**, not the one
requested. The session then proceeds silently pointed at the wrong game:

| Command | Sent | Got | Looked like |
|---|---|---|---|
| `F` | `F\tgs3` | `F\tPREMIUM` | fine — but GST is documented **FREE** tier |
| `G` | `G\tgs3` | `X\tPROBLEM` | a `G` failure; it is **not a `G` response at all** |
| `C` | `C` | `C\t1\t16\t1\t1\t…` | fine — the count differed from the working login's (§4.6a; the count reflects **entitlement**, not instance) |
| `L` | `L\t{code}\tSTORM` | `L\tPROBLEM\t3` | an entitlement or launch failure |

Only the last line is loud, and it names the wrong cause. **Two rules follow, and they are the
substance of this section:**

1. **Validate the game code against `M` before using it.** `M` is the authoritative list of codes
   the server accepts. Refuse an unlisted code immediately rather than discovering it at `L`.
   Do **not** silently uppercase inside the protocol layer — normalise at the input boundary
   (config load, CLI parse) where the user can be told, so the wire layer stays a faithful
   transcript of what was asked for.
2. **Every response must be checked for answering the command that was sent, before any field of it
   is read.** Each response echoes its command letter (`F\t…`, `G\t…`, `C\t…`, `L\t…`), so the
   check is one `starts_with`. Without it a desynchronised stream is indistinguishable from a
   server refusal, and a positional read (`split('\t').nth(2)`) turns a rejected command into a
   plausible-looking field value.

> **This cost most of a debugging session.** Four successive theories — entitlement, `P` side
> effects, session-state drift, instance mismatch — were each built on responses that were never
> checked for being answers to the commands actually sent. The `L` request bytes were correct
> throughout, which is why byte-diffing them against a working Lich login found nothing.

### 4.5 `G` and `P` — sent, responses discarded

```ruby
conn.puts "G\t#{game_code}\n"   # eaccess.rb:256
EAccess.read(conn)              # discarded
conn.puts "P\t#{game_code}\n"   # eaccess.rb:259
EAccess.read(conn)              # discarded
```

Lich reads and throws both away.

**They must still be sent.** `G` in particular is the **select-game** command
(`docs/eaccess-protocol-analysis.md:106`) — it is what points the session at an instance, and
everything after it (`C`'s character list, and therefore `L`) depends on it having succeeded.

> **A retracted claim, kept as a worked example of a bad diagnosis.**
>
> On 2026-09-18 this section briefly read: *"`P` is not inert — it moves the selected instance as a
> side effect; Cena does not send `P`."* The reasoning was that in a captured Lich login, `P` asked
> about **GST** answered `P\tGSX\t1000\tGSX.EC\t-1\tGS3.P\t2500` — GST's documented pricing
> (`analysis:146`) under a **GSX** code.
>
> **Tested and DISPROVEN the same day.** A run with `P` removed entirely produced the identical
> failure. `P` was innocent. The GSX echo remains unexplained and is now filed in §12.2, where it
> belongs.
>
> **The actual cause was a lowercase game code** (§4.4a). Every symptom the `P` theory was invented
> to explain — wrong entitlement tier, wrong slot count, `L\tPROBLEM\t3` — followed from that.
>
> The lesson is not about `P`. It is that **four consecutive theories were built on responses that
> were never checked for being answers to the commands actually sent.** See §4.4a.

### 4.6 `C` — character list

```ruby
conn.puts "C\n"                 # eaccess.rb:262
response = EAccess.read(conn)
```

Parsing (`resolve_char_code`, and the two regexes are **exact, both spec-pinned regression guards**):

```ruby
# strip the header — note [\t\n]: tab OR newline terminator
/^C\t[0-9]+\t[0-9]+\t[0-9]+\t[0-9]+[\t\n]/

# scan code/name pairs — note the second class is [^\t\n], NOT [^\t^\n]
/[^\t]+\t[^\t\n]+/
```

- The `[\t\n]` alternation in the header strip exists for the **empty-account case**, where the
  response is `"C\t0\t0\t0\t0\n"` with no pairs following.
- The `[^\t\n]` class (not `[^\t^\n]`) is a **regression guard for a character named `Foo^Bar`** —
  eaccess_spec.rb:234-242.
- **Name matching is exact and CASE-SENSITIVE:** `c.split("\t")[1] == character` (eaccess.rb:345).
  Note the web-login path uses `casecmp?` — **case-INSENSITIVE**. The two paths are inconsistent in
  Lich; **Cena must pick one deliberately.**
- **`NEW_CHARACTER_CODE = "0"`** enters the character generator. It is selected by **explicit intent
  only** — a character literally named "New" must resolve to its real code. This is a spec
  regression guard, eaccess_spec.rb:222-231.
- Failure: `AuthenticationError, "CHARACTER_NOT_FOUND"`.

### 4.6a The `C` header names the selected instance — use it as a diagnostic

`C\t{N1}\t{N2}\t{N3}\t{N4}\t...` — **`N2` is the account's max character slots on the currently
selected instance**, and it is a direct readout of *which instance that is*:

| `N2` | Instance |
|---|---|
| **100** | GST (free tier) |
| **16** | premium |

Source: `docs/eaccess-protocol-analysis.md:166`, corroborated `:293`, `:301`.

**This is worth logging on every login.** It is what distinguishes "the server refused" from "we
asked the wrong instance" — the two look identical at `L`, because the character code and the `L`
bytes can be **byte-identical** across instances while meaning different things.

> **Worked example, 2026-09-18.** A spike run and a working Lich login used the same account, the
> same character, and the same code `W_ACCOUNT_000`. The `L` requests were byte-for-byte identical.
> Lich's `C` header reported **100** slots; the spike's reported **16**. Lich launched; the spike got
> `L\tPROBLEM\t3`.
>
> The slot count was the first signal that the session was on the wrong instance — but note that it
> is a *symptom*, not the cause. The cause was a lowercase game code four commands earlier (§4.4a),
> and the check that would have caught it immediately is the response-echo check, not this one.
>
> **AMENDED 2026-09-18, after the first live run of the Cena binary.** This paragraph previously
> ended "this one is cheap and **names the instance**, which is useful in logs regardless," and the
> ported code said the same in stronger terms: *"100 means GST, 16 means a premium instance."*
>
> **The count names the account's ENTITLEMENT, not the selected instance.** It only appeared to name
> the instance because the worked example above holds the account constant and varies the instance —
> so entitlement and instance moved together, and one observation cannot separate two variables that
> never varied independently.
>
> Disproved by the author's own account, which holds **Shattered and Premium**: `M` offers it ten
> codes, `F` answers `PREMIUM`, and a correct `GS3` login reports **16** — the same 16 this spec
> once read as evidence of drift. Nothing had drifted. A bare count on a multi-entitlement account
> distinguishes nothing.
>
> What survives is the *contrast*: a count that disagrees with a previous login **on the same code
> and the same account** is a real signal. Any particular number is not. The instance is confirmed
> by `F`, `G` and `P` each echoing the code that was sent, which is the check to rely on.

### 4.7 `L` — launch

```ruby
conn.puts "L\t#{char_code}\tSTORM\n"          # eaccess.rb:273
l_response = EAccess.read(conn)
# eaccess.rb:279
unless l_response =~ /^L\tOK\t/
  # generator path: raise AuthenticationError, "GENERATOR_NOT_AVAILABLE"
  # normal path:    raise StandardError, l_response
end
parsed = l_response.sub(/^L\tOK\t/, '').split("\t").map { |kv|
  k, v = kv.split("=")
  [k.downcase, v]
}.to_h
LaunchResult.normalize(parsed)                 # eaccess.rb:294
```

Five things here are load-bearing:

1. **The literal `STORM` is always sent, regardless of the user's actual frontend.** Frontend
   rewriting happens later, client-side, in `LaunchData.prepare`. The exact bytes are asserted at
   eaccess_spec.rb:318: `expect(conn).to receive(:puts).with("L\t0\tSTORM\n")`.
2. **The success guard MUST be `/^L\tOK\t/`, never `/^L\t/`.** The failure response `L\tPROBLEM\t1`
   also begins with `L\t`, and a loose guard parses it into garbage.
   **[verifier-corrected]** The garbage hash a loose guard actually produces is
   `{"problem"=>nil, "1\n"=>nil}` — two keys, no `"l"` key, and the last key retains the newline.
   (An earlier writeup claimed `{"l"=>nil,"problem"=>nil,"1"=>nil}`; `sub(/^L\t/,'')` strips the L.)
   The argument is unchanged and the strict guard is spec-confirmed at eaccess_spec.rb:346-356
   (generator context) and :358-371 (normal context) — **two separate contexts, not one range**.
2a. **`PROBLEM` codes: all four are now documented.** **CORRECTED 2026-09-18** — this item
   previously read "only 1 is documented" and instructed the reader not to guess at `PROBLEM 3`.
   That instruction was right at the time and is now superseded by a better source. The
   superseded reasoning is preserved below, because knowing *why* a conclusion was wrong is what
   stops it being re-derived.

   **Source: Saga 0.9.9, Simutronics' own client** (`C:\Gemstone\saga-research`, §4.7a). Saga
   added sub-code handling in 0.9.7. The four meanings below are Saga's own **user-facing
   English strings**, read directly — not minified identifiers, not inference from control
   flow, so there is no transcription risk and nothing was copied. They are **VERIFIED**:

   | `n` | Meaning | Retry? |
   |---:|---|---|
   | **1** | The account's access level does not permit playing this instance — a lapsed or missing subscription — **or** the account service timed out. During character creation, a subscription or trial is required. | **No.** Account state. |
   | **2** | The server has no STORM launch entry for this game's configuration. **Server-side**, not an account problem. | **No.** |
   | **3** | The server has no configuration for the selected game. **Server-side.** | **No.** |
   | **4** | The account service failed while assigning the character. | **Yes** — transient. |

   **This changes retry policy, and that is the operational point.** §9.1's blanket 3-retry is
   wrong for **2** and **3**: both are server-side configuration facts that will not change
   between attempts, so retrying burns three logins to reach the same refusal and delays the
   real message to the user. **4** is the one code that genuinely wants a retry. **1** is an
   account-state refusal that a retry cannot fix, and its message should send the user to their
   subscription rather than to a login loop.

   > **What was previously believed, and why it was wrong.** This spec recorded one observation:
   > `PROBLEM 3` returned when `L` was sent on a session whose instance was never validly
   > selected — after a **lowercase game code** (§4.4a) left `G` rejected and the session on the
   > account's default instance, with `L` bytes byte-identical to a working login's. From that
   > single data point two candidate readings were offered — "code not valid on the selected
   > instance" versus "session not in a launchable state" — and the spec correctly **declined to
   > choose**.
   >
   > **Neither candidate was right.** `PROBLEM 3` means *the server has no configuration for the
   > selected game*. The observation was still sound: a rejected `G` left the session pointed at
   > an instance the server had no launch configuration for, which produces exactly this code.
   > The error was not in the measurement but in the space of hypotheses — both candidates
   > framed it as a **session-state** problem, when it is a **server-configuration** problem.
   > A single observation constrained the cause far less than it appeared to.
   >
   > **This is the case for `plan/05` §−2 stated positively.** The spec labelled its own
   > uncertainty honestly and refused to write a guess into code. Because it did, this
   > correction replaces an admitted gap instead of silently overturning a confident error.

   **Diagnosis order when `L` refuses** — unchanged, and still the first thing to check, because
   §4.4a's failure is what produced the `PROBLEM 3` above: was the game code listed in `M`, and
   did every response echo its own command letter? Byte-diffing the `L` request remains a dead
   end; its bytes are correct in exactly this failure.

3. **`L\tPROBLEM` produces two different failure behaviours** from the same server response
   (eaccess.rb:279-286). Generator path → `AuthenticationError, "GENERATOR_NOT_AVAILABLE"`, which is
   in `FATAL_ERROR_CODES` → no retry, no web fallback. Normal path → bare `StandardError,
   l_response`, which is **not** fatal and does **not** respond to `error_code` → full 3 retries,
   then falls through to WebLogin.
4. **`kv.split("=")` has NO limit.** Ruby's `String#split` with no limit discards everything after
   the second `=`. If a KEY value ever contains `=`, the key is **silently truncated** and login
   fails with no error. **Rust must use `splitn(2, '=')`.** eaccess.rb:291.
5. **Keys are downcased at parse time** (`[k.downcase, v]`, :292), *before* `LaunchResult.normalize`.
   Normalize is idempotent on this path; it is not the sole case authority.

**The final field retains its terminating newline**, so `parsed["key"] == "deadbeef\n"`. Lich chomps
it far downstream — but **[verifier-corrected] only on some paths**: `extract_game_key`
(main.rb:338-347) is invoked at :376 **only `if requires_game_key`**, which is set for
`--without-frontend`, SUKS, native SAGA and custom_launch (:350-368) — *not* for the default
launcher path. **Cena must trim it unconditionally, or it sends `KEY\n` to the game server.**

**The socket is closed on every exit path**, including exceptions:
`ensure conn&.close unless conn&.closed?` (eaccess.rb:330-331). **The SGE connection is strictly
single-use per auth** — you cannot hold it open and re-issue `F/G/P/C` for a second game code.

### 4.7a Saga as a source for the login layer — and its licensing constraint

**Added 2026-09-18.** Saga is Simutronics' own Electron client for GemStone IV and DragonRealms,
studied read-only at `C:\Gemstone\saga-research` (versions 0.9.1 and 0.9.9). It performs the
same eAccess exchange this document specifies, against the same server.

**Licensing — binding, not advisory.** Saga is proprietary (`"license": "UNLICENSED"`). The
rule, from the research folder's own README: read it to learn **how the protocol behaves**;
never copy code or data. **Facts about the wire protocol are fine; their implementation is
not.** Everything below is a protocol observation in this document's own words. No Saga
fragment appears anywhere in `plan/` or in any Cena source file. `credentials.enc`,
`passwords.enc` and everything under `AppData\Roaming\saga` were not read, and nothing in the
research folder was modified.

**Why it is worth citing here at all.** `plan/10` was reimplemented from Lich, a *third-party*
client. Saga is a *first-party* one. Where Lich shows what one community implementation happens
to do, Saga shows what Simutronics expects — and on `L\tPROBLEM` it simply knows more, because
Lich never enumerated the sub-codes at all (§4.7 item 2a).

**What Saga contributes, all VERIFIED from plain-English user-facing strings rather than from
minified identifiers or inferred control flow:**

1. **The four `PROBLEM` sub-code meanings and their retry split.** §4.7 item 2a. This is the
   headline, and it corrects a documented gap.

2. **`L\tOK` response validation.** Saga requires **`KEY`, `GAMEHOST` and `GAMEPORT` to all be
   present**, and validates `GAMEPORT` as an **integer in 1–65535**, refusing the launch
   otherwise. This independently corroborates §4.7 item 4's warning: Saga splits each field on
   the **first** `=` only, which is exactly the `splitn(2, '=')` behaviour Rust must use and
   which Lich's unlimited `split("=")` gets wrong.

3. **A third handshake write that this spec does not mention.** On the **non-DragonRealms**
   branch — selected by testing whether the game code begins with `DR` — Saga writes **three
   things** to the game socket: the key with a CRLF, then the
   `/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML` banner with a CRLF, then **a bare CRLF on
   its own**. That third empty line is new information here. §7.4 and `plan/15` §1.2 specify the
   first two; neither mentions the third.

   **UNVERIFIED against the live wire:** whether the bare CRLF is *required*, or merely
   harmless. It is one line to send and costs nothing, and a first-party client sends it
   unconditionally on this branch — so **send it**, and record that its necessity was never
   tested. The DragonRealms branch differs in every particular (bare LF rather than CRLF, its
   own banner, and bare LF rather than CRLF); DragonRealms is deferred
   (`plan/12` §9d), so that path is not Cena's problem.

   > **CORRECTED 2026-09-18.** This read **"GemStone sends no `<c>`"** and the spike was
   > edited to match. **Both were wrong.** VellumFE -- a working GemStone client, by this
   > author, against these same servers -- sends **two `<c>` ready signals ~300 ms apart,
   > unconditionally, with no DragonRealms branch**
   > (`reference/VellumFE/src/network.rs:689-694`, whose own comment reads "Send ready
   > signals - game server expects two `<c>` signals with delay").
   >
   > The claim came from Saga research plus inference, and the spike edit that followed it
   > **was never run against the live server** -- §12's VERIFIED marks all predate it. So a
   > working handshake was changed on theory and recorded as fact.
   >
   > `CLAUDE.md`'s rule is to read VellumFE **first** -- before Lich, before this spec,
   > before theorising -- and it already records that the rule was broken three times during
   > this spike, each time with the answer sitting in `network.rs`. This was the fourth, and
   > it is the reason the rule is written in capitals.
   >
   > Restored in `spike/eaccess-spike/src/main.rs`. Vellum also builds `/P:` from
   > `std::env::consts::OS` rather than hardcoding `WIN_UNKNOWN`; the spike still hardcodes
   > it, which is fine for a Windows spike and wrong for the port.

   > **VERIFIED 2026-09-18 by live A/B against the real server**, run twice by the author
   > with a `CENA_SKIP_C=1` toggle:
   >
   > | `<c>` sent | Result |
   > |---|---|
   > | yes | PASS -- **2,347 bytes**, `<mode id="GAME"/>`, XML mode enabled |
   > | no  | PASS -- **163 bytes**, `<mode id="GAME"/>`, `<playerID>`, `<settingsInfo>` |
   >
   > **The `<c>` signals are NOT required to log in. Both runs reach game text.** So the
   > earlier claim ("GemStone sends no `<c>`") was not wrong about login working -- it was
   > unsupported, and removing them from a working handshake on inference was still the
   > error.
   >
   > **What differs is VOLUME: 2,347 bytes versus 163**, and the dumps say exactly what the
   > extra 2,184 bytes are. The `<c>`-less run stops at `<settingsInfo>`. The `<c>` run
   > continues into the **entire login burst**:
   >
   > ```
   > <mode id="GAME"/> + "Welcome to GemStone IV (R) v5.10"
   > <streamWindow id="main"  title='Story' subtitle=" - Wehnimer's, Erebor Square" .../>
   > <streamWindow id='room'  ... ifClosed='' resident='true'/>
   > <clearStream id='room'/><pushStream id='room'/>
   >   <compDef id='room desc'>  ... with <a exist= noun=> links inline
   >   <compDef id='room objs'>  <compDef id='room players'>
   >   <compDef id='room exits'>Obvious paths: <d>north</d>, <d>south</d>, <d>west</d>
   > <popStream id='room'/>
   > <streamWindow id='inv' .../> ... worn items ... <popStream/>
   > <exposeContainer id='stow'/>
   > ```
   >
   > **So `<c>` is what makes a session USABLE rather than merely connected.** Login
   > succeeds either way, but without it the client never receives the room, the exits or
   > inventory -- and Milestone 1's criterion 2 is "renders a room". **Send them.**
   >
   > Three things this dump settles beyond the `<c>` question:
   >
   > 1. **`<exposeContainer>` arrives**, which is the extended feed (`plan/15` §1.2). The
   >    `/FE:WRAYTH /VERSION:1.0.1.28` banner is doing its job, live, today.
   > 2. **The room arrives as `<compDef id='room desc'>`**, not `<component>`. Cena's
   >    `GameState` folds `Frame::Component { id: "room desc" }` and `look`'s matcher keys
   >    on that same id -- CHECK that `compDef` and `component` reach the same frame, since
   >    the fixtures were cut from corpus files and this is the live login shape.
   > 3. **`<d>north</d>` in `room exits`** -- bare `<d>` with no `cmd=`, the case
   >    `plan/15` §2.4 records, arriving in the very first room of a real session.
   >
   > A note on method, because it nearly went wrong twice. Vellum being right was
   > *evidence*, not proof -- replacing "Saga says no" with "Vellum says yes" would have
   > been the same error pointed the other way. And when the second run appeared to hang,
   > this spec was edited to say **REQUIRED** before the output was seen; it was idling
   > after a successful read, not failing. A live A/B settles a wire question, but only
   > once you have read what it printed.

4. **Two guards on the game socket after launch**, which this spec does not currently specify
   and which turn a silent hang into a diagnosable failure:

   - **The session is not "live" until `<playerID>` arrives.** A **90-second watchdog** closes
     the socket if it never does. Without this, a refused or wedged connection looks
     indistinguishable from a quiet game.
   - **Two server messages mean the key is dead: an invalid-login-key message and a
     please-relogin-to-the-website message.** Either one arriving before `<playerID>` means the
     socket is closed immediately rather than waited on. These arrive as **plain text on the
     game socket**, not as a protocol frame — so the check is a text match, and it is the only
     place in Cena where that is the right tool.

   Both are **cheap and clearly right**, and both are **deferred to the Milestone 1 Step 2
   slice** rather than implemented now: `plan/12` §9c puts the game-socket connect in that step,
   and there is no connect path in-tree yet to attach a watchdog to. Recorded here so it is
   specified before it is written, not retrofitted after a hang.

**What was deliberately NOT taken from Saga into this document.** Its type-ahead pacing, its
send-queue policy and its multibox behaviour are **client policy, not wire protocol**. They
belong in `plan/12` if anywhere. One correction is worth recording, since the figure
circulated: the research README summarises the pacing rule as "cap = N" where N is the number
in the server's refusal text. **VERIFIED that this is not what the code does** — it derives the
cap from N and then clamps the result into a small fixed range, so the README's N is not the
cap. Noted here only so that number is not quoted as though it were protocol. It is not
protocol at all, and neither is the rest of the pacing rule.

### 4.8 The legacy path — different commands, different return shape

Rarely needed, but if Cena ever implements multi-game enumeration:

- **The legacy path issues an `N` command that appears nowhere else.** `eaccess.rb:301-303` sends
  `N\t#{game_code}\n` and gates the entire per-game block on `response =~ /STORM/`. The protocol doc
  documents it at line 100: "Response: Contains STORM if the game supports the Storm/Wrayth
  frontend." Legacy order is **M → [per game: N → F → G → P → C]**.
- **`resolve_char_code` is NOT called.** Legacy inlines its own pair scan at :319-324 and builds
  `{game_code, game_name, char_code, char_name}` hashes, **returning an Array** — never a
  `LaunchResult`, so `REQUIRED_KEYS` normalization never runs. This dual return shape (Hash for
  normal, Array for legacy) caused a real production bug on Ruby 3.2 (§8.6). **A Rust enum makes it
  unrepresentable — do that.**

### 4.9 The launch result

`LaunchResult.normalize` (launch_result.rb:35-42) downcases all keys to Strings, requires only
`key` to be non-blank, and freezes the hash:

```ruby
REQUIRED_KEYS = %w[key]
```

**`GAMEHOST`/`GAMEPORT` are deliberately NOT required**, because the character-generator path
(`L\t0\tSTORM`) can omit them — proven by eaccess_spec.rb:316, where `"L\tOK\tKEY=abc\n"` is a valid
success. **Cena's type must make `gamehost`/`gameport` `Option<>` if it supports the generator.**

Full field set from `L\tOK\t...` (`docs/eaccess-protocol-analysis.md:183-191`):
`UPPORT`, `GAME`, `GAMECODE`, `FULLGAMENAME`, `GAMEFILE`, `GAMEHOST`, `GAMEPORT`, `KEY`.
**Cena needs only `GAMEHOST`, `GAMEPORT`, `KEY`, `GAMECODE`.** The rest exist solely to tell a
Simutronics launcher which `.EXE` to run.

### 4.10 Diagnostic stage taxonomy — worth mirroring

`Thread.current[:eaccess_stage]` (eaccess.rb:79) is set at each step and read back at :386 by the
watchdog to name the stalled stage. Stage names:

```
tcp_connect:pem_bootstrap   tls_handshake:pem_bootstrap
tcp_connect:main            tls_handshake:main
cert_pin_mismatch
k_response   a_response   m_response   entitlement_response   l_response
```

**One wart not to port:** `entitlement_response` spans `F`+`G`+`P`+`C` as a **single stage**
(:240-268), so a `C` failure is reported as an entitlement failure. Give `C` its own stage.

---

## 5. The three auth paths — and which Cena needs

### 5.1 Path A — TLS SGE on 7910 (default). **Cena needs this.**

Everything in §2-§4. This is the primary and, for a single-character login, the only path that
matters when the service is healthy.

### 5.2 Path B — cleartext SGE on 7900. **Cena does NOT need this. Do not build it.**

**It does not exist in Lich.** PR #1569 implemented it, was field-tested during the only recorded
outage, and was **closed unmerged** — because it did not work. The decisive comment, from
`therealatari` (MEMBER) on #1569:

> "I tested the fallback locally against GemStone IV while the normal EAccess path was unavailable.
> The cleartext EAccess endpoint on port 7900 accepted a TCP connection, but it did not send the
> initial usable K challenge / complete the EAccess protocol handshake during our probes.
> Consequently, switching from TLS to cleartext did not produce a login... At the same time,
> Play.net website authentication and Play in Browser remained functional. The website issued a
> one-time game ticket through its web launch path, and feeding that website-issued ticket to the
> existing Lich SAL launch path successfully authenticated to GSIV... The practical limitation is
> that TLS-versus-cleartext fallback only helps when port 7900 is actually serving the legacy
> EAccess handshake; **a TCP accept alone is insufficient.**"

`MahtraDR` (MEMBER) reported the opposite for DragonRealms — "Tested and works during an eaccess tls
outage of DR as a whole" — so **7900 sometimes speaks SGE and sometimes only accepts TCP, and it
differs per game.** Two members' field reports directly conflict. That is the definition of an
unreliable fallback. #1570 (the HTTPS path) was merged at 2026-09-08T21:35:01Z; #1569 was closed
12 minutes later at 21:47:37Z — **[verifier note] the decision was already made when it closed.**

**Design lesson worth keeping regardless of the port: "connected" is not "working."** Any Cena
health check on an auth endpoint must verify a protocol response, not a TCP accept.

### 5.3 Path C — HTTPS web-login on `www.play.net`. **Cena should build this, in Phase 2b.**

`lib/common/authentication/web_login.rb` (576 lines, PR #1570, merged 2026-09-08). Entirely
different mechanism: ASP form POSTs, a session cookie, and a redirect chain whose *target path* is
the success/failure signal.

**Trigger conditions** (`authenticator.rb:89-139`):

```ruby
if auth_provider == :web
  result = with_retry { WebLogin.auth_with_timeout(...) }   # forced, skips EAccess entirely
  return result
end

fallback_available = web_fallback_supported?(character:, game_code:, legacy:, generator:)

begin
  result = authenticate_via_eaccess(..., fast_fail_unreachable: fallback_available)
rescue FatalAuthError
  raise                       # credentials rejected -> do NOT fall back
rescue StandardError => e
  raise unless fallback_available
  Lich.log "warn: EAccess authentication unavailable (#{e.class}: #{e.message}); falling back to web login"
  result = with_retry { WebLogin.auth_with_timeout(...) }
end
```

The rationale is stated at authenticator.rb:66-72: *"EAccess being unreachable is a reason to try a
different transport; EAccess correctly rejecting a bad password is not — falling back in that case
would just resubmit the same bad credentials to a second system for no benefit."*

```ruby
# authenticator.rb:191
def self.web_fallback_supported?(character:, game_code:, legacy:, generator:)
  !!(character && game_code && !legacy && !generator)
end
```

So **legacy enumeration and generator entry have no fallback at all** — and consequently get the
full retry budget, because there is nothing to fail over to.

**[verifier-corrected timing]** With `fast_fail_unreachable: true` (the normal single-character
case), the unreachable break fires after **one** attempt (authenticator.rb:245), so the fallback
typically happens after a **single** EAccess try — not after three. The doc comment at
authenticator.rb:26-30 implies "once EAccess's own retries are exhausted"; that is wrong for the
common case.

### 5.4 The web-login protocol

Enough detail to implement; the file is the reference for the rest.

**The User-Agent is mandatory and exact.** play.net's CloudFront/WAF returns a bare HTTP 500 for any
request without a browser-like UA. Ruby's default `Ruby/x.y.z` is rejected outright, and so is
`reqwest`'s default.

```
Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36
```
web_login.rb:103 **[verifier-corrected line: :103, not :71]**; `docs/web-login-protocol-analysis.md:29-32`.

**A session cookie must be obtained first.** `GET /{family}/signin_needed.asp` before anything else
— POSTing `login.asp` cold returns a bare 500. This is invisible in a browser capture because the
browser already has the cookie. web_login.rb:284 **[verifier-corrected: :284, not :252]**.

**Sequence:**

1. `GET /{family}/signin_needed.asp` — acquire ASP session cookie.
2. `POST` login form (body at web_login.rb:286-297). **The password is sent in the clear as a form
   field over HTTPS — no XOR, unlike EAccess.**
3. Inspect the redirect `Location` **without following it** (guards at web_login.rb:300-304).
   Success vs `LOGIN_FAILED` is determined **purely by the redirect target path.**
4. Possible security-question interstitial: `SECURITY_QA_PATH`, web_login.rb:255.
5. Possible subscription interstitial — **the pattern is per-instance, not one fixed path**
   (web_login.rb:345):
   `%r{\A/#{Regexp.escape(instance[:family])}/play/subscription(?:_to_\w+)?_needed\.asp\z}`.
   Confirmed live variants: DR's plain `subscription_needed.asp` vs DRX/DRF's
   `subscription_to_plat_needed.asp` / `subscription_to_fall_needed.asp`.
6. Scrape the character list for `charID`. **This regex is the single most fragile thing in the
   entire auth surface** (web_login.rb:359):
   ```ruby
   /id="(W_[A-Za-z0-9_]+)"[^>]*>\s*<label for="\1"><span[^>]*>([^<]+)<\/span>/
   ```
   with a **case-insensitive** name compare (`name.strip.casecmp?(character)`). A markup change
   breaks it **silently** — no match, then a misleading `CHARACTER_NOT_FOUND`, which is in
   `FATAL_ERROR_CODES` and therefore **fatal, non-retried, and non-fallback**. The worst possible
   classification for a transient-looking breakage.
7. `POST /includes/common/play/goplay2.asp` (web_login.rb:376-392) with:
   `charID`, `NEWCHARSUB: "TRUE"`, `managesub: 0`, `gameName: instance[:family]`, `instanceID: 0`,
   `game: instance[:web_game_code]`, `frontend: "web"`. (The source notes `NEWCHARSUB` was absent
   from a GemStone Test capture but is sent unconditionally anyway.)
8. Follow redirects manually (`follow_redirects`, web_login.rb:415-437): loop `MAX_REDIRECTS` times;
   any absolute `Location` matching `%r{\Ahttps?://}i` — **explicitly including plain `http://`** —
   is validated and **returned without being fetched**. The comment explains why: a plain `http://`
   Location handed to `get` would be requested literally as a path on the existing HTTPS connection
   rather than recognized as a downgrade attempt. Past the cap → `TOO_MANY_REDIRECTS`.
   **The final URL is deliberately never fetched.**
9. `extract_connection_info` (web_login.rb:516-543) — the web path's entire security boundary, and
   worth porting in full:
   - `DUPLICATE_QUERY_PARAM` if `host`, `port` or `key` appears more than once (query-param
     smuggling defense — implemented via `URI.decode_www_form(uri.query).group_by(&:first)`).
   - `NO_CONNECTION_INFO` if any is blank.
   - `UNEXPECTED_CONNECTION_INFO` if a pinned instance's returned host/port don't match
     `CONFIRMED_INSTANCES` exactly.
   - `UNTRUSTED_CONNECTION_HOST` if an unpinned instance's host fails `trusted_game_host?`.

**The web layer's game codes are NOT the EAccess codes.** Confirmed live mismatch: GemStone Prime is
`GS3` over EAccess but **`GS4` on the web**. Table at web_login.rb:144-150
**[verifier-corrected: :144-150, not :111-119, which is the comment above it]**.
**[verifier: DRT and GST are on entirely different hosts]** — `DRT` is
`hydra.simutronics.com:11624` and `GST` is `chimera.simutronics.com:10624`, **not play.net hosts at
all** (web_login.rb:145, :149). `DRX`/`DRF` remain **unverified guesses** with
`expected_host: nil, expected_port: nil` — the source comment says nobody has an account with those
entitlements. Cena inherits that hole.

**The cookie jar is deliberately non-RFC-6265** (web_login.rb:69+): it keys by cookie **name only**
and drops Path/Domain/Secure/HttpOnly/Expires, on the rationale that it only ever replays to one
host. The chain crosses `/dr/...`, `/includes/common/login/...`, `/includes/common/play/...` and
`/playdotnet/account/...`, so **a correct RFC-6265 jar may withhold a cookie Lich sends.** See §9.4.

### 5.5 Path D — externally-supplied `.sal` launch file (not an auth path, but the real escape hatch)

`main.rb:213-223` reads a file supplied by `--sal=` containing `GAMEHOST=`/`GAMEPORT=`/`KEY=`/
`GAMECODE=`/`GAME=` lines, bypassing authentication entirely. The KEY is already inside. This is
exactly what a maintainer used to get online during the 2026-09-08 outage — the play.net website
issued a one-time ticket and the SAL path consumed it.

**It originates outside Lich and Cena cannot generate one.** But it is worth knowing about as a
manual recovery route, and §5.2's field report shows it works when both SGE paths are down.

**[verifier note]** The `.sal` read at main.rb:213 is a **separate `if`, not an `elsif`**, and
main.rb:215 unconditionally reassigns `@launch_data` — so a `.sal` file **silently overrides a
completed login**. If Cena ever accepts a launch file alongside a saved entry, that precedence is a
decision to make deliberately, not inherit.

---

## 6. Credential storage

### 6.1 What Lich does

**Location — not platform-specific.** `LICH_DIR = File.dirname(File.expand_path($PROGRAM_NAME))`,
`DATA_DIR = File.join(LICH_DIR, "data")` (constants.rb:1, :3 — note :2 is `TEMP_DIR`, where the
`.sal` files holding the eaccess key land). The credential file is
**`<lich install dir>/data/entry.yaml`** on every platform: no `%APPDATA%`, no `~/.config`, no
`~/Library`. Lich is a portable-directory app; the password file sits next to the executable.
Overridable via `--data=` / `--data-dir=`, parsed at lich.rbw:23 with
`/^--(?:data|data-dir)=(.+)[\\\/]?$/i` (case-insensitive, tolerates a trailing slash) *before*
`constants.rb` loads, so the `||=` guard takes the CLI value.

**Files written:**

| Path | Content | Perms | Shredded? |
|---|---|---|---|
| `data/entry.yaml` | accounts, characters, passwords, favorites | `0o600` | never |
| `data/entry.yaml.bak` | full copy of previous version | inherits from `FileUtils.cp` | **never** |
| `data/entry.dat` | legacy Marshal format | default | never |
| `temp/lich<0-9999>.sal` | launch data incl. **eaccess key** | default | yes (`at_exit` + both exits) |

**[verifier-corrected]** `.bak` is created on every save **where the file already exists**
(`if File.exist?(yaml_file)`, entry_store.rb:119-123; same guard at utilities.rb:117, :125) — not
literally every save. The security conclusion stands and is the important part: **a cleartext-era
`.bak` survives a later conversion to encrypted, and is never shredded.** `shred_file` is only ever
applied to `.sal` files (main.rb:488, 529, 542).

**Three encryption modes.** Only the `password` scalar is ever encrypted. **Account names, character
names, game codes, frontends, custom_launch commands and all favorites metadata are always cleartext
in every mode.**

```ruby
# password_cipher.rb:26-32
CIPHER_ALGORITHM = 'AES-256-CBC'
KEY_ITERATIONS   = 10_000
KEY_LENGTH       = 32
```

**Wire format:** `base64_strict( IV[16] || AES-256-CBC-PKCS7(plaintext_utf8) )`.
No newlines, **no version byte, no salt in the blob, no MAC**. Unauthenticated CBC.
password_cipher.rb:42-61 (encrypt), :72-97 (decrypt). `iv_len` for AES-256-CBC is 16, verified.

**Key derivation** (password_cipher.rb:126-147):

```ruby
passphrase = case mode
             when :standard then account_name.upcase
             when :enhanced then master_password
             end
salt = "lich5-password-encryption-#{mode}"     # interpolated from the mode symbol
OpenSSL::PKCS5.pbkdf2_hmac(passphrase, salt, 10_000, 32, OpenSSL::Digest.new('SHA256'))
```

Salt literals, byte-exact: `"lich5-password-encryption-standard"` and
`"lich5-password-encryption-enhanced"` (34 bytes each). **The mode symbol name is the wire format.**

**Stated plainly: `:standard` mode is obfuscation, not encryption.** The passphrase is the account
name, stored in cleartext as the YAML key three lines above the ciphertext. The salt is a
compile-time constant in open-source code. This was verified empirically — recovering a password
knowing *only* what `entry.yaml` itself contains — and independently reproduced by the verifier:

```
PBKDF2("TESTUSER", "lich5-password-encryption-standard", 10000, 32, SHA256) =
  e152625045e564581e49189064f3983474d7ad01733bce84a9a1966e66fea08d
ciphertext (synthetic): BqjMrOzMFTvwMBJndMN5totRl6XFP06rj7LOtufLkH4=
recovered with only the account name: hunter2
```

This is a **design fact, not a criticism** — the threat model `:standard` actually addresses is
real: shoulder-surfing, screenshots, config files pasted into Discord, casual grep of the data
directory. The GUI is honest about it, labelling it *"Standard Encryption (Account Name)"*
(encryption_mode_change.rb:117-118). But the fixed salt means the derived key is **identical across
every Lich install for a given account name**, so one precomputed table breaks all `:standard` files
everywhere; and because the salt is per-*mode* not per-*account*, two accounts sharing a master
password in `:enhanced` mode derive the **identical AES key**.

**`:enhanced` mode** is genuine encryption, keyed by a master password held in the OS keychain
(`KEYCHAIN_SERVICE = 'lich5.master_password'`, master_password_manager.rb:18). Its validation test
(master_password_manager.rb:69-85) uses **100,000** iterations with salt =
`'lich5-master-password-validation-v1'` (35 bytes) **concatenated as BYTES** with
`SecureRandom.random_bytes(16)` → 51 bytes total; only the 16 random bytes are base64'd into
`validation_salt`. Then `SHA256.digest` of the derived key.

Note the asymmetry the source itself flags (master_password_manager.rb:15-16): the **validation**
test uses 100k iterations, the **actual key derivation** uses 10k with a fixed salt. The stronger
KDF guards the "is this the right password" check; the weaker one guards the passwords. That is
backwards, and deliberate — 100k × N accounts per login was too slow.

**Name normalization is a wire-format contract, not cosmetics.** `entry_store.rb:1010-1013` and
`:1021-1024` **[verifier-corrected line numbers, ~3 low in earlier writeups]**:

```ruby
def self.normalize_account_name(name)
  return '' if name.nil?          # <- present in source; omitted from earlier quotes
  name.to_s.strip.upcase
end
def self.normalize_character_name(name)
  return '' if name.nil?
  name.to_s.strip.capitalize
end
```

Account UPCASE is **load-bearing** — `:standard` derives its key from it.
`String#capitalize` **downcases the tail**: `"McDonald"` → `"Mcdonald"`.

**[verifier-corrected, and this inverts an earlier claim]** There are **two disagreeing normalizers
in the tree.** `cli_password.rb:544-546` is `name.to_s.strip.split.map(&:capitalize).join(' ')` — a
**per-word** capitalize, not the single-token `String#capitalize` that `entry_store` uses. For
`"Bob Smith"`, entry_store yields `"Bob smith"` and cli_password yields `"Bob Smith"`. **They
disagree on every multi-word character name**, and character name is part of the 5-tuple favorites
identity. Earlier writeups cited :546 as *confirming* the capitalize quirk; it actually documents a
conflict. **Unresolved which is authoritative** — see §11.5.

**Other storage facts worth knowing:**

- `decrypt_password` (entry_store.rb:241) accepts the mode as **either Symbol or String** throughout
  (`mode.to_sym` at :244, :249, :272). YAML stores a String; callers pass Symbols. A Rust enum needs
  a case-tolerant deserializer for `plaintext` / `standard` / `enhanced`, and must accept a leading
  `:symbol` form since both writers use `YAML.dump(..., permitted_classes: [Symbol])`.
- **Header/payload divergence bug.** `save_entries` (:79-93) calls `convert_legacy_to_yaml_format`
  first, which stamps the header from `entry_data.first[:encryption_mode]`; then :97-117 encrypts
  using `original_encryption_mode` read from the *file*. The header is written first and never
  corrected. A mixed-mode array silently takes element zero's mode. **Derive both from one source.**
- Two write paths disagree: `write_yaml_file` (:1049-1057) applies `0o600`;
  `generate_yaml_content` (:1032-1040) is a near-duplicate that **does not**, so any caller pairing
  it with a plain `File.write` gets default permissions.
- `entry.dat` is Base64 (`pack('m')` — RFC 2045 **with newlines**, not `strict_encode64`) wrapping a
  **Ruby Marshal** dump (state.rb:21). **[verifier note] there IS a legacy write path** —
  state.rb:48 contains `file.write([Marshal.dump(entry_data)].pack('m'))`. Whether it is still
  reachable is unverified; a stale writer could re-emit cleartext credentials after a user converted
  to encrypted. **Rust cannot and should not parse Marshal** (and `Marshal.load` on untrusted data
  is RCE). Treat as out of scope, or shell out to Ruby once for a one-time migration.
- **Zero terminal echo suppression anywhere in `lib/`.** No `io/console`, no `IO#noecho`, no `stty`.
  Every CLI password prompt is bare `print` + `$stdin.gets`. Master passwords are typed in cleartext
  and land in scrollback. The specs pin this by stubbing `$stdin.gets` directly.
- All CLI prompts call `.strip`; **macOS and Linux keychain *retrieval* also `.strip`**
  (master_password_manager.rb:147, :176), while the Windows FFI read at :159 does not. So Lich is
  already inconsistent across platforms for a master password with edge whitespace.
- Retrieval treats **empty string as absent** on mac/Linux (`output.empty? ? nil : output`), so a
  zero-length master password loops the recovery prompt forever. **Reject empty at entry.**
- macOS keychain *writes* go through `system("security add-generic-password ... -w #{escaped}")`
  (master_password_manager.rb:139-143) with only `.shellescape` — **the password transits a shell
  command line and is visible in `ps` to any local process during the write.** Linux uses an
  `IO.popen` pipe and does not have this. Argues strongly for Rust's `keyring` crate (direct API).
- `store_master_password` failure during recovery is **non-fatal** (:291-294): logs a warning,
  continues with the in-memory password. A machine with a locked keychain re-prompts every launch
  but still works. **Replicate that — fail-closed here bricks the client.**
- Windows Credential Manager blob is **UTF-8 bytes, not UTF-16** — only `target_name` and `user_name`
  are UTF-16LE (windows_credential_manager.rb:98-99, read symmetric at :159).
- Favorites identity is a **5-tuple** (username, char_name, game_code, frontend, custom_launch) with
  a `:__unset` sentinel meaning "legacy match, ignore" (entry_store.rb:495, favorites_spec.rb:212).

### 6.2 Recommendation for Cena

**Do not port Lich's three-mode model.** It exists because Lich must run on machines where no
keychain is available and must degrade gracefully for a broad user base. Cena is private,
single-user, and already has better machinery.

1. **Reuse VellumFE's existing credential layer.** It already depends on `keyring` v3
   (`Cargo.toml:136`) and `rpassword` (`Cargo.toml:54`), keys the keychain by lowercased account
   (`profiles.rs:414`), and has a **fail-closed ChaCha20-Poly1305 seal module for mobile**
   (`profiles.rs:487-577`). That is the design; port it, not Lich's.
2. **Two modes, not three:** OS keychain on desktop (Windows Credential Manager / macOS Keychain /
   Secret Service), and the ChaCha20-Poly1305 sealed store on mobile and on headless Linux where no
   Secret Service is running. **No obfuscation tier.** If a secret cannot be protected, prompt.
3. **AEAD everywhere. Never unauthenticated CBC.** Lich's CBC means a wrong key usually raises a
   padding error but **~1/256 of the time yields garbage plaintext instead** — a real, reachable
   state a Rust port must handle. ChaCha20-Poly1305 makes it unrepresentable.
4. **Per-secret random salt, stored with the ciphertext.** No fixed salts, ever.
5. **Do NOT reuse the keychain service string `lich5.master_password`.** Lich stores a *master
   password* there; Cena would store *account passwords*. Colliding corrupts a working Lich install
   on the same machine. Use a distinct service string.
6. **Never accept a password on the command line.** `credential_scrub.rb:34` defines
   `ARGV_SECRET = /\A(--(?:password|master-password)=)(.+)\z/i` — **two flags, case-insensitive**.
   A port that blocks only `--password=` leaves `--master-password=` and `--PASSWORD=` open.
7. **Skip ~80% of `credential_scrub.rb`.** Its ARGV/ivar scrubbing exists because Lich runs
   untrusted Ruby in-process. Cena has no embedded scripting. What survives: the argv rule above,
   and redacting credentials in logs.
8. **Never write the key to a temp file.** Cena is its own frontend; the entire `.sal` mechanism
   (§7.4) disappears, and with it the shred-file hazard.
9. **Use `secrecy::Secret<String>` + `zeroize`.** Lich's `String#replace`-in-place trick has no Rust
   analogue and needs none.
10. **On file permissions:** `File.open(path, 'w', 0o600)` is effectively inert on Windows (maps to
    the read-only attribute, not an ACL), and Rust's `std::fs::Permissions` has the same limitation.
    Best avoided by not writing secrets to disk on desktop at all.

---

## 7. The flow, start to live game socket

### 7.1 The numbered sequence (Cena's version — Lich's detours noted, deleted in §8)

```
 1. Resolve the login target: account, password, character, game_code, frontend.
 2. Load + decrypt the stored password (§6.2).
 3. Set per-session identity state (Cena: per-session struct; Lich: process globals).
 4. AUTH:
    4a. Try EAccess (§2-§4).  fast_fail_unreachable = web_fallback_supported?
    4b. On FatalAuthError -> stop. Do not fall back.
    4c. On any other error, if fallback available -> WebLogin (§5.4), with retry.
 5. Receive LaunchResult { key, gamehost?, gameport?, gamecode, ... }.
 6. TRIM the trailing newline from key.                       <- §4.7 item 5
 7. Rewrite gamehost/gameport via fix_game_host_port.          <- §7.2, MANDATORY
 8. TCP connect to (gamehost, gameport), bounded timeout.
    8a. On connect failure, apply break_game_host_port (the exact inverse) and retry once.
 9. Configure the socket (§7.3).
10. HANDSHAKE (§7.4), exactly:
       send  <KEY>\n
       send  /FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML\n
       sleep 300ms ; send <c>\n
       sleep 300ms ; send <c>\n
11. Record login time. The session is live; hand the socket to the parser.
```

### 7.2 Host/port rewriting — mandatory, bidirectional

EAccess still returns **legacy hostnames**; Lich translates them before connecting.
`Lich.fix_game_host_port` (lich.rb:737-752):

| EAccess returns | Connect to |
|---|---|
| `gs-plat.simutronics.net:10121` | `storm.gs4.game.play.net:10124` |
| `gs3.simutronics.net:4900` | `storm.gs4.game.play.net:10024` |
| `gs4.simutronics.net:10321` | `storm.gs4.game.play.net:10324` |
| `prime.dr.game.play.net:4901` | `dr.simutronics.net:11024` |

**[verifier-corrected]** The DragonRealms guard at lich.rb:749 matches the full
`prime.dr.game.play.net`, not `prime.dr`; the GemStone entries need their full
`*.simutronics.net` hostnames as written above.

`break_game_host_port` (lich.rb:754-768) is the **exact inverse table**. Lich tries the fixed pair
first, and **on connect failure reverses the mapping and retries once** (main.rb:544-562).

**Cena MUST implement both directions**, or logins to Platinum/Prime fail on one of the two host
spellings. This table is also the clearest evidence that **Simutronics has changed endpoints under
Lich before** — treat it as data that will need updating, not as a constant.

`$platinum = true` iff `gameport == '10121' || gameport == '10124'` (main.rb:382-387)
**[verifier-corrected line: :382-387, not :361-365]**.

### 7.3 Socket configuration

`SocketConfigurator`, games.rb:456-478: `SO_KEEPALIVE` (idle 30, interval 30), `SO_LINGER`
(enabled, timeout 5), `SO_RCVTIMEO`/`SO_SNDTIMEO` 30, `SO_RCVBUF`/`SO_SNDBUF` 32768,
`TCP_NODELAY` true, and **Windows-only `TCP_MAXRT` 10**.

**Failures are swallowed** — `rescue StandardError` at games.rb:480-484 logs and continues with OS
defaults. **A Rust implementation that returns `Result` from socket configuration and propagates it
will fail logins that Lich completes.** Match the swallow deliberately.

`@socket.sync = true` (games.rb:480) disables Ruby's userspace write buffering. Rust's `TcpStream`
is unbuffered by default so this is a no-op to port — **but if Cena wraps the socket in a
`BufWriter`, the key/version/`<c>` handshake sits in the buffer and the login hangs.** Real trap.

What actually matters at runtime is not `SO_RCVTIMEO` (meaningless on an async socket) but Lich's
read-loop policy: `READ_TIMEOUT_SECONDS = 100`, `MAX_CONSECUTIVE_READ_TIMEOUTS = 3`
(games.rb:374-380).

### 7.4 The game-socket handshake — exact

Three sends, in this order (main.rb:699-718 **[verifier-corrected: the block starts at :699, not
:697]**):

```ruby
Game._puts(game_key)      # main.rb:702   the raw KEY, nothing added but the socket's newline
Game._puts(client_string) # main.rb:707   /FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML
2.times {                 # main.rb:714-717
  sleep 0.3
  $_CLIENTBUFFER_.push("<c>\r\n")
  Game._puts("<c>")
}
$login_time = Time.now    # main.rb:717
```

`CLIENT_STRING` is at **`lib/common/frontend.rb:515`** **[verifier-corrected file path: it is
`lib/common/frontend.rb`, not `lib/frontend.rb`, which does not exist in this tree]**:

```ruby
CLIENT_STRING = "/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML"
```

**The constant has NO trailing newline.** The `\n` comes from `Game._puts` → `@socket.puts`
(games.rb:650, :668).

**Three byte-level corrections, all of which a naive port gets wrong:**

1. **LF, not CRLF.** `Game._puts` → `@socket.puts` emits `\n`. The `"<c>\r\n"` visible at
   main.rb:715 is pushed into `$_CLIENTBUFFER_` — **a local client-echo buffer, not the wire.** A
   Rust `write!(sock, "{}\r\n", ...)` here is a protocol bug.
2. **The sleep comes BEFORE each `<c>`, not between them.** The real sequence is
   **sleep 300ms → send → sleep 300ms → send.** There is a 300ms delay before the **first** `<c>`.
   The natural transcription of "twice, 300ms apart" is send-sleep-send, which puts the first `<c>`
   300ms too early and diverges from Lich.
3. **Ruby's `IO#puts` will not double-terminate.** If the KEY ever carries a trailing newline,
   Ruby sends exactly one LF; a naive Rust `format!("{}\n", key)` sends two. Trim first (§4.7).

Getting the order or the version string wrong means **the game server never enables XML mode** —
and everything downstream in Cena depends on the XML stream.

> **Saga disagrees with Lich on this sequence in two ways (2026-09-18, §4.7a).** Simutronics'
> own client branches on whether the game code begins with `DR`, and on the **GemStone** branch:
>
> - it sends **no `<c>` at all.** The two `<c>` above are Lich's DragonRealms-capable path
>   leaking into the GemStone one. `plan/15` §1.2 records the same finding, and
>   `spike/eaccess-spike` was corrected to stop sending them.
> - it sends a **third write: a bare line terminator on its own**, after the banner. This spec
>   does not otherwise mention it. **UNVERIFIED** whether it is required or merely harmless;
>   send it, since a first-party client does so unconditionally.
>
> Saga also uses **CRLF** on this branch where Lich uses LF, which does **not** overturn
> correction 1 above: Lich's LF demonstrably works, so both terminators are evidently accepted,
> and correction 1's real point — that the `
` at main.rb:715 is a **client-echo buffer,
> not the wire** — is untouched.

`Frontend.send_handshake` additionally sends three GSL commands (frontend.rb:592-594), each prefixed
with the runtime `$cmd_prefix` global, **not bare**: `#{$cmd_prefix}_injury 2`,
`#{$cmd_prefix}_flag Display Inventory Boxes 1`, `#{$cmd_prefix}_flag Display Dialog Boxes 0`.

**[verifier note]** A fourth handshake path exists that a binary "headless does it / frontend does
it" model misses: when `launcher_cmd =~ /mudlet/`, Lich performs the **exact headless handshake**
even though a frontend *is* launched (main.rb:781-791).

### 7.5 Data objects

**`LaunchData` is NOT an object.** It is `Array<String>` of `"KEY=VALUE"` lines
(launch_data.rb:22: `auth_data.map { |k, v| "#{k.upcase}=#{v}" }`), and every downstream consumer
re-parses them with regexes. **Cena keeps a struct and never serializes to this line format.**

`LaunchData.prepare` then strips any pre-existing `FRONTEND|CUSTOMLAUNCH|CUSTOMLAUNCHDIR|
CUSTOMLAUNCHARGV` lines, appends `FRONTEND=<id>`, and rewrites fields per frontend
(launch_data.rb:44-60): `wizard` → `GAMEFILE=WIZARD.EXE, GAME=WIZ, FULLGAMENAME=Wizard Front End`;
`avalon` → `GAME=AVALON`; `saga` → `GAME=SAGA`; `suks` → `GAMEFILE=WIZARD.EXE, GAME=SUKS`;
`stormfront` unchanged. **[verifier: a sixth, data-driven branch the table misses]** —
launch_data.rb:35-38: if no custom_launch was given and the frontend's catalog metadata has
`launcher_adapter == :custom`, Lich **synthesizes** a custom_launch from
`FrontendLauncher.command(frontend_id)` plus a launch directory.

**Every one of these rewrites is frontend-launch accident. Cena deletes all of it.**

---

## 8. What Cena deletes

Roughly 60% of Lich's login flow is proxy plumbing, not authentication. Cena owns both sides of the
connection, so all of the following disappears:

| Deleted | Why it existed | Source |
|---|---|---|
| **The local TCP listener** (`TCPServer.new(bind_address \|\| '127.0.0.1', nil)`) | The frontend connects to Lich, not the game | main.rb:419 |
| **The temp `.sal` file** (`TEMP_DIR/lich<rand(10000)>.sal`) | Telling an external frontend where to connect | main.rb:479-484 |
| **GAMEHOST/GAMEPORT rewriting to localhost** inside that file | Pointing the frontend at Lich's listener | main.rb:478 |
| **`%1` / `%host%` / `%port%` / `%key%` substitution into a launcher command line** | Credentials on a command line | main.rb:490-493; frontend_launcher.rb:128-147 |
| **`CredentialScrub.shred_file` + the `at_exit` backstop** | Only needed because the key hit disk | main.rb:488 |
| **The 30s wait for the frontend to connect** (`300.times { sleep 0.1; break unless accept_thread.status }`) | Waiting on an external process | main.rb:519-535 |
| **`Frontend.create_session_file`** (writes `<Name>.session` JSON; note `name.downcase.capitalize`) | Frontend discovery | frontend.rb:600-610 |
| **All per-frontend `LaunchData` field rewrites** (WIZARD.EXE, GAME=WIZ, …) | Telling a Simutronics launcher which `.EXE` to run | launch_data.rb:44-60 |
| **`GAME` / `GAMEFILE` / `FULLGAMENAME` / `UPPORT` fields** | Same | — |
| **`SGE.sal` special-case shell-out + `exit`** | Windows installer integration | argv_options.rb:203-239 |
| **`SagaManagedLogin` pre-emption** | Third-party auth owner | main.rb:144-168 |
| **Process-per-character spawning** (`SessionLauncher.spawn_process`) | Lich is structurally one session per process | session_launcher.rb:57-66 |
| **`$_CLIENT_` / `$_CLIENTBUFFER_` echo plumbing** | Relaying to an external client | — |
| **~80% of `credential_scrub.rb`** | Untrusted in-process Ruby scripts | credential_scrub.rb |

**The multi-session point deserves emphasis.** Lich is **one OS process per character** — no shared
auth state, no connection pooling, no rate limiting. `Lich::Common::Account` stores
name/character/game_code/subscription/members in Ruby **class variables** (`@@name` etc.), which are
process-global. Worse, the GUI's persistent-launcher mode **throws away a successful
authentication**: `handle_play_action` (gui_login.rb:645-659) receives fully-authenticated
`launch_data`, passes it to `SessionLauncher.launch`, which extracts only a few identity fields from
it in `build_spawn_args` (session_launcher.rb:88-118 — **[verifier-corrected]** including
`launch_map['NAME']` as a character fallback at :92, plus `optional_spawn_flags`), and the child
**re-runs the entire EAccess exchange from scratch. The KEY is discarded.**

**That is a Lich process-model artifact, not a protocol requirement.** Cena, as one process with 25+
sessions, authenticates once per session and connects directly — and must make every piece of that
class-variable identity state **per-session**.

**What Cena keeps from `--without-frontend`.** That is the mode to port: Lich itself sends the
key/version/`<c><c>` handshake on a background thread (main.rb:699-718), with no listener and no
external client. Every other branch hands the handshake to the frontend by pumping `$_CLIENT_.gets`
into `Game._puts`. **[verifier note]** An earlier claim that main.rb:396 sets `$_CLIENT_ = nil` could
not be confirmed at that line (:392 is `elsif @argv_options[:pipe]`); the substance is right — the
headless branch runs with no listener and no `$_CLIENT_` — but **that specific citation is
unverified** and should be re-derived if it matters.

---

## 9. Failure modes and history

### 9.1 Error classification — the contract

```ruby
# authenticator.rb:29
FATAL_ERROR_CODES = %w[REJECT NORECORD INVALID PASSWORD CHARACTER_NOT_FOUND
                       GENERATOR_NOT_AVAILABLE LOGIN_FAILED NO_SUBSCRIPTION]
```

**Classification is by error-code STRING, never by exception class** (authenticator.rb:229), and it
applies uniformly across both providers. Matching is `.include?` — **substring, not equality**. So a
server response of `PASSWORD_EXPIRED` is classified fatal by substring match on `PASSWORD`.
**Cena must decide deliberately whether to preserve substring semantics**; a strict Rust `match` on
an enum behaves differently.

Client-synthesized codes: `MALFORMED_K_RESPONSE`, `CHARACTER_NOT_FOUND`, `GENERATOR_NOT_AVAILABLE`.
`AuthenticationError`'s message format is exactly `"Error(<code>)"`.

```ruby
# authenticator.rb:41-44, :57-61
UNREACHABLE_ERROR_CLASSES = [SocketError, Errno::ECONNREFUSED, Errno::ECONNRESET,
                             Errno::ETIMEDOUT, Errno::EHOSTUNREACH, Errno::ENETUNREACH,
                             OpenSSL::SSL::SSLError]
# plus: a RuntimeError whose message starts with 'error: timed out authenticating'
```

**Retry policy** (`with_retry`, authenticator.rb:206-266): `MAX_AUTH_RETRIES = 3`,
`AUTH_RETRY_BASE_DELAY = 5`, delay `5 * 2**attempt` → **5s then 10s**, with no sleep after the last
attempt (guarded at :252-258). **[verifier note] the source comment at authenticator.rb:18 says
"5s, 10s, 20s" and is wrong about its own code.** Trust the code.

`fast_fail_unreachable` breaks after **one** attempt — but **only when a fallback exists**. Legacy,
generator, and WebLogin-itself calls get the full retry budget because there is nothing to fail over
to. That conditional retry depth is a real design decision; `authenticator_spec.rb:302-364` asserts
all three cases.

**LIVE LATENT BUG — fix it in Cena.** `FATAL_ERROR_CODES.any? { |code| e.error_code&.include?(code) }`
is a **case-sensitive** `include?` against an uppercase list. Issue #1266's reporter observed
`Error(NoRecord)` in **mixed case**, and `"NoRecord".include?("NORECORD")` is `false`. That response
would be classified **transient** and retried 3× with backoff instead of failing fast.
**Normalize case before matching.**

### 9.2 The failure catalogue

| Date | Incident | Fix | Ref |
|---|---|---|---|
| 2023-02-13 | Ruby 3.2 tightened `Array#=~` → `NoMethodError` on the legacy path's Array return; swallowed by the GTK queue, GUI sat at "working..." forever | `.to_s` | issue #290 |
| 2026-02-15 | `SSL_read: unexpected eof while reading` killed login outright — server load, network instability, session-cleanup delays, brief handshake failures | `with_retry`, 3 attempts, 5s/10s | PR #1205 |
| 2026-03-02 | Bad credentials → 3 retries (~35s) → hang or silent zombie process. Three bugs: `EAccess.auth` *returned* an error string instead of raising; `with_retry` then called `.map` on a String → `NoMethodError`; main thread ran inside `Thread.new` so the process survived silently | `AuthenticationError` with `error_code`; fatal/transient split; `Thread.abort_on_exception = true` + `SystemExit` | PR #1237 |
| 2026-03-28 | GUI hung forever on a wrong password — "Inputting incorrect password hangs on 'working...'. Does not error out" | error surfaced to the GUI | issue #1266 |
| 2026-08-20 | `GS4` present in `VALID_GAME_CODES` but absent from `GAME_MAPPING`, so a validated code persisted `game_name: 'Unknown'` | lists reconciled | issue #1507 |
| 2026-09-01 | **Silent indefinite hang, no log output.** "the TCP connect, TLS handshake, and every K/A/M/F/G/P/C/L round-trip are bare blocking calls with no connect or read timeout anywhere... confirmed via `lsof` showing the socket stuck in `SYN_SENT`" | `auth_with_timeout`, 30s watchdog thread | PR #1502 |
| 2026-09-08 | **The EAccess :7910 outage.** `nc -vz eaccess.play.net 7910` → exit 124 (SYN silently dropped, no RST). 7900 accepted TCP but did not complete the handshake for GSIV, while it did work for DR. Website auth and Play-in-Browser stayed up throughout | HTTPS web-login (#1570 merged); cleartext fallback (#1569) **closed unmerged** | §5.2 |

### 9.3 What this history says about protocol stability

**The SGE wire protocol itself has never changed.** Not one incident in the catalogue is a change to
the K/A/M/F/G/P/C/L exchange or to the password obfuscation. That is genuinely reassuring for the
highest-risk part of the port.

**Everything around it has moved:**

- **Endpoints change.** The `fix_game_host_port` table (§7.2) exists because Simutronics moved game
  hosts and ports under Lich, and the bidirectional retry exists because the change was not
  simultaneous everywhere. `eaccess.play.net` now resolves to AWS.
- **Game codes drift.** Per OSXLich-Doug on issue #1507: *"For Gemstone, specifically, GS3 and GSF
  are the two valid game codes returned by Simu during login. GSX was, at one time, valid, but with
  the retirement of GS Platinum, it is no longer seen in the wild... The one valid place that `GS4`
  actually can be observed is in the game stream itself upon initializing."* Current list
  (login_helpers.rb:16): `VALID_GAME_CODES = %w[GS3 GST GSF DR DRX DRT DRF]`. `GS4` and `GSX` are
  both gone. **Treat game codes as configuration, not constants.**
- **Availability is the real risk, not correctness.** Both 2026 outages were the endpoint being
  unreachable or unresponsive, never the protocol being wrong.
- **The web path is HTML scraping and will break.** Not if — when. §5.4 step 6.

**The durable lesson**, from ondreian on issue #290: *"the larger issue here is that Lich is
swallowing errors that should bubble up to stderr."* Three of the seven incidents above were
**silent hangs**, not crashes. Cena's login must be incapable of hanging quietly: every stage
bounded, every failure logged with its stage name (§4.10), and no error path that ends in a wait.

---

## 10. Rust implementation notes

### 10.1 Crate choices

| Need | Recommendation |
|---|---|
| TLS | **`rustls` with a custom `ServerCertVerifier`** — see 10.2 |
| Root store | `rustls::RootCertStore::empty()` — **do NOT pull in `webpki-roots` or `rustls-platform-verifier`** |
| Async | `tokio`; `tokio::time::timeout` per protocol step |
| HTTP (web path) | `reqwest` with **`redirect::Policy::none()`** — see 10.4 |
| Socket options | `socket2` (keepalive/linger/nodelay/bufsize); `TCP_MAXRT` needs a raw Windows `setsockopt` |
| Keychain | `keyring` v3 (already a VellumFE dep) |
| Password entry | `rpassword` (already a VellumFE dep) — but see 10.7 |
| Secrets in memory | `secrecy` + `zeroize` |
| Mobile sealed store | ChaCha20-Poly1305, per `profiles.rs:487-577` |

### 10.2 TLS — the single largest divergence surface

**Use `rustls` with a custom `ServerCertVerifier`, not `native-tls`.** The requirement — trust
exactly one self-signed cert, no hostname verification, no system roots — maps badly onto
`native-tls`, which delegates to **schannel on Windows**, and Cena is Windows-primary. schannel's
self-signed handling is unpredictable here.

Do **not** expect `RootCertStore::add` to work with this certificate. Ruby's `cert_store.add_file` +
`VERIFY_PEER` makes a plain leaf self-signed cert a trusted **root**; `rustls` enforces stricter
constraints (basicConstraints, keyUsage) and will reject a cert that a lenient OpenSSL store
accepts. **The custom verifier should do exactly what Lich does: compare the presented certificate
to the pinned one and accept on match** — but compare a **SHA-256 of the DER**, not PEM text
(strictly better than Lich's string equality, and not a wire-visible change; it simply will not
interoperate byte-for-byte with a Lich-written `simu.pem`).

Three hard divergences to measure before committing:

1. **SNI.** Ruby sends **none**. `rustls` requires a `ServerName` and **always sends SNI**. If the
   AWS load balancer in front of `eaccess.play.net` routes on SNI, **Cena reaches a different
   backend than Lich does** — maddening to debug. An IP-based `ServerName` can suppress SNI if
   needed. Spike item S1.
2. **TLS version floor.** Ruby permits **TLS 1.0+**. `rustls` is **TLS 1.2+ only**. If this legacy
   endpoint negotiates 1.0/1.1, `rustls` fails where Ruby succeeds. Spike item S2.
3. **Certificate comparison semantics.** §2.3.

### 10.3 Arithmetic and bytes

- **Compute the password hash in `i32`/`u32`, never `u8`.** Rust `u8` wraps in release and panics in
  debug; **neither matches Ruby**, which raises on both overflow and underflow.
- **Recommended policy: replicate Ruby's failure.** Reject the password locally when
  `((pw[i] as i32) - 32) ^ (key[i] as i32) + 32` falls outside `0..=255`, rather than masking.
  **Lich has never successfully sent a byte outside that range, so the server's behavior there is
  completely unobserved.** Wrapping with `& 0xFF` would emit bytes Lich has never produced — that is
  an untested protocol change disguised as a port.
- **Assemble the `A` line as `Vec<u8>`, never `String`** (§3.3, hazard #1).
- **Bound password length against the 32-byte key length explicitly** (§3.3 item 3).

### 10.3a Rust-vs-Ruby hazards the spike actually hit

Four hazards that **cost real debugging time** on 2026-09-18. Each is invisible in Ruby because
Ruby's semantics hide it; each produces a failure that points somewhere other than its cause.

1. **One command = one `write_all` = one TLS record.** Ruby's `IO#puts` is inherently a single
   write, so Lich never had to think about this. Building the command and its `\n` with two
   `write_all` calls can emit **two TLS records**, and this server does not reassemble a command
   split across records. Symptom: authentication fails with a generic server rejection, pointing at
   the credential rather than the framing.
   Port note: VellumFE does this deliberately and says so — `network.rs:920-931`,
   *"Match Ruby's puts … in a SINGLE write … to ensure it goes out as a single TLS record."*

2. **The hashed password must never transit a Rust `String`.** It is arbitrary bytes, not UTF-8.
   `String::from_utf8_lossy` replaces **every byte above 0x7F with U+FFFD**, silently corrupting the
   credential. Ruby Strings are byte arrays, so Lich has no equivalent failure. Build the `A`
   request as `Vec<u8>`.

3. **`set_nodelay(true)`.** Without it Nagle can coalesce or delay the small command writes this
   protocol is built from. VellumFE sets it (`network.rs:847`).

4. **The success guard is `L\tOK\t`, not `^L\t`.** See §4.7 item 2. This one *is* in Lich, with a
   comment explaining it — the hazard is that a careless port drops the `OK`.

5. **Read the response before parsing it, and check it answers the command you sent.** Not a
   language hazard — a *discipline* hazard, and the costliest of the session by a wide margin.
   Ruby's `EAccess.read` returns a String that a human reads in a debugger; a Rust port that pipes
   it straight into `split('\t').nth(2)` renders a rejected command as an ordinary-looking field
   value. See §4.4a. **One `starts_with` per response** would have turned a multi-hour
   misdiagnosis into a one-line error.

### 10.4 HTTP behavior differences (web path)

- **`reqwest` follows redirects by default (up to 10). `Net::HTTP.start` does not, and Lich depends
  on that.** Set `redirect::Policy::none()` or the whole flow breaks: the final
  `/play/home.asp?host=...&port=...&key=...` gets fetched, possibly **consuming the one-time key**,
  and the success/failure discrimination — which is *purely* the redirect target path — becomes
  impossible.
- **Reimplement Lich's naive cookie jar; do not enable `reqwest`'s correct one.** `cookie_store`
  implements real RFC-6265 Path scoping and **will withhold a cookie Lich sends**, because the chain
  crosses four unrelated path prefixes.
- **Use `query_pairs()`, not `.get("key")`.** The duplicate-parameter check (§5.4 step 9) depends on
  seeing *all* pairs; convenience APIs return only the first match and silently defeat a deliberate
  security check.
- **Set the exact User-Agent on every request, including the first GET** (§5.4).
- `URI.encode_www_form` percent-encodes with `+` for spaces; `reqwest`'s `.form()` matches.

### 10.5 I/O and timeouts

- **`Socket.tcp(host, port, connect_timeout:)` resolves and connects in one call**, raising
  `SocketError` for DNS failure. Rust's `TcpStream::connect_timeout` takes an **already-resolved**
  `SocketAddr`, so resolution and its error mapping must be handled **separately** to preserve the
  diagnostic distinction between DNS failure, `ETIMEDOUT` and `ECONNREFUSED` that the stage taxonomy
  (§4.10) depends on.
- **`sysread(8192)` maps to `Read::read`, not `read_exact`** — the response length is unknown.
  But Cena should replace it with a **read-until-`\n` framed reader** regardless (§3.2). That is a
  deliberate divergence from Lich, not a port, and it needs S4 to know the real terminators.
- **Per-operation deadlines change observable timing.** Lich fails connect in 5s and everything else
  in 30s. If Cena bounds each read at, say, 5s, it fails faster than Lich on a slow-but-alive
  backend and may report an outage where Lich would have succeeded. Pick the budgets deliberately.
- **The watchdog does not port.** Lich uses `Thread.new{}.join(timeout)` + `Thread#kill`, relying on
  three behaviors Rust does not share: `join(n)` returns `nil` on timeout **and** re-raises the
  thread's exception on failure; `Thread#kill` still runs `ensure` blocks (which is how the stuck
  socket gets closed, eaccess.rb:330-332); and `Thread.current[:eaccess_stage]` survives the kill.
  Also, **`Thread#kill` does not actually abort a blocked `TCPSocket.open` in MRI** — the socket may
  still be connecting. Use `tokio::time::timeout`, which returns
  `Result<Result<T, E>, Elapsed>` and makes the three-way outcome explicit — **do not `.unwrap()`
  that into a two-way one**, because `break_game_host_port` must fire on **both** timeout and error.
  Carry the stage as a plain `Stage` enum in the state machine; that is simpler and more reliable
  than thread-locals.
- **`sleep` in the retry loop must be `tokio::time::sleep`.** With 25 sessions, a blocking sleep
  stalls the runtime — exactly the GVL-shaped workaround this project already knows to avoid.
- **Rust's `std::process::Command` does not auto-reap.** Lich uses `Process.detach`. Cena spawns no
  children, but if any survive, zombie-reaping must be explicit.

### 10.6 Parsing

- **`splitn(2, '=')`** for the `L`-response key/value split (§4.7 item 4).
- **Model the return shape as an enum**, not one function returning Hash-or-Array (§4.8).
- `String#capitalize` downcases the tail and has no direct Rust equivalent; `to_uppercase` /
  `to_lowercase` are Unicode-aware where Ruby's ASCII-ish behavior may differ (Turkish dotless i,
  German eszett). Matters for comparing stored character names.
- Ruby's `casecmp?` vs Rust's `eq_ignore_ascii_case` — ASCII-only in Rust. Low risk for these games'
  character names, but §4.6 already requires picking one case policy deliberately.
- **Ruby's `sort_by` is NOT stable; Rust's `sort_by` is.** `get_favorites` sorts by
  `favorite_order || 999`, so multiple nils collide nondeterministically in Lich. A direct port
  produces different — better, but different — ordering.

### 10.7 Credential-layer differences

- `OpenSSL::Cipher#random_iv` both **generates and sets** the IV as a side effect. Rust crates
  require explicit generation and passing; missing the side effect yields a silently wrong
  implementation that still round-trips against itself.
- `OpenSSL::Cipher` defaults to **PKCS7** padding with no explicit call; Rust's `cbc` crate requires
  choosing it explicitly. (Moot if Cena uses AEAD, as recommended.)
- `OpenSSL::PKCS5.pbkdf2_hmac` takes the salt as **raw bytes**. The validation test concatenates a
  35-byte ASCII prefix with 16 random **bytes** — in Rust that is `Vec<u8>` concat, **not** string
  concat. Encoding it as UTF-8 text corrupts the salt.
- **`rpassword` strips only the trailing newline; Lich's prompts call `.strip` (both ends, all
  whitespace) — and so does mac/Linux keychain *retrieval*.** For compatibility with an existing
  `:enhanced` install whose master password has edge whitespace, a port must strip on **read** too.
  Decide explicitly rather than inheriting whichever the crate does.
- **`keyring` v3's Linux backend requires a running Secret Service with an unlocked collection.**
  It fails on headless / bare-WM / tmux-without-D-Bus. Lich hits the same wall via `secret-tool` and
  reports unavailable. **Detect and fall back to prompting; never hard-error.**
- `keyring` v3 on Windows uses `CredWriteW`/`CredReadW` but manages blob encoding itself. Interop
  with an existing Lich entry is **not wanted** anyway — use a separate service string (§6.2 item 5).
- `serde_yaml` has no Symbol type; existing files may contain `:symbol` scalars in the mode field.

---

## 11. THE SPIKE — Phase 2's first deliverable

> **CLOSED 2026-09-18.** The spike passed, and its successor — the ported `cena-platform::eaccess`
> driven by the `cena` binary — **completed a full live login**, with the author at the keyboard:
> `K A M F G P C L` clean, game socket open, a room description rendered from typed frames, a
> behavior interleaved with a manual command, `stop` in **84µs**, clean disconnect. That is
> `plan/12` §7.2 criteria 1–6 against the live server.
>
> Two things the run taught that reading could not:
>
> - **`L` answers with a family code.** A `GS3` login returns `GAMECODE=GS`, not `GS3`. Anything
>   comparing the launch payload's code against the requested instance must expect the shorter form.
> - **The `C` slot count reflects entitlement, not instance** — see the amendment in §4.6a. This
>   spec had it wrong, and only an account holding *two* entitlements could show that.
>
> One thing it did **not** teach, which is worth recording as a negative result: the two `<c>` ready
> signals and the WRAYTH banner behaved exactly as the A/B run of the same day predicted. No
> surprises in the game-socket handshake.
>
> The four unknowns below are settled (§12.1). The section is kept because the *method* — settle at
> the byte level before writing production code — is what produced a first live run with no protocol
> defects, and because §11.2's pass criterion ("recognizable game text, not a TCP accept") is the
> distinction the whole exercise turned on.

**This section is the point of the document.** Four things in this specification are uncertain at
the byte level, and **none of them can be settled by reading more Ruby.** The spike exists to settle
them before any production login code is written.

### 11.1 Definition

> **A single-file Rust binary that reads an account, password, character and game code from stdin,
> performs the full SGE exchange against `eaccess.play.net:7910`, connects to the returned game
> host/port, sends the three-part handshake, and prints the first 2 KB the game server sends back.**
> It writes nothing to disk except a `tcpdump`/`pktmon` capture, has no config file, no GUI, no
> session model, and no reconnect logic.

Everything else in Phase 2 — the `GameAdapter` seam, session management, the XML parser — waits on
this returning game text.

> **RESULT — PASSED 2026-09-18.** `spike/eaccess-spike`. Authenticates against the live service and
> prints game text with XML mode enabled. Wrong-password and wrong-game-code paths both fail in
> under 0.5s naming the stage. Four unit tests cover the hash (round-trip, out-of-range refusal,
> short-key refusal) and credential redaction.
>
> **What it cost, and what that bought:** the session's one real bug was a **lowercase game code**
> (§4.4a), which took four wrong theories to find because responses were parsed without first
> checking they answered the command sent. The guards that came out of it — validate the code
> against `M`, echo-check every response — are the spike's most portable output, more so than the
> login flow itself. §10.3a item 5.

### 11.2 Pass criterion

**The spike passes when it prints recognizable GemStone/DragonRealms game text — the login banner or
room description — proving XML mode was enabled.** Not "TLS connected." Not "got a KEY." Not "TCP
accepted." §5.2's field evidence is explicit that a TCP accept proves nothing; the same standard
applies here.

**Secondary pass criteria (all must also hold):**

- The K→L exchange completes with no protocol desync.
- A deliberately wrong password produces a clean `Error(<code>)` in under 2 seconds, with the
  failing stage named — **not a hang.** (Three of seven historical incidents were silent hangs.)
- The `tcpdump` capture is saved and answers S1-S4 below.

### 11.3 What the spike MUST measure — the four loud unknowns

> **S1 — SNI. HIGHEST VALUE. Capture this before writing protocol code.**
> Ruby sends **no** `server_name` extension; `rustls` **always** sends one. `eaccess.play.net` is
> behind an AWS load balancer. **If that balancer routes on SNI, Cena reaches a different backend
> than Lich does from the very first packet.** This is the one item where "the Ruby relies on gem
> behavior Rust differs on" is unambiguously true and unavoidable. Capture both a Lich login and a
> Cena login with `tcpdump`/`pktmon` and diff the ClientHellos. If SNI changes the backend, use an
> IP-based `ServerName` to suppress it.

> **S2 — TLS version floor.** Ruby permits **TLS 1.0+**; `rustls` is **TLS 1.2+ only**. Record the
> negotiated version from the capture. **If this endpoint negotiates 1.0 or 1.1, `rustls` is
> disqualified** and the TLS crate decision (§10.2) reverses. Cheap to measure, expensive to
> discover late.

> **S3 — Password-hash out-of-range behavior.** Ruby **raises** on both overflow (>255) and
> underflow (<0) — 14,336 of 65,536 byte pairs, split evenly between the two directions. **Lich has
> therefore never sent such a byte, and the server's behavior is completely unobserved.** Test with
> a password containing a byte below 0x20 and one that drives a result above 255, against a throwaway
> test account. Until measured, **Cena must replicate Ruby's failure (reject locally), not wrap** —
> that is the only behavior known to match production. Also determine whether Simutronics constrains
> the password charset server-side, which would make this moot (and would also settle the non-ASCII
> mangling question in §3.3 hazard #2).

> **S4 — Response terminators and framing.** Lich does one `sysread(8192)` per step and its own docs
> admit truncation is unhandled. **Cena must implement framed reads, which requires knowing the real
> terminator for each of K/A/M/F/G/P/C/L — and nobody has documented them.** Read the capture and
> write them down. Specifically settle §4.1: **does the K response carry a trailing newline?** The
> Ruby cannot tell you, because positional indexing never reaches past `len(password)-1`, and the
> spec fixture is a test double, not a capture. It is practically safe either way for passwords
> ≤31 characters — but **measure it rather than assume it.**

### 11.4 Suggested order of work

1. Capture a **Lich** login first, before writing any Rust. That single `tcpdump` answers S1, S2 and
   S4 and costs an evening.
2. Write the spike against what the capture shows.
3. Run the spike under capture; diff the two ClientHellos and the two exchanges byte for byte.
4. Only then start Phase 2 proper.

### 11.5 Unit tests the spike should carry

- The §3.3 vector: `"P@ssw0rd!"` with `"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"` →
  `[145, 130, 48, 55, 50, 118, 53, 44, 104]`.
- `"L\tPROBLEM\t1"` must **not** parse as success (the `/^L\tOK\t/` guard, §4.7 item 2).
- `"C\t0\t0\t0\t0\n"` (empty account) must parse to zero characters, not error.
- A character named `Foo^Bar` must resolve (the `[^\t\n]` class, §4.6).
- A character named `New` must resolve to its real code, not the generator (§4.6).
- `"L\tOK\tKEY=abc\n"` is a **valid success** with no GAMEHOST/GAMEPORT (§4.9).
- A password longer than 32 bytes is rejected **before** any socket write (§3.3 item 3).
- `"KEY=a=b"` parses to `key = "a=b"`, not `"a"` (§4.7 item 4).

---

## 12. Open and unverified items

Marked by confidence. Nothing here should be treated as protocol.

### 12.1 Must be settled by the spike (§11.3)

| # | Item | Status |
|---|---|---|
| S1 | SNI presence changing the backend | **VERIFIED 2026-09-18 — Lich sends NO SNI** |
| S2 | Negotiated TLS version (blocks the `rustls` decision) | **VERIFIED 2026-09-18 — TLS 1.2, static-RSA** |
| S3 | Server behavior for out-of-range password bytes | **UNOBSERVABLE BY DESIGN** — Ruby raises before sending, so Lich has never emitted one. Cena replicates the failure rather than wrapping. |
| S4 | Per-response terminators; K's trailing newline | **VERIFIED 2026-09-18 — there are NO terminators** |

#### S4 — no response carries a terminator. VERIFIED.

Captured 2026-09-18 by instrumenting `EAccess.read` (the single funnel for every response) to
log `data.inspect` during one real login. Probe removed afterward; live install verified
identical to the reference clone.

Observed, full sequence, with all 8 responses (account and key redacted):

| Cmd | len | Response shape |
|---|---:|---|
| `K` | 32 | 32 random bytes, **not printable ASCII** (contained ``) |
| `A` | 83 | `A	<ACCOUNT>	KEY	<32-hex>	<REAL NAME>` |
| `M` | 251 | `M	<code>	<name>` repeated — 11 games |
| `F` | 9 | `F	PREMIUM` |
| `G` | 731 | `G	<name>	<tier>	0		` then `KEY=VALUE` pairs, tab-separated |
| `P` | 32 | `P	GS3	1495	GS3.EC	250	GS3.P	2500` |
| `C` | 48 | `C	<n>	<n>	<n>	<n>	<charcode>	<CharName>` |
| `L` | 164 | `L	OK	UPPORT=…	GAME=…	GAMEHOST=…	GAMEPORT=…	KEY=…` |

**Not one response ends in `
`, `
`, or any terminator.** Each is a bare tab-delimited
record. This settles the question the spec flagged as *"the single most dangerous open
question."*

**Three consequences:**

1. **`K` has no trailing newline**, so the hash loop consumes exactly the bytes sent. A Rust
   port that strips or preserves a terminator behaves identically — the hazard is void.
2. **Framing is by read, not by delimiter.** A framed reader cannot scan for a terminator
   because there is none. Read what the socket yields and dispatch on the **leading command
   letter**, which is present and unambiguous in every response (`A`, `M`, `F`, `G`, `P`, `C`,
   `L`). Length is not a reliable frame boundary either — sizes vary from 9 to 731 bytes and
   are content-dependent.
3. **S3 is LIVE, not theoretical.** §12.2 INFERRED that passwords are printable ASCII and that
   a printable K key keeps the hash in range. **The captured K key is not printable ASCII** —
   it contained `` (DEL). So the inference's premise is false on the key side, and
   `((pw[i] - 32) ^ key[i]) + 32` can plausibly leave `0..255`. **Cena must replicate Ruby's
   raise rather than wrapping** (§12.1 S3), because wrapping would emit a byte Lich has never
   sent and the server has never been observed to accept.

**Security note:** the probe wrote a real session key and account name to disk. That file is
under `capture/` (gitignored) and must be deleted once read — the key at `L	KEY=` is the
live game credential.

#### S1 / S2 — the capture, and what it decides

Captured 2026-09-18, `tshark -i 8 -f "host eaccess.play.net"`, 59 packets, 2 ClientHellos,
during a real Lich login.

**S1 — Lich sends no SNI. VERIFIED.**

```
tshark -r eaccess.pcapng -Y "tls.handshake.type == 1" -T fields -e tls.handshake.extension.type
  -> 65281,11,10,35,22,23,49,13,43,45,51,27      (both ClientHellos)
```

**Extension type 0 (`server_name`) is absent.** A direct query for
`tls.handshake.extensions_server_name` returns nothing. This confirms the reading of
`eaccess.rb`, which never assigns `ssl_socket.hostname`.

> **Beware the false positive:** testing the field for emptiness reports "present" because the
> query yields an empty string either way. Query the **extension type list**, which is
> unambiguous.

**S2 — TLS 1.2, `TLS_RSA_WITH_AES_128_GCM_SHA256`. VERIFIED.**

```
tshark -r eaccess.pcapng -Y "tls.handshake.type == 2" -T fields -e tls.handshake.version -e tls.handshake.ciphersuite
  -> 0x0303  0x009c
```

`0x0303` = TLS 1.2. `0x009c` = `TLS_RSA_WITH_AES_128_GCM_SHA256` — **static RSA key exchange,
no forward secrecy.**

**Consequence — this decides the TLS crate, and it decides against `rustls`:**

| | |
|---|---|
| **`rustls` is ruled out** | it does not implement static-RSA key exchange (only ECDHE/DHE), so it cannot negotiate `0x009c`. It also has no supported way to omit SNI. Two independent blockers. |
| **Use `native-tls` or `openssl`** | both can negotiate static-RSA and both allow suppressing SNI, matching Lich's ClientHello |
| **Match Lich's ClientHello deliberately** | `eaccess.play.net` resolved to two AWS addresses (`3.22.54.28`, `18.118.231.211`) on 2026-09-18. A load balancer that routes on SNI would send Cena to a different backend than Lich. Sending no SNI is the known-good path. |
| **Certificate pinning still applies** (§2.3) | the cert is self-signed with no chain of trust; pin by **DER fingerprint**, not PEM string equality |

**Independently confirmed by VellumFE**, which already connects to this server. Its
`Cargo.toml:57-59` carries the same conclusion, reached before this capture existed:

> ```toml
> # NOTE: rustls cannot replace this — eaccess.play.net only speaks TLS 1.2 with
> # static-RSA key exchange (AES128-GCM-SHA256), which rustls refuses to implement.
> native-tls = "0.2"
> ```

Two further decisions to inherit from that file rather than re-derive:

- **One TLS stack, not two.** `ureq` is configured with `default-features = false,
  features = ["native-tls", ...]` (`Cargo.toml:82-83`), with the stated reason: *"native-tls
  stack as eAccess login; no rustls, no second TLS stack."* The HTTPS web-login fallback
  (§5) therefore shares the SGE path's TLS implementation.
- **`vendored` where the platform needs it** (`Cargo.toml:156`). A system-OpenSSL dependency
  is a classic cross-compilation failure, and `12` §1a requires the mobile targets to keep
  compiling in CI.

**VERIFIED means measured, not assumed** (`05` §−2 rule E.3). The commands above reproduce
the capture result; the Vellum comment corroborates it from an independent direction.

### 12.2 Uncertain, lower stakes

- **SETTLED 2026-09-18 — was: "what `L\tPROBLEM\t3` actually means is still open."** All four
  `PROBLEM` sub-codes are now documented from Saga 0.9.9, Simutronics' own client, whose
  user-facing English strings state them outright. **`PROBLEM 3` = the server has no
  configuration for the selected game** (server-side, not retryable). See §4.7 item 2a, which
  carries the full table, the retry split, and the record of what this spec previously believed
  and why both of its candidate readings were wrong.
  **The proposed experiment is no longer needed** — it was: select instance A with `G`, resolve
  a character code from A's `C` list, re-select instance B, then send A's code to `L`. Do not
  spend the login. Note that it would not have answered cleanly anyway: it was designed to
  separate "wrong instance for this code" from "broader session-state refusal", and the true
  meaning is **neither** — it is a server-configuration refusal. An experiment can only
  distinguish the hypotheses it was built from.
- **The `P\tGSX` echo in the 2026-09-18 Lich capture is unexplained.** `P` asked about **GST**
  returned GST's documented pricing (`1000 / -1 / 2500`, `analysis:146`) under the code **`GSX`**.
  A theory that `P` moves session state was built on this and **disproven** — removing `P` changed
  nothing (§4.5). The echo itself is still unaccounted for. Low stakes: nothing reads `P`'s
  response. Recorded so it is not re-discovered and re-theorised.
- **Character-name normalization is genuinely ambiguous.** Two disagreeing normalizers in-tree
  (§6.1): `entry_store.rb:1021` single-token vs `cli_password.rb:544` per-word. They differ on every
  multi-word name, and character name is part of the favorites identity 5-tuple. **Not resolved.**
  If Cena ever imports an existing `entry.yaml`, determine empirically which one wrote it —
  matching the wrong one silently orphans favorites rows.
- **Password charset constraints.** INFERRED (moderate confidence) that Simutronics restricts
  passwords to printable ASCII `0x20-0x7E`, which combined with a printable-ASCII K key keeps the
  hash in range. **No charset constraint appears anywhere in the source.** Folded into S3.
- **`DRX` and `DRF` web game codes are unverified guesses** with `expected_host: nil,
  expected_port: nil` (web_login.rb:146-147). The source comment says nobody has an account with
  those entitlements. Cena inherits the hole.
- **`entry.dat`'s write path.** state.rb:48 contains a Marshal writer. Whether it is still reachable
  is untraced. Confirm before declaring `entry.dat` out of scope — a stale writer could re-emit
  cleartext credentials after a conversion to encrypted.
- **`$_CLIENT_ = nil` at main.rb:396.** Cited in an earlier writeup; could not be confirmed at that
  line. The substance (headless runs with no listener and no client) is right; the citation is not.
- **`docs/eaccess-protocol-analysis.md` documents server commands Lich does not use.** It was
  produced by probing the real server, so it records response shapes Lich never parses. Worth
  re-reading in full if Cena ever wants multi-game enumeration or anything beyond the happy path.

### 12.3 Known-bad in Lich — do not port

- Case-sensitive `FATAL_ERROR_CODES` matching (§9.1) — normalize case.
- Single `sysread` with no framing (§3.2) — implement framed reads.
- Full-PEM string comparison for the cert pin (§2.3) — compare DER fingerprints.
- `verify_pem` continuing on the unverified connection after a mismatch (§2.3).
- `split("=")` with no limit truncating KEY values (§4.7) — `splitn(2, '=')`.
- The `A`-success regex depending on a trailing field (§4.2) — parse positionally, and never let the
  error path log `.split.last` when that could be the session key.
- `entitlement_response` covering four commands as one diagnostic stage (§4.10).
- The header/payload mode divergence in `save_entries` (§6.1).
- Dual Hash/Array return from one auth function (§4.8) — use an enum.

### 12.4 Security note

**No real credentials, tokens or account data were encountered anywhere in the tree**, and none
appear in this document. Every value shown here is synthetic: the spec fixture key
`ABCDEFGHIJKLMNOPQRSTUVWXYZ012345`, the demo password `P@ssw0rd!`, the account `TESTUSER`, the
recovered demo value `hunter2`, and the test fixtures `test_password` / `another_password`
(favorites_spec.rb:24, :43) and `TestMasterPassword123!` (master_password_manager_spec.rb:8).

**One caution for future digs:** on a live Lich install, `data/entry.yaml` **does** contain real
encrypted credentials, and `data/entry.yaml.bak` may contain them in cleartext from before a
conversion (§6.1). Exclude both from any operation that copies data out of a Lich directory. The
`--web-login-test` orchestration (`lib/common/cli/cli_orchestration.rb:293-345`) reads its password
from `entry.yaml` for exactly this reason and scrubs the KEY from its own success output.

---

## 13. Related documents

- `01-architecture.md` — Phase 2 gating; this document discharges its "highest-risk
  reimplementation" flag by converting an unknown protocol into four measurable unknowns.
- `09-gap-audit.md` — the audit that commissioned this dig.
- In the reference tree: `docs/eaccess-protocol-analysis.md` (live-probe writeup of the full server
  command surface, including commands Lich does not use — **the single most valuable artifact
  found**), `docs/eaccess-failure-diagnostics.md` (the stage taxonomy and its admitted gaps),
  `docs/web-login-protocol-analysis.md`.
