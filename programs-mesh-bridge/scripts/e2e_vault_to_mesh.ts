/**
 * E2E: deposit devnet SOL → mint tMESH for mesh wallet.
 *
 * From programs-mesh-bridge (with yarn deps):
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=$HOME/.config/solana/id.json \
 *   npx ts-node --compiler-options '{"module":"commonjs","esModuleInterop":true,"resolveJsonModule":true}' \
 *     ../scripts/e2e_vault_to_mesh.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";

const PROGRAM_ID = new PublicKey(
  "CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx"
);
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const VAULT_SEED = Buffer.from("mesh-bridge-vault");
const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");

const ROOT = path.resolve(__dirname, "../..");
const DATA = path.join(ROOT, "data");
const IDL_PATH = path.join(
  ROOT,
  "programs-mesh-bridge/target/idl/programs_mesh_bridge.json"
);
const CARGO_TARGET = "/tmp/mc-e2e";

function sha256(buf: Buffer): Buffer {
  return crypto.createHash("sha256").update(buf).digest();
}

function shortIdFromPubkeyHex(pubkeyHex: string): Buffer {
  return sha256(Buffer.from(pubkeyHex, "hex")).subarray(0, 8);
}

function cargoEnv() {
  return { ...process.env, CARGO_TARGET_DIR: CARGO_TARGET };
}

function ensureGenesis() {
  if (!fs.existsSync(path.join(DATA, "genesis.json"))) {
    console.log("Running mesh testnet-setup…");
    execSync("cargo run -q -p mesh -- testnet-setup", {
      cwd: ROOT,
      stdio: "inherit",
      env: cargoEnv(),
    });
  }
}

function ensureMeshWallet(): { pubkeyHex: string; shortHex: string; path: string } {
  const keysDir = path.join(DATA, "keys");
  fs.mkdirSync(keysDir, { recursive: true });
  const walletPath = path.join(keysDir, "user_e2e.json");
  if (!fs.existsSync(walletPath)) {
    execSync(
      `cargo run -q -p meshchain-wallet -- keygen --out "${walletPath}"`,
      { cwd: ROOT, stdio: "inherit", env: cargoEnv() }
    );
  }
  const w = JSON.parse(fs.readFileSync(walletPath, "utf8"));
  const pubkeyHex = w.public_hex as string;
  const shortHex = shortIdFromPubkeyHex(pubkeyHex).toString("hex");
  return { pubkeyHex, shortHex, path: walletPath };
}

async function main() {
  console.log("=== E2E: vault SOL (devnet) → tMESH mint ===\n");
  ensureGenesis();
  const wallet = ensureMeshWallet();
  console.log("Mesh wallet:", wallet.path);
  console.log("Mesh short:", wallet.shortHex);

  const meshShort = Array.from(shortIdFromPubkeyHex(wallet.pubkeyHex));
  const depositSol = 0.05;
  const amount = new anchor.BN(Math.floor(depositSol * LAMPORTS_PER_SOL));

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(IDL_PATH, "utf8"));
  const program = new Program(idl, provider);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [vaultPda] = PublicKey.findProgramAddressSync([VAULT_SEED], PROGRAM_ID);
  const cfg = await (program.account as any).bridgeConfig.fetch(configPda);
  const seq = cfg.depositCount as anchor.BN;
  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(BigInt(seq.toString()));
  const [depositPda] = PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, seqBuf],
    PROGRAM_ID
  );

  console.log("\n--- 1) Deposit SOL on vault ---");
  console.log("Amount:", depositSol, "SOL → bound to", wallet.shortHex);
  const sig = await program.methods
    .depositSol(amount, meshShort)
    .accounts({
      depositor: provider.wallet.publicKey,
      config: configPda,
      solVault: vaultPda,
      depositRecord: depositPda,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log("tx:", sig);
  console.log("https://explorer.solana.com/tx/" + sig + "?cluster=devnet");

  const dep = await (program.account as any).depositRecord.fetch(depositPda);
  const amountNet = dep.amountNet.toString();
  console.log("net lamports (mint amount):", amountNet);
  console.log("fee lamports:", dep.fee.toString());

  const extRef = sha256(Buffer.from(sig)).subarray(0, 16).toString("hex");

  console.log("\n--- 2) Mint tMESH on mesh ledger ---");
  execSync(
    `cargo run -q -p meshchain-node -- mint-for-deposit --data-dir "${DATA}" --to-pubkey ${wallet.pubkeyHex} --amount ${amountNet} --external-ref-hex ${extRef} --validator-index 0`,
    { cwd: ROOT, stdio: "inherit", env: cargoEnv() }
  );

  const state = JSON.parse(
    fs.readFileSync(path.join(DATA, "chain_state.json"), "utf8")
  );
  const acc = state.accounts[wallet.shortHex];
  const summary = {
    step: "vault_deposit_then_mesh_mint",
    mesh_short_id: wallet.shortHex,
    mesh_wallet_file: wallet.path,
    sol_deposit_tx: sig,
    deposit_sol: depositSol,
    amount_net_lamports: amountNet,
    tmesh_balance_base: acc?.balance ?? null,
    tmesh_balance_display: acc ? Number(acc.balance) / 1e6 : null,
    chain_id: state.chain_id,
    height: state.height,
    explorer: `https://explorer.solana.com/tx/${sig}?cluster=devnet`,
  };
  const out = path.join(DATA, "e2e_last_result.json");
  fs.writeFileSync(out, JSON.stringify(summary, null, 2));
  console.log("\n=== SUCCESS ===");
  console.log(JSON.stringify(summary, null, 2));
  console.log("Saved", out);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
