//! In-process multi-validator simulation (mesh transport = memory).

use crate::consensus::{leader_index, produce_block, FinalityTracker};
use anyhow::{bail, Context, Result};
use meshchain_ledger::genesis::GenesisConfig;
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{short_id, short_id_hex};
use meshchain_proto::crypto::Keypair;
use meshchain_proto::tx::{Tx, TxBody};
use std::fs;
use std::path::Path;

pub fn run_sim(data_dir: &Path, n_transfers: u32, mut slot_time: u64) -> Result<()> {
    let genesis_path = data_dir.join("genesis.json");
    let genesis: GenesisConfig = serde_json::from_str(
        &fs::read_to_string(&genesis_path)
            .with_context(|| format!("missing genesis — run init first: {}", genesis_path.display()))?,
    )?;

    let mut state = ChainState::from_genesis(&genesis)?;
    let n = state.validators.len();
    println!(
        "sim start chain={} validators={} finality_threshold={}",
        state.chain_id,
        n,
        FinalityTracker::threshold(n)
    );

    // Load validator keys
    let mut validators: Vec<Keypair> = Vec::new();
    for i in 0..n {
        let path = data_dir.join("keys").join(format!("validator-{i}.json"));
        let file: meshchain_proto::crypto::KeypairFile =
            serde_json::from_str(&fs::read_to_string(&path)?)?;
        let kp = Keypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // order must match genesis validators
        let expected = hex::decode(&genesis.validators[i])?;
        if kp.public_key().as_slice() != expected.as_slice() {
            bail!("validator-{i} key mismatch with genesis");
        }
        validators.push(kp);
    }

    let alice_file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(data_dir.join("keys/alice.json"))?)?;
    let bob_file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(data_dir.join("keys/bob.json"))?)?;
    let alice = Keypair::from_file(&alice_file).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let bob = Keypair::from_file(&bob_file).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut finality = FinalityTracker::new();

    // Genesis block
    {
        let idx = leader_index(0, n);
        let block = produce_block(&state, &validators[idx as usize], idx, slot_time, vec![])?;
        commit_with_finality(&mut state, &mut finality, &block, &validators)?;
        println!(
            "height={} genesis tip={} supply={}",
            state.height,
            hex::encode(state.tip_hash),
            state.total_supply
        );
    }

    // REGISTER alice & bob (if needed) — already in genesis with balances
    // Transfers alice -> bob
    let alice_sid = short_id(&alice.public_key());
    let bob_sid = short_id(&bob.public_key());
    println!(
        "alice {} bal={}",
        short_id_hex(&alice_sid),
        state.balance_of(&alice_sid)
    );
    println!(
        "bob   {} bal={}",
        short_id_hex(&bob_sid),
        state.balance_of(&bob_sid)
    );

    for i in 0..n_transfers {
        slot_time += genesis.slot_secs;
        let alice_acc = state
            .account(&alice_sid)
            .context("alice account missing")?
            .clone();
        let amount = 1_000_000; // 1 MESH
        let body = TxBody::Transfer {
            nonce: alice_acc.nonce,
            from: alice_sid,
            to: bob_sid,
            amount,
            fee: 0,
        };
        let tx = Tx::sign(body, &alice).map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let next_height = state.height + 1;
        let idx = leader_index(next_height, n);
        let block = produce_block(
            &state,
            &validators[idx as usize],
            idx,
            slot_time,
            vec![tx],
        )?;
        commit_with_finality(&mut state, &mut finality, &block, &validators)?;
        println!(
            "transfer #{i}: +1 MESH alice->bob | height={} alice={} bob={} supply={}",
            state.height,
            state.balance_of(&alice_sid),
            state.balance_of(&bob_sid),
            state.total_supply
        );
    }

    // Double-spend attempt (same nonce) must fail
    {
        slot_time += genesis.slot_secs;
        let body = TxBody::Transfer {
            nonce: 0, // already used if transfers > 0
            from: alice_sid,
            to: bob_sid,
            amount: 1,
            fee: 0,
        };
        let tx = Tx::sign(body, &alice).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let next_height = state.height + 1;
        let idx = leader_index(next_height, n);
        let block = produce_block(
            &state,
            &validators[idx as usize],
            idx,
            slot_time,
            vec![tx],
        )?;
        match state.apply_block(&block) {
            Ok(_) => bail!("double-spend should have failed"),
            Err(e) => println!("double-spend correctly rejected: {e}"),
        }
    }

    // Overspend attempt
    {
        slot_time += genesis.slot_secs;
        let alice_acc = state.account(&alice_sid).unwrap().clone();
        let body = TxBody::Transfer {
            nonce: alice_acc.nonce,
            from: alice_sid,
            to: bob_sid,
            amount: alice_acc.balance.saturating_add(1),
            fee: 0,
        };
        let tx = Tx::sign(body, &alice).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let next_height = state.height + 1;
        let idx = leader_index(next_height, n);
        let block = produce_block(
            &state,
            &validators[idx as usize],
            idx,
            slot_time,
            vec![tx],
        )?;
        match state.apply_block(&block) {
            Ok(_) => bail!("overspend should have failed"),
            Err(e) => println!("overspend correctly rejected: {e}"),
        }
    }

    // Bridge-shaped MINT (validator as minter) then BURN
    {
        slot_time += genesis.slot_secs;
        let minter = &validators[0];
        let minter_sid = short_id(&minter.public_key());
        let minter_nonce = state.account(&minter_sid).unwrap().nonce;
        let mint_amount = 50 * 1_000_000;
        let body = TxBody::Mint {
            nonce: minter_nonce,
            to: bob_sid,
            amount: mint_amount,
            external_ref: [7u8; 16], // stands in for Solana vault deposit tx hash
        };
        let tx = Tx::sign(body, minter).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let next_height = state.height + 1;
        let idx = leader_index(next_height, n);
        let block = produce_block(
            &state,
            &validators[idx as usize],
            idx,
            slot_time,
            vec![tx],
        )?;
        commit_with_finality(&mut state, &mut finality, &block, &validators)?;
        println!(
            "bridge MINT 50 MESH -> bob (external_ref=vault deposit) bob={}",
            state.balance_of(&bob_sid)
        );
    }
    {
        use meshchain_proto::pq::PqKeypair;
        use meshchain_proto::privacy::redeem_hint;
        slot_time += genesis.slot_secs;
        let bob_acc = state.account(&bob_sid).unwrap().clone();
        let burn_amount = 10 * 1_000_000;
        // Privacy: only a hash of the destination goes on the mesh
        let redeem = redeem_hint(b"sol", b"DemoDestinationPubkey");
        let body = TxBody::Burn {
            nonce: bob_acc.nonce,
            from: bob_sid,
            amount: burn_amount,
            redeem_hint: redeem,
            asset_id: 1, // SOL-claim — vault-linked ⇒ always needs cold PQ key
        };
        let bob_pq = PqKeypair::generate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let tx = Tx::sign_with_pq(body, &bob, &bob_pq)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let next_height = state.height + 1;
        let idx = leader_index(next_height, n);
        let block = produce_block(
            &state,
            &validators[idx as usize],
            idx,
            slot_time,
            vec![tx],
        )?;
        commit_with_finality(&mut state, &mut finality, &block, &validators)?;
        println!(
            "bridge BURN 10 MESH from bob (hybrid off-ramp, PQ+hashed dest) bob={} supply={}",
            state.balance_of(&bob_sid),
            state.total_supply
        );
    }

    // Large transfer without PQ must fail; with PQ must succeed
    {
        use meshchain_proto::pq::PqKeypair;
        slot_time += genesis.slot_secs;
        let alice_acc = state.account(&alice_sid).unwrap().clone();
        let big = state.pq_required_above; // at threshold
        if alice_acc.balance >= big {
            let body = TxBody::Transfer {
                nonce: alice_acc.nonce,
                from: alice_sid,
                to: bob_sid,
                amount: big,
                fee: 0,
            };
            let bad = Tx::sign(body.clone(), &alice).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let next_height = state.height + 1;
            let idx = leader_index(next_height, n);
            let block = produce_block(
                &state,
                &validators[idx as usize],
                idx,
                slot_time,
                vec![bad],
            )?;
            match state.apply_block(&block) {
                Ok(_) => bail!("large transfer without cold key should fail"),
                Err(e) => println!("large send without cold key correctly rejected: {e}"),
            }

            slot_time += genesis.slot_secs;
            let pq = PqKeypair::generate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let good =
                Tx::sign_with_pq(body, &alice, &pq).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let next_height = state.height + 1;
            let idx = leader_index(next_height, n);
            let block = produce_block(
                &state,
                &validators[idx as usize],
                idx,
                slot_time,
                vec![good],
            )?;
            commit_with_finality(&mut state, &mut finality, &block, &validators)?;
            println!(
                "large send WITH cold key OK alice={} bob={}",
                state.balance_of(&alice_sid),
                state.balance_of(&bob_sid)
            );
        }
    }

    let state_path = data_dir.join("chain_state.json");
    state.save_json(&state_path)?;
    println!("saved {}", state_path.display());
    println!(
        "SIM OK — final height={} alice={} bob={} supply={}",
        state.height,
        state.balance_of(&alice_sid),
        state.balance_of(&bob_sid),
        state.total_supply
    );
    Ok(())
}

