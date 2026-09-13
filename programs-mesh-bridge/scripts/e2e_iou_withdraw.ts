/**
 * Product-loop tail: last_iou.json → withdraw_hybrid_sol on DEVNET.
 * Does not mint RELAY. Does not talk to mainnet.
 *
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=~/.config/solana/id.json \
 *   DEPOSIT_SEQ=0 \
 *   npx ts-node scripts/e2e_iou_withdraw.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const VAULT_SEED = Buffer.from("mesh-bridge-vault");
const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");
const WITHDRAW_SEED = Buffer.from("mesh-bridge-withdraw");

const ROOT = path.resolve(__dirname, "../..");
const DATA = process.env.MESHCHAIN_DATA
  ? path.resolve(process.env.MESHCHAIN_DATA)
  : path.join(ROOT, "data");

function findIdl(): string {
  const candidates = [
    process.env.MESH_BRIDGE_IDL,
    path.join(ROOT, "programs-mesh-bridge/idl/programs_mesh_bridge.json"),
    path.join(ROOT, "programs-mesh-bridge/target/idl/programs_mesh_bridge.json"),
  ].filter(Boolean) as string[];
  for (const p of candidates) {
    if (fs.existsSync(p)) return p;
  }
  throw new Error("IDL not found");
}

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const genesis = await provider.connection.getGenesisHash();
  if (genesis !== DEVNET_GENESIS) {
    throw new Error(`refusing to withdraw off devnet (genesis=${genesis})");
  }

  const iouPath = fs.existsSync(path.join(DATA, "last_iou.json"))
    ? path.join(DATA, "last_iou.json")
    : path.join(DATA, "v0/last_iou.json");
  if (!fs.existsSync(iouPath)) {
    throw new Error("no data/last_iou.json — run python3 tools/mesh_iou.py first");
  }
  const iou = JSON.parse(fs.readFileSync(iouPath, "utf8"));
  const burnTxid = Array.from(Buffer.from(iou.burn_txid_hex, "hex"));
  const amount = new anchor.BN(iou.amount);
  const meshHeight = new anchor.BN(process.env.MESH_HEIGHT || "0");
  const meshShort = Array.from(Buffer.from(iou.from_hex, "hex"));
  const depositSeq = Number(iou.deposit_seq ?? process.env.DEPOSIT_SEQ ?? "0");
  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(BigInt(depositSeq));

  const program = new Program(
    JSON.parse(fs.readFileSync(findIdl(), "utf8")) as anchor.Idl,
    provider
  );
  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [vaultPda] = PublicKey.findProgramAddressSync([VAULT_SEED], PROGRAM_ID);
  const [depositPda] = PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, seqBuf],
    PROGRAM_ID
  );
  const [withdrawPda] = PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, Buffer.from(iou.burn_txid_hex, "hex")],
    PROGRAM_ID
  );

  const dest = new PublicKey(Buffer.from(iou.dest_hex, "hex"));
  const attestor2Path = path.join(__dirname, "attestor2-devnet.json");
  const extraSigners: Keypair[] = [];
  const remaining: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [
    { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: false },
  ];
  if (fs.existsSync(attestor2Path)) {
    const attestor2 = Keypair.fromSecretKey(
      Uint8Array.from(JSON.parse(fs.readFileSync(attestor2Path, "utf8")))
    );
    extraSigners.push(attestor2);
    remaining.push({ pubkey: attestor2.publicKey, isSigner: true, isWritable: false });
  }

  console.log("iou_id", iou.iou_id_hex);
  console.log("burn_txid", iou.burn_txid_hex);
  console.log("deposit_seq", depositSeq);
  console.log("dest", dest.toBase58());

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
    .remainingAccounts(remaining)
    .signers(extraSigners)
    .rpc();

  const summary = {
    withdraw_tx: sig,
    iou_id: iou.iou_id_hex,
    burn_txid: iou.burn_txid_hex,
    explorer: `https://explorer.solana.com/tx/${sig}?cluster=devnet`,
  };
  fs.writeFileSync(path.join(DATA, "e2e_iou_withdraw_result.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
