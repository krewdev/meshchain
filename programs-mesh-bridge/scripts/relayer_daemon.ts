/**
 * Automated Relayer Daemon for MeshChain ↔ Solana Bridge.
 * Monitors the Solana vault program for new deposits, resolves the recipient's
 * public key via the registry, and submits mint transactions to the MeshChain ledger.
 *
 * Production (public seed): systemd unit deploy/meshchain-relayer.service
 *
 * Manual:
 *   cd programs-mesh-bridge
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=$HOME/.config/solana/id.json \
 *   MESHCHAIN_ROOT=/opt/meshchain \
 *   MESH_MINT_PEER=127.0.0.1:9100 \
 *   npx ts-node --compiler-options '{"module":"commonjs","esModuleInterop":true,"resolveJsonModule":true}' \
 *     scripts/relayer_daemon.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import { execFileSync } from "child_process";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");

/** Repo root (…/meshchain). Override with MESHCHAIN_ROOT. */
function defaultRoot(): string {
  // programs-mesh-bridge/scripts → ../.. ; repo scripts/ → ..
  const parent = path.basename(path.dirname(__dirname));
  if (parent === "programs-mesh-bridge") {
    return path.resolve(__dirname, "../..");
  }
  return path.resolve(__dirname, "..");
}
const ROOT = process.env.MESHCHAIN_ROOT
  ? path.resolve(process.env.MESHCHAIN_ROOT)
  : defaultRoot();

/** Host data dir with v0/chain_state.json (public seed: data/host). */
const DATA = process.env.MESHCHAIN_DATA
  ? path.resolve(process.env.MESHCHAIN_DATA)
  : path.join(ROOT, "data/host");

const IDL_CANDIDATES = [
  process.env.MESH_BRIDGE_IDL,
  path.join(ROOT, "programs-mesh-bridge/idl/programs_mesh_bridge.json"),
  path.join(ROOT, "programs-mesh-bridge/target/idl/programs_mesh_bridge.json"),
].filter(Boolean) as string[];

const BIN_CANDIDATES = [
  process.env.MESHCHAIN_BIN,
  path.join(ROOT, "target/release/meshchain-node"),
  path.join(ROOT, "target/debug/meshchain-node"),
].filter(Boolean) as string[];

/** Gossip peer for mint (required on multi-validator public seed). */
const MINT_PEER =
  process.env.MESH_MINT_PEER || process.env.MESH_SUBMIT_PEER || "127.0.0.1:9100";

const VALIDATOR_INDEX = process.env.MESH_MINT_VALIDATOR_INDEX || "0";

function sha256(buf: Buffer): Buffer {
  return crypto.createHash("sha256").update(buf).digest();
}

const STATE_FILE = path.join(DATA, "relayer_state.json");

function loadRelayerState(): { processedSeqs: number[] } {
  if (fs.existsSync(STATE_FILE)) {
    try {
      return JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
    } catch {
      /* ignore */
    }
  }
  return { processedSeqs: [] };
}

