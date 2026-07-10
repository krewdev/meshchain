import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ProgramsMeshBridge } from "../target/types/programs_mesh_bridge";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import { assert } from "chai";

describe("mesh_bridge vault", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .ProgramsMeshBridge as Program<ProgramsMeshBridge>;

  const CONFIG_SEED = Buffer.from("mesh-bridge-config");
  const VAULT_SEED = Buffer.from("mesh-bridge-vault");
  const DEPOSIT_SEED = Buffer.from("mesh-bridge-deposit");
  const WITHDRAW_SEED = Buffer.from("mesh-bridge-withdraw");

  let configPda: PublicKey;
  let vaultPda: PublicKey;

  const meshShortId = Buffer.from("aabbccdd", "hex"); // 4 bytes wrong - need 8
  const meshShort = Uint8Array.from([0xaa, 0xbb, 0xcc, 0xdd, 0x11, 0x22, 0x33, 0x44]);

  before(async () => {
    [configPda] = PublicKey.findProgramAddressSync(
      [CONFIG_SEED],
      program.programId
    );
    [vaultPda] = PublicKey.findProgramAddressSync(
      [VAULT_SEED],
      program.programId
    );
  });

  it("initializes bridge", async () => {
    const feeBps = 30; // 0.30%
    const withdrawFeeBps = 30;

    // hybrid on, need 2 mesh attestors for production-like tests
    const attestorA = Keypair.generate();
    const attestorB = Keypair.generate();
    for (const k of [attestorA, attestorB]) {
      const sig = await provider.connection.requestAirdrop(
        k.publicKey,
        LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig);
    }

    await program.methods
      .initialize(feeBps, withdrawFeeBps, 2, true)
      .accounts({
        authority: provider.wallet.publicKey,
        config: configPda,
        solVault: vaultPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await program.methods
      .setAttestors([attestorA.publicKey, attestorB.publicKey])
      .accounts({
        authority: provider.wallet.publicKey,
        config: configPda,
      })
      .rpc();

    const config = await program.account.bridgeConfig.fetch(configPda);
    assert.equal(config.feeBps, feeBps);
    assert.equal(config.hybridEnabled, true);
    assert.equal(config.minAttestations, 2);
    assert.equal(config.paused, false);

    // stash on (global as any) for later tests
    (global as any).__meshAttestors = [attestorA, attestorB];
  });

  it("deposits SOL and records net after fee", async () => {
    const amount = new anchor.BN(1 * LAMPORTS_PER_SOL);
    const configBefore = await program.account.bridgeConfig.fetch(configPda);
    const seq = configBefore.depositCount;
    const seqBuf = Buffer.alloc(8);
    seqBuf.writeBigUInt64LE(BigInt(seq.toString()));

    const [depositPda] = PublicKey.findProgramAddressSync(
      [DEPOSIT_SEED, seqBuf],
      program.programId
    );

    await program.methods
      .depositSol(amount, Array.from(meshShort) as number[])
      .accounts({
        depositor: provider.wallet.publicKey,
        config: configPda,
        solVault: vaultPda,
        depositRecord: depositPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const dep = await program.account.depositRecord.fetch(depositPda);
    const expectedFee = Math.floor((1 * LAMPORTS_PER_SOL * 30) / 10_000);
    const expectedNet = 1 * LAMPORTS_PER_SOL - expectedFee;
    assert.equal(dep.amountGross.toNumber(), 1 * LAMPORTS_PER_SOL);
    assert.equal(dep.amountNet.toNumber(), expectedNet);
    assert.equal(dep.fee.toNumber(), expectedFee);
    assert.deepEqual(Array.from(dep.meshShortId), Array.from(meshShort));
  });

  it("hybrid unlock: mesh id + 2 attestors; internet alone fails", async () => {
    const attestors = (global as any).__meshAttestors as Keypair[];
    const burnTxid = new Uint8Array(32);
    burnTxid[0] = 0xbe;
    burnTxid[1] = 0xef;

    const dest = Keypair.generate();
    const sig = await provider.connection.requestAirdrop(
      dest.publicKey,
      LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig);

    const amount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);
    const [withdrawPda] = PublicKey.findProgramAddressSync(
      [WITHDRAW_SEED, Buffer.from(burnTxid)],
      program.programId
    );

    // Wrong mesh id must fail
    try {
      await program.methods
        .withdrawHybridSol(
          Array.from(burnTxid) as number[],
          amount,
          new anchor.BN(42),
          Array.from(Uint8Array.from([0, 0, 0, 0, 0, 0, 0, 1])) as number[]
        )
        .accounts({
          relayer: provider.wallet.publicKey,
          config: configPda,
          solVault: vaultPda,
          destination: dest.publicKey,
          depositRecord: PublicKey.findProgramAddressSync(
            [DEPOSIT_SEED, (() => { const b = Buffer.alloc(8); b.writeBigUInt64LE(0n); return b; })()],
            program.programId
          )[0],
          withdrawRecord: withdrawPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(
          attestors.map((a) => ({
            pubkey: a.publicKey,
            isSigner: true,
            isWritable: false,
          }))
        )
        .signers(attestors)
        .rpc();
      assert.fail("wrong mesh id should fail");
    } catch (e) {
      assert.ok(e);
    }

    // Relayer alone (no attestors) must fail under hybrid
    try {
      await program.methods
        .withdrawHybridSol(
          Array.from(burnTxid) as number[],
          amount,
          new anchor.BN(42),
          Array.from(meshShort) as number[]
        )
        .accounts({
          relayer: provider.wallet.publicKey,
          config: configPda,
          solVault: vaultPda,
          destination: dest.publicKey,
          depositRecord: PublicKey.findProgramAddressSync(
            [DEPOSIT_SEED, (() => { const b = Buffer.alloc(8); b.writeBigUInt64LE(0n); return b; })()],
            program.programId
          )[0],
          withdrawRecord: withdrawPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      assert.fail("relayer alone should fail");
    } catch (e) {
      assert.ok(e);
    }

    const depositPda = PublicKey.findProgramAddressSync(
      [DEPOSIT_SEED, (() => { const b = Buffer.alloc(8); b.writeBigUInt64LE(0n); return b; })()],
      program.programId
    )[0];

    const destBefore = await provider.connection.getBalance(dest.publicKey);
    await program.methods
      .withdrawHybridSol(
        Array.from(burnTxid) as number[],
        amount,
        new anchor.BN(42),
        Array.from(meshShort) as number[]
      )
      .accounts({
        relayer: provider.wallet.publicKey,
        config: configPda,
        solVault: vaultPda,
        destination: dest.publicKey,
        depositRecord: depositPda,
        withdrawRecord: withdrawPda,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(
        attestors.map((a) => ({
          pubkey: a.publicKey,
          isSigner: true,
          isWritable: false,
        }))
      )
      .signers(attestors)
      .rpc();

    const destAfter = await provider.connection.getBalance(dest.publicKey);
    const fee = Math.floor((0.5 * LAMPORTS_PER_SOL * 30) / 10_000);
    const out = 0.5 * LAMPORTS_PER_SOL - fee;
    assert.equal(destAfter - destBefore, out);
  });

  it("rejects double unlock same burn_txid", async () => {
    const attestors = (global as any).__meshAttestors as Keypair[];
    const burnTxid = new Uint8Array(32);
    burnTxid[0] = 0xbe;
    burnTxid[1] = 0xef;
    const dest = Keypair.generate();
    const amount = new anchor.BN(0.1 * LAMPORTS_PER_SOL);
    const [withdrawPda] = PublicKey.findProgramAddressSync(
      [WITHDRAW_SEED, Buffer.from(burnTxid)],
      program.programId
    );
    const depositPda = PublicKey.findProgramAddressSync(
      [DEPOSIT_SEED, (() => { const b = Buffer.alloc(8); b.writeBigUInt64LE(0n); return b; })()],
      program.programId
    )[0];

    try {
      await program.methods
        .withdrawHybridSol(
          Array.from(burnTxid) as number[],
          amount,
          new anchor.BN(43),
          Array.from(meshShort) as number[]
        )
        .accounts({
          relayer: provider.wallet.publicKey,
          config: configPda,
          solVault: vaultPda,
          destination: dest.publicKey,
          depositRecord: depositPda,
          withdrawRecord: withdrawPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(
          attestors.map((a) => ({
            pubkey: a.publicKey,
            isSigner: true,
            isWritable: false,
          }))
        )
        .signers(attestors)
        .rpc();
      assert.fail("should have failed");
    } catch (e) {
      assert.ok(e);
    }
  });
});