/// Extreme cold-storage demo: ML-DSA-65 redeem auth fragmented over sim mesh.
pub fn run_pq_cold_demo(data_dir: &Path) -> Result<()> {
    use meshchain_proto::pq::{PqKeypair, PqSigned};
    use meshchain_transport::{
        decode_frame, fragment_bytes, session_id_from_hash, FragAssembler, SimTransport,
    };

    fs::create_dir_all(data_dir.join("keys"))?;
    let cold = PqKeypair::generate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let path = data_dir.join("keys/cold_pq.json");
    fs::write(&path, serde_json::to_string_pretty(&cold.to_file())?)?;
    println!(
        "generated offline PQ cold key short={}",
        hex::encode(cold.short_id())
    );
    println!("stored {}", path.display());
    println!("scheme=ml-dsa-65 — keep this file air-gapped (no internet/5G host)");

    // Message that would authorize vault release after mesh burn
    let redeem_msg = format!(
        "MESHCHAIN-REDEEM|asset=SOL|amount=1000000000|dest=ExampleSolAddr|height=42|short={}",
        hex::encode(cold.short_id())
    );
    let env = PqSigned::sign_message(redeem_msg.as_bytes(), &cold)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    env.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let encoded = env.encode().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let sid = session_id_from_hash(&encoded);
    let frames = fragment_bytes(sid, &encoded).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    println!(
        "PQ redeem envelope {} bytes → {} LoRa frames (sim mesh)",
        encoded.len(),
        frames.len()
    );

    let peers = SimTransport::new_network(2);
    let cold_radio = &peers[0];
    let gateway = &peers[1];
    for f in &frames {
        cold_radio.broadcast(f);
    }

    let mut asm = FragAssembler::new();
    let mut assembled = None;
    let mut n = 0;
    while let Some(raw) = gateway.try_recv() {
        n += 1;
        let frame = decode_frame(&raw).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if let Some(done) = asm
            .push_frame(&frame)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
        {
            assembled = Some(done);
        }
    }
    let assembled = assembled.context("failed to reassemble PQ envelope on gateway")?;
    let env2 = PqSigned::decode(&assembled).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    env2.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    println!("gateway reassembled {n} frames; ML-DSA-65 verify OK");
    println!("redeem_msg={}", redeem_msg);
    println!("PQ COLD DEMO OK — radio can power off; keys never needed internet");
    Ok(())
}

fn commit_with_finality(
    state: &mut ChainState,
    finality: &mut FinalityTracker,
    block: &meshchain_proto::block::Block,
    validators: &[Keypair],
) -> Result<()> {
    // Producer implicit ACK by producing; all validators ACK in sim (honest)
    let hash_hex = block.hash_hex();
    for v in validators {
        finality.ack(&hash_hex, v.public_key());
    }
    let n = validators.len();
    if !finality.is_final(&hash_hex, n) {
        bail!(
            "block not final acks={} need={}",
            finality.ack_count(&hash_hex),
            FinalityTracker::threshold(n)
        );
    }
    state.apply_block(block)?;
    Ok(())
}
