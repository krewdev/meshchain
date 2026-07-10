/**
 * E2E cash-out: mesh burn (already done) → hybrid withdraw on Solana devnet.
 * Reads data/last_burn.json and deposit seq 0 (or --seq).
 */
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

const PROGRAM_ID = new PublicKey(
  "CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx"
);
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const VAULT_SEED = Buffer.from("mesh-bridge-vault");
const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");
const WITHDRAW_SEED = Buffer.from("mesh-bridge-withdraw");

const ROOT = path.resolve(__dirname, "../..");
const DATA = path.join(ROOT, "data");
const IDL_PATH = path.join(
  ROOT,
  "programs-mesh-bridge/target/idl/programs_mesh_bridge.json"
);

async function main() {
  const burn = JSON.parse(
    fs.readFileSync(path.join(DATA, "last_burn.json"), "utf8")
  );
  const burnTxid = Array.from(Buffer.from(burn.burn_txid_hex, "hex"));
  const amount = new anchor.BN(burn.amount);
  const meshHeight = new anchor.BN(burn.mesh_height);
  const meshShort = Array.from(Buffer.from(burn.mesh_short_id_hex, "hex"));

  // deposit seq 0 from first e2e deposit (adjust if needed)
  const depositSeq = 0;
  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(BigInt(depositSeq));

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = JSON.parse(fs.readFileSync(IDL_PATH, "utf8"));
  const program = new Program(idl, provider);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [vaultPda] = PublicKey.findProgramAddressSync([VAULT_SEED], PROGRAM_ID);
  const [depositPda] = PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, seqBuf],
    PROGRAM_ID
  );
  const [withdrawPda] = PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, Buffer.from(burn.burn_txid_hex, "hex")],
    PROGRAM_ID
  );

  // Attestors: authority + attestor2
  const attestor2Path = path.join(__dirname, "attestor2-devnet.json");
  const secret = Uint8Array.from(JSON.parse(fs.readFileSync(attestor2Path, "utf8")));
  const attestor2 = Keypair.fromSecretKey(secret);

  const dest = provider.wallet.publicKey; // withdraw back to same wallet

  console.log("=== Hybrid cash-out ===");
  console.log("burn_txid", burn.burn_txid_hex);
  console.log("amount", amount.toString());
  console.log("mesh_short", burn.mesh_short_id_hex);
  console.log("deposit", depositPda.toBase58());

  const dep = await (program.account as any).depositRecord.fetch(depositPda);
  console.log("deposit mesh_short", Buffer.from(dep.meshShortId).toString("hex"));
  console.log("deposit remaining net", dep.amountNet.toString(), "unlocked", dep.amountUnlocked.toString());

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
  console.log("withdraw tx", sig);
  console.log("https://explorer.solana.com/tx/" + sig + "?cluster=devnet");
  console.log("destination SOL delta lamports:", balAfter - balBefore);

  const summary = {
    withdraw_tx: sig,
    burn_txid: burn.burn_txid_hex,
    amount_base: amount.toString(),
    sol_delta_lamports: balAfter - balBefore,
    explorer: `https://explorer.solana.com/tx/${sig}?cluster=devnet`,
  };
  fs.writeFileSync(
    path.join(DATA, "e2e_cashout_result.json"),
    JSON.stringify(summary, null, 2)
  );
  console.log(JSON.stringify(summary, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
