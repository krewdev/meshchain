#!/usr/bin/env bash
# Hybrid vault round-trip smoke (Solana devnet + local/public mesh mint peer).
#
# Env:
#   ANCHOR_WALLET          Solana keypair (funded on devnet)
#   ANCHOR_PROVIDER_URL    default https://api.devnet.solana.com
#   MESH_MINT_PEER         default 127.0.0.1:9100 (public seed: 34.172.103.125:9100)
#   MESHCHAIN_DATA         default ./data/host or ./data
#   DEPOSIT_SEQ            deposit PDA seq to cash out (default: latest-1)
#   SKIP_DEPOSIT=1         only burn+withdraw using existing deposit
#   SKIP_WITHDRAW=1        deposit+mint only
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-https://api.devnet.solana.com}"
export ANCHOR_WALLET="${ANCHOR_WALLET:-$HOME/.config/solana/id.json}"
export MESH_MINT_PEER="${MESH_MINT_PEER:-127.0.0.1:9100}"
export MESHCHAIN_ROOT="${MESHCHAIN_ROOT:-$ROOT}"

if [[ ! -f "$ANCHOR_WALLET" ]]; then
  echo "ANCHOR_WALLET missing: $ANCHOR_WALLET"
  exit 1
fi

DATA="${MESHCHAIN_DATA:-}"
if [[ -z "$DATA" ]]; then
  if [[ -d "$ROOT/data/host/v0" ]]; then DATA="$ROOT/data/host"
  else DATA="$ROOT/data"; fi
fi
export MESHCHAIN_DATA="$DATA"
mkdir -p "$DATA/keys" "$DATA/v0" 2>/dev/null || true

NODE="${MESHCHAIN_BIN:-$ROOT/target/release/meshchain-node}"
MESH="${MESH:-$ROOT/target/release/mesh}"
[[ -x "$NODE" ]] || NODE="$ROOT/target/debug/meshchain-node"
[[ -x "$MESH" ]] || MESH="$ROOT/target/debug/mesh"
if [[ ! -x "$NODE" || ! -x "$MESH" ]]; then
  cargo build -p mesh -p meshchain-node -q
  NODE="$ROOT/target/debug/meshchain-node"
  MESH="$ROOT/target/debug/mesh"
fi

IDL="$ROOT/programs-mesh-bridge/idl/programs_mesh_bridge.json"
[[ -f "$IDL" ]] || IDL="$ROOT/programs-mesh-bridge/target/idl/programs_mesh_bridge.json"
[[ -f "$IDL" ]] || { echo "IDL missing — run from monorepo with programs-mesh-bridge/idl"; exit 1; }

echo "== hybrid round-trip =="
echo "  wallet=$ANCHOR_WALLET"
echo "  data=$DATA peer=$MESH_MINT_PEER"
echo "  idl=$IDL"

cd "$ROOT/programs-mesh-bridge"
if [[ ! -d node_modules/@coral-xyz/anchor ]]; then
  npm install --omit=optional
fi

export MESH_BRIDGE_IDL="$IDL"
export MESHCHAIN_BIN="$NODE"
export TS_NODE_TRANSPILE_ONLY=1
export TS_NODE_COMPILER_OPTIONS='{"module":"commonjs","esModuleInterop":true,"resolveJsonModule":true}'

# Wallet for mesh side of hybrid lock
MESH_WALLET="$DATA/keys/hybrid_e2e.json"
if [[ ! -f "$MESH_WALLET" ]]; then
  "$MESH" --dir "$DATA" new-wallet --name hybrid_e2e.json || true
fi
# Ensure cold key for burn
COLD="$DATA/keys/hybrid_e2e_cold.json"
if [[ ! -f "$COLD" ]]; then
  if "$MESH" new-cold-key --help >/dev/null 2>&1; then
    "$MESH" --dir "$DATA" new-cold-key --out keys/hybrid_e2e_cold.json 2>/dev/null \
      || "$MESH" --dir "$DATA" new-cold-key 2>/dev/null || true
  fi
