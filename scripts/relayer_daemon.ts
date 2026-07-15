/**
 * Automated Relayer Daemon for MeshChain ↔ Solana Bridge.
 * Monitors the Solana vault program for new deposits, resolves the recipient's
 * public key via the registry, and submits mint transactions to the MeshChain ledger.
 *
 * To run:
 *   cd /Users/krewdev/meshchain/programs-mesh-bridge
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=$HOME/.config/solana/id.json \
 *   npx ts-node --compiler-options '{"module":"commonjs","esModuleInterop":true,"resolveJsonModule":true}' \
 *     ../scripts/relayer_daemon.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");

const ROOT = path.resolve(__dirname, "..");
const DATA = path.join(ROOT, "data/host");
const IDL_PATH = path.join(ROOT, "programs-mesh-bridge/target/idl/programs_mesh_bridge.json");
const BIN = path.join(ROOT, "target/release/meshchain-node");

// Helper to calculate sha256 hash
function sha256(buf: Buffer): Buffer {
  return crypto.createHash("sha256").update(buf).digest();
}

// Relayer state persistence to prevent double-minting
const STATE_FILE = path.join(DATA, "relayer_state.json");
function loadRelayerState(): { processedSeqs: number[] } {
  if (fs.existsSync(STATE_FILE)) {
    try {
      return JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
    } catch (e) {}
  }
  return { processedSeqs: [] };
}

function saveRelayerState(state: { processedSeqs: number[] }) {
  fs.mkdirSync(DATA, { recursive: true });
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

// Lookup recipient's full public key from ledger chain_state or faucet registry
function resolvePublicKey(shortHex: string): string | null {
  // 1. Try registry.json (from faucet)
  const regPath = path.join(DATA, "v0/registry.json");
  if (fs.existsSync(regPath)) {
    try {
      const reg = JSON.parse(fs.readFileSync(regPath, "utf8"));
      if (reg[shortHex]) return reg[shortHex];
    } catch (e) {}
  }

  // 2. Try chain_state.json
  const statePath = path.join(DATA, "v0/chain_state.json");
  if (fs.existsSync(statePath)) {
    try {
      const st = JSON.parse(fs.readFileSync(statePath, "utf8"));
      const acc = st.accounts?.[shortHex];
      if (acc && acc.pubkey) {
        if (typeof acc.pubkey === "string") return acc.pubkey;
        if (Array.isArray(acc.pubkey)) return Buffer.from(acc.pubkey).toString("hex");
      }
    } catch (e) {}
  }

  return null;
}

async function processDeposit(event: any, signature: string) {
  const seq = Number(event.seq.toString());
  const state = loadRelayerState();

  if (state.processedSeqs.includes(seq)) {
    console.log(`[Relayer] Deposit seq=${seq} already processed. Skipping.`);
    return;
  }

  const shortIdBytes = Buffer.from(event.meshShortId);
  const shortHex = shortIdBytes.toString("hex");
  const amountNet = event.amountNet.toString();

  console.log(`[Relayer] New Deposit Detected!`);
  console.log(`  Seq:        ${seq}`);
  console.log(`  Mesh Short: ${shortHex}`);
  console.log(`  Amount Net: ${amountNet} base units`);
  console.log(`  Solana Tx:  ${signature}`);

  // Resolve full public key
  const toPubkeyHex = resolvePublicKey(shortHex);
  if (!toPubkeyHex) {
    console.warn(`⚠️ [Relayer] Could not resolve public key for short ID ${shortHex}. Deposit deferred.`);
    return;
  }

  // 16-byte external ref hash from Solana signature to enforce idempotency
  const extRef = sha256(Buffer.from(signature)).subarray(0, 16).toString("hex");

  // Determine correct binary
  const nodeBin = fs.existsSync(BIN) ? BIN : path.join(ROOT, "target/debug/meshchain-node");

  console.log(`[Relayer] Minting tMESH on MeshChain ledger...`);
  try {
    const cmd = `"${nodeBin}" mint-for-deposit --data-dir "${path.join(DATA, "v0")}" --to-pubkey ${toPubkeyHex} --amount ${amountNet} --external-ref-hex ${extRef} --validator-index 0`;
    execSync(cmd, { stdio: "inherit" });
    
    // Save to processed list
    state.processedSeqs.push(seq);
    saveRelayerState(state);
    console.log(`✅ [Relayer] Successfully processed deposit seq=${seq}\n`);
  } catch (err) {
    console.error(`❌ [Relayer] Minting execution failed:`, err);
  }
}

async function main() {
  console.log("╔══════════════════════════════════════════════════╗");
  console.log("║    MeshChain ↔ Solana Bridge Relayer Daemon      ║");
  console.log("╚══════════════════════════════════════════════════╝");

  if (!fs.existsSync(IDL_PATH)) {
    console.error(`IDL file not found at ${IDL_PATH}. Build programs-mesh-bridge first.`);
    process.exit(1);
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(IDL_PATH, "utf8"));
  const program = new Program(idl, provider);

  console.log(`Subscribing to DepositEvents from program ${PROGRAM_ID.toBase58()}...`);

  // Subscribe to live events
  program.addEventListener("DepositEvent", (event: any, slot: number, signature: string) => {
    processDeposit(event, signature).catch((e) => console.error(e));
  });

  // Keep process alive
  console.log("Relayer Daemon online and listening.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
