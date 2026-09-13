/**
 * Path A: initialize RelayConfig on DEVNET only.
 * emissions_enabled is forced false by the program.
 *
 *   cd programs-mesh-bridge
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=~/.config/solana/id.json \
 *   npx ts-node scripts/init_relay_devnet.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const RELAY_CONFIG_SEED = Buffer.from("mesh-relay-config");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const genesis = await provider.connection.getGenesisHash();
  if (genesis !== "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG") {
    throw new Error(`refusing to init relay config off devnet (genesis=${genesis})`);
  }

  const idl = require("../target/idl/programs_mesh_bridge.json");
  const program = new Program(idl as anchor.Idl, provider);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [relayPda] = PublicKey.findProgramAddressSync([RELAY_CONFIG_SEED], PROGRAM_ID);

  console.log("wallet", provider.wallet.publicKey.toBase58());
  console.log("relayConfig", relayPda.toBase58());

  const relayBps = 7000;
  const emissionsEnabled = false;

  const sig = await program.methods
    .initRelayConfig(relayBps, emissionsEnabled)
    .accounts({
      authority: provider.wallet.publicKey,
      config: configPda,
      relayConfig: relayPda,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log("init_relay_config", sig);

  const rc = await (program.account as any).relayConfig.fetch(relayPda);
  console.log("relay config", {
    emissionsEnabled: rc.emissionsEnabled,
    relayBpsOfWithdrawFee: rc.relayBpsOfWithdrawFee,
    relayMint: rc.relayMint.toBase58(),
  });
  if (rc.emissionsEnabled) {
    throw new Error("emissions_enabled must be false on Path A");
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