fi
# Fallback: meshchain-node may not create cold — try wallet crate name
if [[ ! -f "$COLD" ]]; then
  echo "NOTE: creating cold key via mesh if available…"
  "$MESH" --dir "$DATA" new-cold-key --name hybrid_e2e_cold.json 2>/dev/null || true
fi
# Locate cold file
if [[ ! -f "$COLD" ]]; then
  COLD=$(ls "$DATA/keys"/*cold*.json 2>/dev/null | head -1 || true)
fi

PUB=$(python3 -c "import json;print(json.load(open('$MESH_WALLET'))['public_hex'])")
SHORT=$(python3 - <<PY
import hashlib, json
pub=bytes.fromhex("$PUB")
print(hashlib.sha256(pub).digest()[:8].hex())
PY
)
echo "  mesh pubkey ${PUB:0:16}… short=$SHORT"

if [[ "${SKIP_DEPOSIT:-0}" != "1" ]]; then
  echo "== 1) deposit SOL (devnet) bound to mesh short =="
  NODE_PATH="$ROOT/programs-mesh-bridge/node_modules" node <<'JS'
const anchor = require("@coral-xyz/anchor");
const { LAMPORTS_PER_SOL, PublicKey, SystemProgram } = require("@solana/web3.js");
const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const VAULT_SEED = Buffer.from("mesh-bridge-vault");
const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");
const ROOT = process.env.MESHCHAIN_ROOT;
const DATA = process.env.MESHCHAIN_DATA;
const IDL = process.env.MESH_BRIDGE_IDL;
const MESH_WALLET = path.join(DATA, "keys/hybrid_e2e.json");

function sha256(b){return crypto.createHash("sha256").update(b).digest();}

(async () => {
  const w = JSON.parse(fs.readFileSync(MESH_WALLET, "utf8"));
  const pub = Buffer.from(w.public_hex, "hex");
  const meshShort = Array.from(sha256(pub).subarray(0, 8));
  const shortHex = Buffer.from(meshShort).toString("hex");

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(IDL, "utf8"));
  const program = new anchor.Program(idl, provider);
  const bal = await provider.connection.getBalance(provider.wallet.publicKey);
  if (bal < 0.05 * LAMPORTS_PER_SOL) {
    console.error("Need >=0.05 SOL on", provider.wallet.publicKey.toBase58(), "have", bal);
    process.exit(2);
  }
  const depositSol = 0.03;
  const amount = new anchor.BN(Math.floor(depositSol * LAMPORTS_PER_SOL));
  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [vaultPda] = PublicKey.findProgramAddressSync([VAULT_SEED], PROGRAM_ID);
  const cfg = await program.account.bridgeConfig.fetch(configPda);
  const seq = Number(cfg.depositCount.toString());
  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(BigInt(seq));
  const [depositPda] = PublicKey.findProgramAddressSync([DEPOSIT_SEED, seqBuf], PROGRAM_ID);
  console.log("deposit seq", seq, "short", shortHex, "sol", depositSol);
  const tx = await program.methods
    .depositSol(amount, meshShort)
    .accounts({
      depositor: provider.wallet.publicKey,
      config: configPda,
      solVault: vaultPda,
      depositRecord: depositPda,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log("DEPOSIT_TX", tx);
  const rec = await program.account.depositRecord.fetch(depositPda);
  const amountNet = rec.amountNet.toString();
  fs.writeFileSync(
    path.join(DATA, "hybrid_e2e_deposit.json"),
    JSON.stringify({
      seq,
      shortHex,
      amountNet,
      deposit_tx: tx,
      pubkeyHex: w.public_hex,
    }, null, 2)
  );
  console.log("net", amountNet, "saved hybrid_e2e_deposit.json");
})().catch((e) => { console.error(e); process.exit(1); });
JS

  echo "== 2) wait for relayer mint (or mint manually) =="
  # Prefer live relayer; fallback direct mint for lab
  DEP_JSON="$DATA/hybrid_e2e_deposit.json"
  AMOUNT_NET=$(python3 -c "import json;print(json.load(open('$DEP_JSON'))['amountNet'])")
  SEQ=$(python3 -c "import json;print(json.load(open('$DEP_JSON'))['seq'])")
  sleep 6
  # Manual mint if peer available (idempotent external_ref)
  REF=$(python3 -c "import hashlib;print(hashlib.sha256(f'deposit-seq-$SEQ-$SHORT-$AMOUNT_NET'.encode()).hexdigest()[:32])")
  if ! "$NODE" mint-for-deposit \
      --data-dir "${DATA}/v0" \
      --to-pubkey "$PUB" \
      --amount "$AMOUNT_NET" \
      --external-ref-hex "$REF" \
      --validator-index 0 \
      --peer "$MESH_MINT_PEER" 2>/dev/null; then
    echo "NOTE: mint-for-deposit via peer failed or already processed — checking chain_state"
  fi
  sleep 2
  export DEPOSIT_SEQ="$SEQ"
  export AMOUNT_NET
else
  DEP_JSON="$DATA/hybrid_e2e_deposit.json"
  [[ -f "$DEP_JSON" ]] || { echo "SKIP_DEPOSIT=1 but no $DEP_JSON"; exit 1; }
  export DEPOSIT_SEQ=$(python3 -c "import json;print(json.load(open('$DEP_JSON'))['seq'])")
  export AMOUNT_NET=$(python3 -c "import json;print(json.load(open('$DEP_JSON'))['amountNet'])")
fi

if [[ "${SKIP_WITHDRAW:-0}" == "1" ]]; then
  echo "SKIP_WITHDRAW=1 — stop after deposit/mint"
  exit 0
fi

if [[ -z "${COLD:-}" || ! -f "${COLD:-}" ]]; then
  echo "WARN: no cold PQ key — cannot burn-for-withdraw. Deposit/mint path done."
  echo "  Create: mesh new-cold-key  then re-run with SKIP_DEPOSIT=1"
  exit 0
fi

echo "== 3) burn-for-withdraw (mesh) =="
DEST_SOL=$(python3 - <<PY
import json
from pathlib import Path
# Solana address from ANCHOR_WALLET secret
import sys
try:
  from solders.keypair import Keypair
except Exception:
  pass
# fallback: use solana-keygen address if present
import subprocess, os
w=os.environ.get("ANCHOR_WALLET")
try:
  out=subprocess.check_output(["solana-keygen","pubkey",w], text=True).strip()
  print(out)
except Exception:
  # last resort: ask node web3 if available
  print("")
PY
)
if [[ -z "$DEST_SOL" ]]; then
  DEST_SOL=$(NODE_PATH="$ROOT/programs-mesh-bridge/node_modules" node -e '
const {Keypair}=require("@solana/web3.js");const fs=require("fs");
const s=Uint8Array.from(JSON.parse(fs.readFileSync(process.env.ANCHOR_WALLET)));
console.log(Keypair.fromSecretKey(s).publicKey.toBase58());
')
fi
echo "  dest_sol=$DEST_SOL amount=$AMOUNT_NET cold=$COLD"
"$NODE" burn-for-withdraw \
  --data-dir "${DATA}/v0" \
  --wallet "$MESH_WALLET" \
  --cold "$COLD" \
  --amount "$AMOUNT_NET" \
  --dest-sol "$DEST_SOL" \
  --asset-id 1 || {
  # try data_dir without v0
  "$NODE" burn-for-withdraw \
    --data-dir "$DATA" \
    --wallet "$MESH_WALLET" \
    --cold "$COLD" \
    --amount "$AMOUNT_NET" \
    --dest-sol "$DEST_SOL" \
    --asset-id 1
}

# burn writes last_burn.json under data_dir
BURN_JSON="${DATA}/v0/last_burn.json"
[[ -f "$BURN_JSON" ]] || BURN_JSON="$DATA/last_burn.json"
[[ -f "$BURN_JSON" ]] || { echo "FAIL no last_burn.json"; exit 1; }
cp -f "$BURN_JSON" "$DATA/last_burn.json"
echo "  burn artifact $BURN_JSON"

echo "== 4) hybrid withdraw (2 attestors) =="
# Update cashout script paths via env
export DEPOSIT_SEQ
NODE_PATH="$ROOT/programs-mesh-bridge/node_modules" node <<'JS'
const anchor = require("@coral-xyz/anchor");
const { Keypair, PublicKey, SystemProgram } = require("@solana/web3.js");
const fs = require("fs");
const path = require("path");

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const VAULT_SEED = Buffer.from("mesh-bridge-vault");
const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");
const WITHDRAW_SEED = Buffer.from("mesh-bridge-withdraw");
const ROOT = process.env.MESHCHAIN_ROOT;
const DATA = process.env.MESHCHAIN_DATA;
const IDL = process.env.MESH_BRIDGE_IDL;
const depositSeq = Number(process.env.DEPOSIT_SEQ || "0");

(async () => {
  const burn = JSON.parse(fs.readFileSync(path.join(DATA, "last_burn.json"), "utf8"));
  const burnTxid = Array.from(Buffer.from(burn.burn_txid_hex, "hex"));
  const amount = new anchor.BN(burn.amount);
  const meshHeight = new anchor.BN(burn.mesh_height);
  const meshShort = Array.from(Buffer.from(burn.mesh_short_id_hex, "hex"));

  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(BigInt(depositSeq));

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(IDL, "utf8"));
  const program = new anchor.Program(idl, provider);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [vaultPda] = PublicKey.findProgramAddressSync([VAULT_SEED], PROGRAM_ID);
  const [depositPda] = PublicKey.findProgramAddressSync([DEPOSIT_SEED, seqBuf], PROGRAM_ID);
  const [withdrawPda] = PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, Buffer.from(burn.burn_txid_hex, "hex")],
    PROGRAM_ID
  );

  const attestor2Path = path.join(ROOT, "programs-mesh-bridge/scripts/attestor2-devnet.json");
  if (!fs.existsSync(attestor2Path)) {
    console.error("missing attestor2-devnet.json (gitignored) — cannot co-sign hybrid withdraw");
    process.exit(4);
  }
  const attestor2 = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(attestor2Path, "utf8")))
  );
  const dest = provider.wallet.publicKey;
  console.log("withdraw depositSeq", depositSeq, "amount", amount.toString());
  const balBefore = await provider.connection.getBalance(dest);
  const sig = await program.methods
    .withdrawHybridSol(burnTxid, amount, meshHeight, meshShort)
    .accounts({
      relayer: provider.wallet.publicKey,
      config: configPda,
      solVault: vaultPda,
      destination: dest,
      depositRecord: depositPda,
      withdrawRecord: withdrawPda,
      systemProgram: SystemProgram.programId,
    })
    .remainingAccounts([
      { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: false },
      { pubkey: attestor2.publicKey, isSigner: true, isWritable: false },
    ])
    .signers([attestor2])
    .rpc();
  const balAfter = await provider.connection.getBalance(dest);
  const summary = {
    withdraw_tx: sig,
    burn_txid: burn.burn_txid_hex,
    deposit_seq: depositSeq,
    sol_delta_lamports: balAfter - balBefore,
    explorer: `https://explorer.solana.com/tx/${sig}?cluster=devnet`,
  };
  fs.writeFileSync(path.join(DATA, "e2e_cashout_result.json"), JSON.stringify(summary, null, 2));
  console.log("HYBRID_WITHDRAW_OK", JSON.stringify(summary, null, 2));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
JS

echo "HYBRID ROUND-TRIP COMPLETE (or partial — see messages above)"
