/**
 * Drain data/air_acks.jsonl into post_air_ack.
 * Radio process only logs; this script submits.
 *
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *   ANCHOR_WALLET=~/.config/solana/id.json \
 *   ACK_LOG=../../data/air_acks.jsonl \
 *   MESH_SHORT_ID=aabbccdd11223344 \
 *   npx ts-node scripts/post_air_acks.ts
 *
 * Signer must be BridgeConfig.attestors[validator_index].
 * Refuses any cluster that is not devnet genesis.
 */
import * as fs from "fs";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");
const CONFIG_SEED = Buffer.from("mesh-bridge-config");
const NODE_SEED = Buffer.from("mesh-relay-node");
const ACK_SEED = Buffer.from("mesh-relay-ack");
const SCORE_SEED = Buffer.from("mesh-relay-score");

function hexToBuf(hex: string): Buffer {
  return Buffer.from(hex.replace(/^0x/, ""), "hex");
}

async function main() {
  const logPath = process.env.ACK_LOG || "data/air_acks.jsonl";
  const shortHex = process.env.MESH_SHORT_ID;
  if (!shortHex || shortHex.length !== 16) {
    throw new Error("MESH_SHORT_ID must be 8-byte hex (16 chars)");
  }
  const meshShort = hexToBuf(shortHex);

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const genesis = await provider.connection.getGenesisHash();
  if (genesis !== "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG") {
    throw new Error(`refusing to post acks off devnet (genesis=${genesis})`);
  }

  const idl = require("../target/idl/programs_mesh_bridge.json");
  const program = new Program(idl as anchor.Idl, provider);
  const wallet = provider.wallet.publicKey;

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [nodePda] = PublicKey.findProgramAddressSync([NODE_SEED, meshShort], PROGRAM_ID);
  const [scorePda] = PublicKey.findProgramAddressSync([SCORE_SEED, wallet.toBuffer()], PROGRAM_ID);

  if (!fs.existsSync(logPath)) {
    console.log("no ack log", logPath);
    return;
  }

  const lines = fs.readFileSync(logPath, "utf8").trim().split("\n").filter(Boolean);
  for (const line of lines) {
    const row = JSON.parse(line);
    const height = BigInt(row.height);
    const heightBuf = Buffer.alloc(8);
    heightBuf.writeBigUInt64LE(height);
    const hash = hexToBuf(row.block_hash_hex);
    const vidx: number = row.validator_index;
    const [ackPda] = PublicKey.findProgramAddressSync(
      [ACK_SEED, heightBuf, Buffer.from([vidx])],
      PROGRAM_ID
    );

    const existing = await provider.connection.getAccountInfo(ackPda);
    if (existing) {
      console.log("skip existing ack", row.height, vidx);
      continue;
    }

    const sig = await program.methods
      .postAirAck(new anchor.BN(row.height.toString()), Array.from(hash), vidx)
      .accounts({
        validator: wallet,
        config: configPda,
        ack: ackPda,
        node: nodePda,
        score: scorePda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("posted", row.height, "v" + vidx, sig);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
