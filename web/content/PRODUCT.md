# Product: mesh-gated Solana cash

**Not a messenger. Not “Solana tx over LoRa.” Not tMESH-as-money.**

MeshChain’s only claim that is not already a crowded demo:

> Solana-held value that cannot move with internet alone — it needs the mesh.

Everything else is transport.

## Why this, not the other headlines

| Idea | Status | Verdict |
|------|--------|---------|
| Encrypted radio chat | Meshtastic PKC DMs exist | Not the product |
| Execute a Solana tx from the trail | Soltastic / SolMesh Mode 1 | Commodity |
| tMESH + mesh PoA as money | Parallel token on 200-byte frames | Hard to trust |
| Agent wallet kit / new brand | Resets proof you already have | Waste |
| Hybrid vault + offline IOU that settles to USDC/SOL | Started in this repo | **The idea** |

People do not need another ledger in the woods. They need: load USDC while they have LTE, walk into a dead zone, pay someone on the same channel, both see a receipt over radio, L1 moves when either hits a gateway — and a laptop on hotel Wi-Fi cannot drain the vault without mesh witnesses.

That is offline cash with a Solana bank window.

## The loop that must work

1. **Online** — deposit USDC/SOL into the vault PDA, bound to `mesh_short_id`.
2. **Offline** — sign a bounded IOU (`amount`, Solana `dest`, `nonce`, `expiry`, `deposit_seq`) with the key the vault already knows.
3. **Radio** — one frame (`AirIou` type 15, 129 B) + ack (`AirIouAck` type 16, 82 B). No 1232-byte Solana tx on the trail.
4. **Online again** — first gateway + required mesh witnesses complete `withdraw_hybrid_*`. Double-pay rejected by `iou_id` / nonce.
5. **Stolen laptop / RPC-only attacker** — cannot settle without the witness set.

If step 5 fails, this is a delayed Solana wallet, not MeshChain.

tMESH stays a **test counter** so the mesh can practice mint/burn. Product settlement is **devnet then mainnet USDC/SOL in the vault**.

## What already exists

- Vault program with `hybrid_enabled`, attestors, `deposit_*`, `withdraw_hybrid_sol/spl` (`programs-mesh-bridge`)
- Identifier binding: deposit stores `mesh_short_id`; unlock must match
- Relayer: Solana deposit → mesh mint (test counter)
- Mesh `Burn` + attestor co-sign path (current off-ramp)
- **New:** `AirIou` / `AirIouAck` codec (`crates/proto/src/iou.rs`) and radio relay types 15/16

## What to ship next (in order)

1. Two-radio demo: airplane-mode IOU → mesh receipt → later Solscan withdraw (devnet USDC).
2. Witness daemon that signs `AirIouAck` only after it saw the IOU on air or local relay.
3. Map `iou_id` → `burn_txid` seed on the existing withdraw PDA (no new program if the 32-byte id hashes up).
4. Video of that loop. Then Superteam / Foundation on the **program + witness spec**, not a token narrative.

## What not to start

- A messenger repo
- Grant copy for “AA Wallet Kit”
- Growing mesh consensus so tMESH looks like an L1
- A logo pass before the vault settle video

Chat can ride as an optional Msg frame in the same codec. It is a checkbox, not the pitch.
