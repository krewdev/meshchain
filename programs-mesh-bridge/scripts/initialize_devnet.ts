/**
 * Initialize MeshChain hybrid vault on Solana devnet.
 * Run from programs-mesh-bridge:
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=~/.config/solana/id.json \
 *   npx ts-node scripts/initialize_devnet.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import idl from "../target/idl/programs_mesh_bridge.json";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const VAULT_SEED = Buffer.from("mesh-bridge-vault");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = new Program(idl as anchor.Idl, provider);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [vaultPda] = PublicKey.findProgramAddressSync([VAULT_SEED], PROGRAM_ID);

  console.log("wallet", provider.wallet.publicKey.toBase58());
  console.log("program", PROGRAM_ID.toBase58());
  console.log("config", configPda.toBase58());
  console.log("vault", vaultPda.toBase58());

  const feeBps = 30;
  const withdrawFeeBps = 30;
  const minAttestations = 2;
  const hybridEnabled = true;

  try {
    const existing = await (program.account as any).bridgeConfig.fetch(configPda);
    console.log("config already exists", {
      hybrid: existing.hybridEnabled,
      minAttestations: existing.minAttestations,
      authority: existing.authority.toBase58(),
    });
  } catch {
    console.log("initializing config…");
    const sig = await program.methods
      .initialize(feeBps, withdrawFeeBps, minAttestations, hybridEnabled)
      .accounts({
        authority: provider.wallet.publicKey,
        config: configPda,
        solVault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("initialize tx", sig);
  }

  const attestor0 = provider.wallet.publicKey;
  const secondPath = path.join(__dirname, "attestor2-devnet.json");
  let attestor1: PublicKey;
  if (fs.existsSync(secondPath)) {
    const secret = Uint8Array.from(JSON.parse(fs.readFileSync(secondPath, "utf8")));
    attestor1 = Keypair.fromSecretKey(secret).publicKey;
  } else {
    const kp = Keypair.generate();
    fs.writeFileSync(secondPath, JSON.stringify(Array.from(kp.secretKey)));
    attestor1 = kp.publicKey;
    console.log("wrote attestor2 keypair (testnet only):", secondPath);
  }
  console.log("attestors", attestor0.toBase58(), attestor1.toBase58());

  const sig2 = await program.methods
    .setAttestors([attestor0, attestor1])
    .accounts({
      authority: provider.wallet.publicKey,
      config: configPda,
    })
    .rpc();
  console.log("setAttestors tx", sig2);

  const cfg = await (program.account as any).bridgeConfig.fetch(configPda);
  console.log("final config", {
    hybridEnabled: cfg.hybridEnabled,
    minAttestations: cfg.minAttestations,
    attestorCount: cfg.attestorCount,
    feeBps: cfg.feeBps,
  });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
