/**
 * Automated Relayer: Solana vault deposits → MeshChain tMESH mint.
 *
 * Polls BridgeConfig.depositCount + DepositRecord PDAs (reliable on public RPC).
 * Event websockets on free RPCs often never fire.
 *
 *   systemd: deploy/meshchain-relayer.service
 *   manual:  ./scripts/start_relayer.sh
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import { execFileSync } from "child_process";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");

function defaultRoot(): string {
  const parent = path.basename(path.dirname(__dirname));
  if (parent === "programs-mesh-bridge") {
    return path.resolve(__dirname, "../..");
  }
  return path.resolve(__dirname, "..");
}

const ROOT = process.env.MESHCHAIN_ROOT
  ? path.resolve(process.env.MESHCHAIN_ROOT)
  : defaultRoot();

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

const MINT_PEER =
  process.env.MESH_MINT_PEER || process.env.MESH_SUBMIT_PEER || "127.0.0.1:9100";
const VALIDATOR_INDEX = process.env.MESH_MINT_VALIDATOR_INDEX || "0";
const POLL_MS = Number(process.env.MESH_RELAYER_POLL_MS || "4000");

function sha256(buf: Buffer): Buffer {
  return crypto.createHash("sha256").update(buf).digest();
}

const STATE_FILE = path.join(DATA, "relayer_state.json");

type RelayerState = { processedSeqs: number[]; lastSeenSeq: number };

function loadRelayerState(): RelayerState {
  if (fs.existsSync(STATE_FILE)) {
    try {
      const s = JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
      return {
        processedSeqs: s.processedSeqs || [],
        lastSeenSeq: typeof s.lastSeenSeq === "number" ? s.lastSeenSeq : -1,
      };
    } catch {
      /* ignore */
    }
  }
  return { processedSeqs: [], lastSeenSeq: -1 };
}

function saveRelayerState(state: RelayerState) {
  fs.mkdirSync(DATA, { recursive: true });
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

function findIdl(): string {
  for (const p of IDL_CANDIDATES) {
    if (p && fs.existsSync(p)) return p;
  }
  throw new Error(`IDL not found. Tried:\n  ${IDL_CANDIDATES.join("\n  ")}`);
}

function findNodeBin(): string {
  for (const p of BIN_CANDIDATES) {
    if (p && fs.existsSync(p)) return p;
  }
  throw new Error(
    `meshchain-node not found. Tried:\n  ${BIN_CANDIDATES.join("\n  ")}`
  );
}

function resolvePublicKey(shortHex: string): string | null {
  const sid = shortHex.toLowerCase();
  for (const regPath of [
    path.join(DATA, "v0/registry.json"),
    path.join(DATA, "registry.json"),
  ]) {
    if (!fs.existsSync(regPath)) continue;
    try {
      const reg = JSON.parse(fs.readFileSync(regPath, "utf8"));
      if (reg[sid]) {
        const v = reg[sid];
        if (typeof v === "string") return v.replace(/^0x/, "");
        if (v?.pubkey) return String(v.pubkey).replace(/^0x/, "");
        if (v?.public_key_hex) return String(v.public_key_hex).replace(/^0x/, "");
      }
      if (reg.names?.[sid]) {
        const v = reg.names[sid];
        if (typeof v === "string") return v.replace(/^0x/, "");
      }
    } catch {
      /* ignore */
    }
  }
  for (const statePath of [
    path.join(DATA, "v0/chain_state.json"),
    path.join(DATA, "chain_state.json"),
  ]) {
    if (!fs.existsSync(statePath)) continue;
    try {
      const st = JSON.parse(fs.readFileSync(statePath, "utf8"));
      const acc = st.accounts?.[sid];
      if (!acc) continue;
      if (typeof acc.pubkey === "string") return acc.pubkey.replace(/^0x/, "");
      if (Array.isArray(acc.pubkey)) return Buffer.from(acc.pubkey).toString("hex");
      if (acc.public_key_hex) return String(acc.public_key_hex).replace(/^0x/, "");
    } catch {
      /* ignore */
    }
  }
  return null;
}

function meshShortToHex(raw: unknown): string {
  if (Buffer.isBuffer(raw)) return raw.toString("hex");
  if (raw instanceof Uint8Array) return Buffer.from(raw).toString("hex");
  if (Array.isArray(raw)) return Buffer.from(raw).toString("hex");
  if (typeof raw === "string") {
    if (/^[0-9a-fA-F]{16}$/.test(raw)) return raw.toLowerCase();
    return Buffer.from(raw).toString("hex").slice(0, 16);
  }
  // BN / number array-like from anchor
  if (raw && typeof raw === "object" && "length" in (raw as any)) {
    try {
      return Buffer.from(raw as any).toString("hex");
    } catch {
      /* ignore */
    }
  }
  throw new Error(`bad mesh_short_id: ${JSON.stringify(raw)}`);
}

function amountToString(v: unknown): string {
  if (v == null) return "0";
  if (typeof v === "string" || typeof v === "number") return String(v);
  if (typeof (v as any).toString === "function") return (v as any).toString();
  return String(v);
}

async function processDeposit(
  seq: number,
  shortHex: string,
  amountNet: string,
  externalRefSeed: string
) {
  const state = loadRelayerState();
  if (state.processedSeqs.includes(seq)) {
    console.log(`[Relayer] seq=${seq} already processed`);
    return;
  }

  console.log(`[Relayer] Deposit seq=${seq}`);
  console.log(`  Mesh Short: ${shortHex}`);
  console.log(`  Amount Net: ${amountNet} base units`);

  const toPubkeyHex = resolvePublicKey(shortHex);
  if (!toPubkeyHex) {
    console.warn(
      `⚠️ [Relayer] no pubkey for ${shortHex} — register wallet on mesh first. Deferred.`
    );
    return;
  }

  const extRef = sha256(Buffer.from(externalRefSeed))
    .subarray(0, 16)
    .toString("hex");
  const nodeBin = findNodeBin();
  const dataDir = fs.existsSync(path.join(DATA, "v0"))
    ? path.join(DATA, "v0")
    : DATA;

  console.log(`[Relayer] mint via peer ${MINT_PEER}…`);
  try {
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
    if (state.processedSeqs.length > 10_000) {
      state.processedSeqs = state.processedSeqs.slice(-5_000);
    }
    if (seq > state.lastSeenSeq) state.lastSeenSeq = seq;
    saveRelayerState(state);
    console.log(`✅ [Relayer] minted seq=${seq}\n`);
  } catch (err) {
    console.error(`❌ [Relayer] mint failed:`, err);
  }
}

function depositPda(seq: number): PublicKey {
  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(BigInt(seq));
  return PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, seqBuf],
    PROGRAM_ID
  )[0];
}