function saveRelayerState(state: { processedSeqs: number[] }) {
  fs.mkdirSync(DATA, { recursive: true });
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

function findIdl(): string {
  for (const p of IDL_CANDIDATES) {
    if (p && fs.existsSync(p)) return p;
  }
  throw new Error(
    `IDL not found. Tried:\n  ${IDL_CANDIDATES.join("\n  ")}\n` +
      `Copy idl/programs_mesh_bridge.json or set MESH_BRIDGE_IDL.`
  );
}

function findNodeBin(): string {
  for (const p of BIN_CANDIDATES) {
    if (p && fs.existsSync(p)) return p;
  }
  throw new Error(
    `meshchain-node binary not found. Tried:\n  ${BIN_CANDIDATES.join("\n  ")}`
  );
}

/** Lookup recipient full public key from registry or chain_state. */
function resolvePublicKey(shortHex: string): string | null {
  const sid = shortHex.toLowerCase();

  const regCandidates = [
    path.join(DATA, "v0/registry.json"),
    path.join(DATA, "registry.json"),
  ];
  for (const regPath of regCandidates) {
    if (!fs.existsSync(regPath)) continue;
    try {
      const reg = JSON.parse(fs.readFileSync(regPath, "utf8"));
      // map short_id_hex -> pubkey hex, or nested
      if (reg[sid]) {
        const v = reg[sid];
        if (typeof v === "string") return v.replace(/^0x/, "");
        if (v && typeof v.pubkey === "string") return v.pubkey.replace(/^0x/, "");
        if (v && typeof v.public_key_hex === "string")
          return v.public_key_hex.replace(/^0x/, "");
      }
      if (reg.names && reg.names[sid]) {
        const v = reg.names[sid];
        if (typeof v === "string") return v.replace(/^0x/, "");
      }
    } catch {
      /* ignore */
    }
  }

  const stateCandidates = [
    path.join(DATA, "v0/chain_state.json"),
    path.join(DATA, "chain_state.json"),
  ];
  for (const statePath of stateCandidates) {
    if (!fs.existsSync(statePath)) continue;
    try {
      const st = JSON.parse(fs.readFileSync(statePath, "utf8"));
      const acc = st.accounts?.[sid];
      if (acc && acc.pubkey) {
        if (typeof acc.pubkey === "string") return acc.pubkey.replace(/^0x/, "");
        if (Array.isArray(acc.pubkey))
          return Buffer.from(acc.pubkey).toString("hex");
      }
      if (acc && acc.public_key_hex) {
        return String(acc.public_key_hex).replace(/^0x/, "");
      }
    } catch {
      /* ignore */
    }
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

  const toPubkeyHex = resolvePublicKey(shortHex);
  if (!toPubkeyHex) {
    console.warn(
      `⚠️ [Relayer] Could not resolve public key for short ID ${shortHex}. ` +
        `Register the wallet on mesh first (mesh register / faucet drip). Deposit deferred.`
    );
    return;
  }

  const extRef = sha256(Buffer.from(signature)).subarray(0, 16).toString("hex");
  const nodeBin = findNodeBin();
  const dataDir = fs.existsSync(path.join(DATA, "v0"))
    ? path.join(DATA, "v0")
    : DATA;

  console.log(`[Relayer] Minting tMESH via peer ${MINT_PEER}…`);
  try {
    // Use execFileSync so args are not shell-interpolated.
    execFileSync(
      nodeBin,
      [
        "mint-for-deposit",
        "--data-dir",
        dataDir,
        "--to-pubkey",
        toPubkeyHex,
        "--amount",
        amountNet,
        "--external-ref-hex",
        extRef,
        "--validator-index",
        VALIDATOR_INDEX,
        "--peer",
        MINT_PEER,
      ],
      { stdio: "inherit" }
    );

    state.processedSeqs.push(seq);
    // Bound growth
    if (state.processedSeqs.length > 10_000) {
      state.processedSeqs = state.processedSeqs.slice(-5_000);
    }
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
  console.log(`ROOT=${ROOT}`);
  console.log(`DATA=${DATA}`);
  console.log(`MINT_PEER=${MINT_PEER}`);

  const idlPath = findIdl();
  console.log(`IDL=${idlPath}`);
  findNodeBin(); // fail fast

  if (!process.env.ANCHOR_PROVIDER_URL) {
    console.warn(
      "ANCHOR_PROVIDER_URL unset — defaulting Anchor to cluster from env if any"
    );
  }
  if (!process.env.ANCHOR_WALLET) {
    console.error(
      "ANCHOR_WALLET required (path to Solana keypair JSON). Example: ~/.config/solana/id.json"
    );
    process.exit(1);
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
  // Anchor 0.30+ Program(idl, provider); older: Program(idl, programId, provider)
  let program: Program;
  try {
    program = new Program(idl, provider);
  } catch {
    program = new Program(idl, PROGRAM_ID, provider);
  }

  console.log(
    `Subscribing to DepositEvents from program ${PROGRAM_ID.toBase58()}…`
  );
  console.log(`RPC: ${provider.connection.rpcEndpoint}`);
  console.log(`Wallet: ${provider.wallet.publicKey.toBase58()}`);

  program.addEventListener("DepositEvent", (event: any, _slot: number, signature: string) => {
    processDeposit(event, signature).catch((e) => console.error(e));
  });

  console.log("Relayer Daemon online and listening.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