async function main() {
  console.log("╔══════════════════════════════════════════════════╗");
  console.log("║    MeshChain ↔ Solana Bridge Relayer Daemon      ║");
  console.log("╚══════════════════════════════════════════════════╝");
  console.log(`ROOT=${ROOT}`);
  console.log(`DATA=${DATA}`);
  console.log(`MINT_PEER=${MINT_PEER}`);
  console.log(`POLL_MS=${POLL_MS}`);

  const idlPath = findIdl();
  console.log(`IDL=${idlPath}`);
  findNodeBin();

  if (!process.env.ANCHOR_WALLET) {
    console.error("ANCHOR_WALLET required");
    process.exit(1);
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const program = new Program(idl as anchor.Idl, provider) as any;

  const [configPda] = PublicKey.findProgramAddressSync(
    [CONFIG_SEED],
    PROGRAM_ID
  );
  console.log(`Program ${PROGRAM_ID.toBase58()}`);
  console.log(`Config  ${configPda.toBase58()}`);
  console.log(`RPC     ${provider.connection.rpcEndpoint}`);
  console.log(`Wallet  ${provider.wallet.publicKey.toBase58()}`);

  let state = loadRelayerState();
  console.log(
    `State: lastSeenSeq=${state.lastSeenSeq} processed=${state.processedSeqs.length}`
  );
  console.log("Relayer online — polling DepositRecord PDAs…");

  // Catch-up: if lastSeenSeq is -1, start from 0 so past deposits are minted.
  for (;;) {
    try {
      const cfg = await program.account.bridgeConfig.fetch(configPda);
      const depositCount = Number(cfg.depositCount.toString());
      // seq is 0-based index assigned at deposit; count is next free seq
      // records exist for seq in [0, depositCount)
      const start = Math.max(0, state.lastSeenSeq + 1);
      for (let seq = start; seq < depositCount; seq++) {
        if (state.processedSeqs.includes(seq)) {
          state.lastSeenSeq = Math.max(state.lastSeenSeq, seq);
          saveRelayerState(state);
          continue;
        }
        const pda = depositPda(seq);
        let rec: any;
        try {
          rec = await program.account.depositRecord.fetch(pda);
        } catch (e) {
          console.warn(`[Relayer] deposit PDA seq=${seq} missing:`, e);
          continue;
        }
        const shortHex = meshShortToHex(
          rec.meshShortId ?? rec.mesh_short_id
        );
        const amountNet = amountToString(rec.amountNet ?? rec.amount_net);
        // stable external ref without needing original sol sig
        const refSeed = `deposit-seq-${seq}-${shortHex}-${amountNet}`;
        await processDeposit(seq, shortHex, amountNet, refSeed);
        state = loadRelayerState();
        state.lastSeenSeq = Math.max(state.lastSeenSeq, seq);
        saveRelayerState(state);
      }
    } catch (e) {
      console.error("[Relayer] poll error:", e);
    }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
