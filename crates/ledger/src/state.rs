use crate::error::LedgerError;
use crate::genesis::GenesisConfig;
use meshchain_proto::address::{short_id, short_id_hex, Address, ShortId};
use meshchain_proto::block::Block;
use meshchain_proto::crypto::PublicKey;
use meshchain_proto::tx::{Tx, TxBody};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub pubkey: Address,
    pub balance: u64,
    pub nonce: u32,
    /// Bound ML-DSA-65 public key (first large PQ spend locks it to this account).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pq_pk: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedBlock {
    pub height: u64,
    pub hash_hex: String,
    pub tx_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainState {
    pub chain_id: String,
    pub height: u64,
    pub tip_hash: [u8; 32],
    pub block_reward: u64,
    pub slot_secs: u64,
    pub validators: Vec<PublicKey>,
    pub minters: HashSet<PublicKey>,
    pub accounts: HashMap<String, Account>, // key = short_id hex
    pub total_supply: u64,
    pub applied: Vec<AppliedBlock>,
    /// Amounts at or above this require PQ signatures on Transfer/Burn.
    #[serde(default = "default_pq_threshold_state")]
    pub pq_required_above: u64,
    /// Hex-encoded mint external_ref values already consumed (bridge deposit uniqueness).
    #[serde(default)]
    pub used_external_refs: HashSet<String>,
}

fn default_pq_threshold_state() -> u64 {
    crate::genesis::ONE_MESH.saturating_mul(100)
}

impl ChainState {
    pub fn from_genesis(genesis: &GenesisConfig) -> Result<Self, LedgerError> {
        let validators = genesis
            .validator_keys()
            .map_err(|e| LedgerError::State(e))?;
        if validators.is_empty() {
            return Err(LedgerError::State("at least one validator required".into()));
        }
        let minters = genesis.minter_set().map_err(|e| LedgerError::State(e))?;
        let initial = genesis
            .initial_accounts()
            .map_err(|e| LedgerError::State(e))?;

        let mut accounts = HashMap::new();
        let mut total_supply = 0u64;
        for (sid, (pk, bal)) in initial {
            total_supply = total_supply.saturating_add(bal);
            accounts.insert(
                short_id_hex(&sid),
                Account {
                    pubkey: pk,
                    balance: bal,
                    nonce: 0,
                    pq_pk: None,
                },
            );
        }

        // Ensure validators have accounts (0 balance ok)
        for pk in &validators {
            let sid = short_id(pk);
            let key = short_id_hex(&sid);
            accounts.entry(key).or_insert(Account {
                pubkey: *pk,
                balance: 0,
                nonce: 0,
                pq_pk: None,
            });
        }

        Ok(Self {
            chain_id: genesis.chain_id.clone(),
            height: 0,
            tip_hash: [0u8; 32],
            block_reward: genesis.block_reward,
            slot_secs: genesis.slot_secs,
            validators,
            minters,
            accounts,
            total_supply,
            applied: vec![],
            pq_required_above: genesis.pq_required_above,
            used_external_refs: HashSet::new(),
        })
    }

    pub fn account(&self, sid: &ShortId) -> Option<&Account> {
        self.accounts.get(&short_id_hex(sid))
    }

    pub fn account_mut(&mut self, sid: &ShortId) -> Option<&mut Account> {
        self.accounts.get_mut(&short_id_hex(sid))
    }

    pub fn balance_of(&self, sid: &ShortId) -> u64 {
        self.account(sid).map(|a| a.balance).unwrap_or(0)
    }

    pub fn ensure_account(&mut self, pk: &PublicKey) -> ShortId {
        let sid = short_id(pk);
        let key = short_id_hex(&sid);
        self.accounts.entry(key).or_insert(Account {
            pubkey: *pk,
            balance: 0,
            nonce: 0,
            pq_pk: None,
        });
        sid
    }

    pub fn validator_index(&self, pk: &PublicKey) -> Option<u8> {
        self.validators
            .iter()
            .position(|v| v == pk)
            .map(|i| i as u8)
    }

    /// Fail-secure: require ML-DSA-65 when amount is large, when burning vault-linked assets,
    /// OR when the account has already bound a PQ key (preventing small classical drains).
    fn enforce_pq_policy(
        &mut self,
        tx: &Tx,
        from: &ShortId,
        amount: u64,
        vault_linked: bool,
    ) -> Result<(), LedgerError> {
        let already_bound = self.account(from).and_then(|a| a.pq_pk.as_ref()).is_some();
        let need_pq = vault_linked || amount >= self.pq_required_above || already_bound;
        if !need_pq {
            return Ok(());
        }
        if !tx.has_pq() {
            return Err(LedgerError::PqRequired);
        }
        tx.verify_pq()
            .map_err(|e| LedgerError::Proto(e.to_string()))?;
        let pk = tx.pq_pk.as_ref().unwrap();
        let acc = self
            .account_mut(from)
            .ok_or_else(|| LedgerError::AccountNotFound(short_id_hex(from)))?;
        match &acc.pq_pk {
            None => {
                // First cold spend binds this PQ key forever to the account.
                acc.pq_pk = Some(pk.clone());
            }
            Some(bound) if bound == pk => {}
            Some(_) => return Err(LedgerError::PqKeyMismatch),
        }
        Ok(())
    }

    /// Apply a verified tx to state (mutates). Does not check block context.
    pub fn apply_tx(&mut self, tx: &Tx) -> Result<(), LedgerError> {
        tx.verify().map_err(|e| LedgerError::Proto(e.to_string()))?;

        match &tx.body {
            TxBody::Register { nonce, pubkey } => {
                let sid = short_id(pubkey);
                let key = short_id_hex(&sid);
                if let Some(existing) = self.accounts.get(&key) {
                    if existing.pubkey != *pubkey {
                        return Err(LedgerError::ShortIdCollision);
                    }
                    // idempotent if same
                    if existing.nonce != 0 || *nonce != 0 {
                        return Err(LedgerError::AlreadyRegistered);
                    }
                }
                let acc = self.accounts.entry(key).or_insert(Account {
                    pubkey: *pubkey,
                    balance: 0,
                    nonce: 0,
                    pq_pk: None,
                });
                if acc.nonce != *nonce {
                    return Err(LedgerError::InvalidNonce {
                        expected: acc.nonce,
                        got: *nonce,
                    });
                }
                acc.nonce = acc.nonce.saturating_add(1);
            }
            TxBody::Transfer {
                nonce,
                from,
                to,
                amount,
                fee,
            } => {
                // Debit amount + priority fee (fee paid to block producer in apply_block).
                let total = amount
                    .checked_add(*fee)
                    .ok_or_else(|| LedgerError::State("amount+fee overflow".into()))?;
                // Balance check before PQ so users see "not enough" first.
                {
                    let from_acc = self
                        .account(from)
                        .ok_or_else(|| LedgerError::AccountNotFound(short_id_hex(from)))?;
                    if from_acc.balance < total {
                        return Err(LedgerError::InsufficientBalance);
                    }
                    if from_acc.nonce != *nonce {
                        return Err(LedgerError::InvalidNonce {
                            expected: from_acc.nonce,
                            got: *nonce,
                        });
                    }
                }
                // PQ policy is on transfer amount (value moved), not the tip.
                self.enforce_pq_policy(tx, from, *amount, false)?;
                // Debit from (amount + fee); fee is reassigned to producer after apply_tx.
                {
                    let from_acc = self
                        .account_mut(from)
                        .ok_or_else(|| LedgerError::AccountNotFound(short_id_hex(from)))?;
                    if from_acc.balance < total {
                        return Err(LedgerError::InsufficientBalance);
                    }
                    from_acc.balance -= total;
                    from_acc.nonce = from_acc.nonce.saturating_add(1);
                }
                // Credit to (create if needed with unknown pubkey placeholder — need full key)
                // For transfers to unregistered short ids, require recipient already known
                let to_key = short_id_hex(to);
                if !self.accounts.contains_key(&to_key) {
                    return Err(LedgerError::AccountNotFound(to_key));
                }
                let to_acc = self.accounts.get_mut(&to_key).unwrap();
                to_acc.balance = to_acc.balance.saturating_add(*amount);
            }
            TxBody::Mint {
                nonce,
                to,
                amount,
                external_ref,
                to_pubkey,
            } => {
                if !self.minters.contains(&tx.signer) {
                    return Err(LedgerError::UnauthorizedMinter);
                }
                let ref_hex = hex::encode(external_ref);
                if self.used_external_refs.contains(&ref_hex) {
                    return Err(LedgerError::DuplicateExternalRef);
                }
                // Minter nonce on minter account
                let minter_sid = short_id(&tx.signer);
                {
                    let minter = self
                        .account_mut(&minter_sid)
                        .ok_or_else(|| LedgerError::AccountNotFound(short_id_hex(&minter_sid)))?;
                    if minter.nonce != *nonce {
                        return Err(LedgerError::InvalidNonce {
                            expected: minter.nonce,
                            got: *nonce,
                        });
                    }
                    minter.nonce = minter.nonce.saturating_add(1);
                }
                let to_key = short_id_hex(to);
                if !self.accounts.contains_key(&to_key) {
                    // Peer-path faucet/bridge mints may create the recipient.
                    match to_pubkey {
                        Some(pk) if short_id(pk) == *to => {
                            self.ensure_account(pk);
                        }
                        Some(_) => {
                            return Err(LedgerError::State(
                                "mint to_pubkey does not match short id".into(),
                            ));
                        }
                        None => return Err(LedgerError::AccountNotFound(to_key)),
                    }
                } else if let Some(pk) = to_pubkey {
                    // Existing account: reject pubkey mismatch
                    let acc = self.accounts.get(&to_key).unwrap();
                    if acc.pubkey != *pk {
                        return Err(LedgerError::ShortIdCollision);
                    }
                }
                let to_acc = self.accounts.get_mut(&to_key).unwrap();
                to_acc.balance = to_acc.balance.saturating_add(*amount);
                self.total_supply = self.total_supply.saturating_add(*amount);
                self.used_external_refs.insert(ref_hex);
            }
            TxBody::Burn {
                nonce,
                from,
                amount,
                asset_id,
                redeem_hint,
                ..
            } => {
                // Vault-linked burns (SOL/BTC claims) always need cold PQ auth — fail secure.
                let vault_linked = *asset_id != 0;
                // redeem_hint must not be empty/all-zero (force hashed destination commitment)
                if *redeem_hint == [0u8; 32] {
                    return Err(LedgerError::State(
                        "burn needs a hashed redeem destination (privacy)".into(),
                    ));
                }
                {
                    let from_acc = self
                        .account(from)
                        .ok_or_else(|| LedgerError::AccountNotFound(short_id_hex(from)))?;
                    if from_acc.balance < *amount {
                        return Err(LedgerError::InsufficientBalance);
                    }
                    if from_acc.nonce != *nonce {
                        return Err(LedgerError::InvalidNonce {
                            expected: from_acc.nonce,
                            got: *nonce,
                        });
                    }
                }
                self.enforce_pq_policy(tx, from, *amount, vault_linked)?;
                let from_acc = self
                    .account_mut(from)
                    .ok_or_else(|| LedgerError::AccountNotFound(short_id_hex(from)))?;
                from_acc.balance -= amount;
                from_acc.nonce = from_acc.nonce.saturating_add(1);
                self.total_supply = self.total_supply.saturating_sub(*amount);
            }
        }
        Ok(())
    }

    /// Apply a block: verify linkage, producer, txs, pay block reward.
    pub fn apply_block(&mut self, block: &Block) -> Result<(), LedgerError> {
        block
            .verify_producer_sig()
            .map_err(|e| LedgerError::Proto(e.to_string()))?;

        // First applied block must be genesis height 0; then strictly increasing.
        let expected_height = if self.applied.is_empty() {
            0
        } else {
            self.height + 1
        };

        if block.header.height != expected_height {
            return Err(LedgerError::BadHeight {
                expected: expected_height,
                got: block.header.height,
            });
        }

        if block.header.height == 0 {
            if block.header.prev_hash != [0u8; 32] {
                return Err(LedgerError::BadPrevHash);
            }
        } else if block.header.prev_hash != self.tip_hash {
            return Err(LedgerError::BadPrevHash);
        }

        let prod_idx = self
            .validator_index(&block.header.producer)
            .ok_or(LedgerError::UnknownProducer)?;
        if prod_idx != block.header.producer_index {
            return Err(LedgerError::UnknownProducer);
        }
        // Enforce round-robin leader schedule: seat = height % N
        let n = self.validators.len();
        if n > 0 {
            let expected_leader = (block.header.height as usize % n) as u8;
            if block.header.producer_index != expected_leader {
                return Err(LedgerError::WrongLeader {
                    expected: expected_leader,
                });
            }
        }

        for tx in &block.txs {
            self.apply_tx(tx)?;
            // Priority fee (MEV tip) → block producer. Supply conserved (debited from sender above).
            let tip = tx.priority_fee();
            if tip > 0 {
                let prod_sid = short_id(&block.header.producer);
                self.ensure_account(&block.header.producer);
                let acc = self.account_mut(&prod_sid).unwrap();
                acc.balance = acc.balance.saturating_add(tip);
            }
        }

        // Block reward on non-genesis blocks
        let reward = self.block_reward;
        if block.header.height > 0 && reward > 0 {
            let sid = short_id(&block.header.producer);
            self.ensure_account(&block.header.producer);
            let acc = self.account_mut(&sid).unwrap();
            acc.balance = acc.balance.saturating_add(reward);
            self.total_supply = self.total_supply.saturating_add(reward);
        }

        let hash = block.hash();
        self.height = block.header.height;
        self.tip_hash = hash;
        self.applied.push(AppliedBlock {
            height: block.header.height,
            hash_hex: hex::encode(hash),
            tx_count: block.header.tx_count,
        });
        Ok(())
    }

    /// Dry-run: would this tx apply against current state? (sig + economic checks)
    pub fn can_apply_tx(&self, tx: &Tx) -> bool {
        let mut trial = self.clone();
        trial.apply_tx(tx).is_ok()
    }

    /// Atomic write: temp file in same dir then rename (crash-safe on POSIX).
    pub fn save_json(&self, path: &Path) -> Result<(), LedgerError> {
        let s = serde_json::to_string_pretty(self).map_err(|e| LedgerError::Io(e.to_string()))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| LedgerError::Io(e.to_string()))?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, s.as_bytes()).map_err(|e| LedgerError::Io(e.to_string()))?;
        fs::rename(&tmp, path).map_err(|e| LedgerError::Io(e.to_string()))
    }

    pub fn load_json(path: &Path) -> Result<Self, LedgerError> {
        let s = fs::read_to_string(path).map_err(|e| LedgerError::Io(e.to_string()))?;
        serde_json::from_str(&s).map_err(|e| LedgerError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::{GenesisAccount, GenesisConfig, ONE_MESH};
    use meshchain_proto::address::short_id;
    use meshchain_proto::crypto::Keypair;
    use meshchain_proto::pq::PqKeypair;
    use meshchain_proto::tx::{Tx, TxBody};

    #[test]
    fn transfer_priority_fee_goes_to_producer() {
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let producer = Keypair::generate();
        let genesis = GenesisConfig {
            chain_id: "test".into(),
            validators: vec![hex::encode(producer.public_key())],
            block_reward: 0,
            allocations: vec![
                GenesisAccount {
                    public_key_hex: hex::encode(alice.public_key()),
                    balance: 100 * ONE_MESH,
                },
                GenesisAccount {
                    public_key_hex: hex::encode(bob.public_key()),
                    balance: 0,
                },
            ],
            minters: vec![],
            slot_secs: 1,
            pq_required_above: 1000 * ONE_MESH,
            protocol_version: 1,
        };
        let mut st = ChainState::from_genesis(&genesis).unwrap();
        // genesis block height 0
        let gblock =
            meshchain_proto::block::Block::new(0, [0u8; 32], 1, 0, &producer, vec![]).unwrap();
        st.apply_block(&gblock).unwrap();

        let from = short_id(&alice.public_key());
        let to = short_id(&bob.public_key());
        let amount = 10 * ONE_MESH;
        let fee = ONE_MESH / 2;
        let body = TxBody::Transfer {
            nonce: 0,
            from,
            to,
            amount,
            fee,
        };
        let tx = Tx::sign(body, &alice).unwrap();
        let block =
            meshchain_proto::block::Block::new(1, st.tip_hash, 2, 0, &producer, vec![tx]).unwrap();
        st.apply_block(&block).unwrap();

        assert_eq!(st.balance_of(&from), 100 * ONE_MESH - amount - fee);
        assert_eq!(st.balance_of(&to), amount);
        let prod = short_id(&producer.public_key());
        assert_eq!(st.balance_of(&prod), fee);
        // supply unchanged by fee transfer
        assert_eq!(st.total_supply, 100 * ONE_MESH);
    }

    fn setup_two_party() -> (ChainState, Keypair, Keypair, Keypair) {
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let producer = Keypair::generate();
        let genesis = GenesisConfig {
            chain_id: "test".into(),
            validators: vec![hex::encode(producer.public_key())],
            block_reward: 0,
            allocations: vec![
                GenesisAccount {
                    public_key_hex: hex::encode(alice.public_key()),
                    balance: 50 * ONE_MESH,
                },
                GenesisAccount {
                    public_key_hex: hex::encode(bob.public_key()),
                    balance: 0,
                },
            ],
            minters: vec![],
            slot_secs: 1,
            pq_required_above: 1000 * ONE_MESH,
            protocol_version: 1,
        };
        let mut st = ChainState::from_genesis(&genesis).unwrap();
        let gblock =
            meshchain_proto::block::Block::new(0, [0u8; 32], 1, 0, &producer, vec![]).unwrap();
        st.apply_block(&gblock).unwrap();
        (st, alice, bob, producer)
    }

    #[test]
    fn rejects_double_spend_same_nonce() {
        let (mut st, alice, bob, producer) = setup_two_party();
        let from = short_id(&alice.public_key());
        let to = short_id(&bob.public_key());
        let body = TxBody::Transfer {
            nonce: 0,
            from,
            to,
            amount: ONE_MESH,
            fee: 0,
        };
        let tx1 = Tx::sign(body.clone(), &alice).unwrap();
        let block1 =
            meshchain_proto::block::Block::new(1, st.tip_hash, 2, 0, &producer, vec![tx1]).unwrap();
        st.apply_block(&block1).unwrap();

        let tx2 = Tx::sign(body, &alice).unwrap(); // same nonce 0
        let block2 =
            meshchain_proto::block::Block::new(2, st.tip_hash, 3, 0, &producer, vec![tx2]).unwrap();
        let err = st.apply_block(&block2).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonce") || msg.contains("invalid"),
            "expected nonce error, got {msg}"
        );
    }

    #[test]
    fn rejects_overspend() {
        let (mut st, alice, bob, producer) = setup_two_party();
        let from = short_id(&alice.public_key());
        let to = short_id(&bob.public_key());
        let body = TxBody::Transfer {
            nonce: 0,
            from,
            to,
            amount: 51 * ONE_MESH,
            fee: 0,
        };
        let tx = Tx::sign(body, &alice).unwrap();
        let block =
            meshchain_proto::block::Block::new(1, st.tip_hash, 2, 0, &producer, vec![tx]).unwrap();
        assert!(st.apply_block(&block).is_err());
    }

    #[test]
    fn rejects_wrong_leader_schedule() {
        let (_st, alice, bob, producer) = setup_two_party();
        let v0 = producer;
        let v1 = Keypair::generate();
        let genesis = GenesisConfig {
            chain_id: "test".into(),
            validators: vec![hex::encode(v0.public_key()), hex::encode(v1.public_key())],
            block_reward: 0,
            allocations: vec![
                GenesisAccount {
                    public_key_hex: hex::encode(alice.public_key()),
                    balance: 50 * ONE_MESH,
                },
                GenesisAccount {
                    public_key_hex: hex::encode(bob.public_key()),
                    balance: 0,
                },
            ],
            minters: vec![],
            slot_secs: 1,
            pq_required_above: 1000 * ONE_MESH,
            protocol_version: 1,
        };
        let mut st = ChainState::from_genesis(&genesis).unwrap();
        let gblock = meshchain_proto::block::Block::new(0, [0u8; 32], 1, 0, &v0, vec![]).unwrap();
        st.apply_block(&gblock).unwrap();
        let from = short_id(&alice.public_key());
        let to = short_id(&bob.public_key());
        let body = TxBody::Transfer {
            nonce: 0,
            from,
            to,
            amount: ONE_MESH,
            fee: 0,
        };
        let tx = Tx::sign(body, &alice).unwrap();
        let bad = meshchain_proto::block::Block::new(1, st.tip_hash, 2, 0, &v0, vec![tx]).unwrap();
        let err = st.apply_block(&bad).unwrap_err();
        assert!(
            err.to_string().contains("leader") || err.to_string().contains("Wrong"),
            "got {err}"
        );
    }

    #[test]
    fn rejects_duplicate_mint_external_ref() {
        let (mut st, _alice, bob, producer) = setup_two_party();
        let to = short_id(&bob.public_key());
        let minter_sid = short_id(&producer.public_key());
        let nonce = st.account(&minter_sid).map(|a| a.nonce).unwrap_or(0);
        let ext = [9u8; 16];
        let body = TxBody::Mint {
            nonce,
            to,
            amount: ONE_MESH,
            external_ref: ext,
            to_pubkey: None,
        };
        let tx = Tx::sign(body, &producer).unwrap();
        let b1 =
            meshchain_proto::block::Block::new(1, st.tip_hash, 2, 0, &producer, vec![tx]).unwrap();
        st.apply_block(&b1).unwrap();
        let nonce2 = st.account(&minter_sid).map(|a| a.nonce).unwrap_or(0);
        let body2 = TxBody::Mint {
            nonce: nonce2,
            to,
            amount: ONE_MESH,
            external_ref: ext, // same ref
            to_pubkey: None,
        };
        let tx2 = Tx::sign(body2, &producer).unwrap();
        let b2 =
            meshchain_proto::block::Block::new(2, st.tip_hash, 3, 0, &producer, vec![tx2]).unwrap();
        let err = st.apply_block(&b2).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("duplicate")
                || err.to_string().to_lowercase().contains("external"),
            "got {err}"
        );
    }

    fn test_genesis() -> GenesisConfig {
        let mut genesis = GenesisConfig::default();
        let val_kp = Keypair::generate();
        genesis.validators = vec![hex::encode(val_kp.public_key())];
        genesis
    }

    #[test]
    fn test_register_and_transfer() {
        let genesis = test_genesis();
        let mut state = ChainState::from_genesis(&genesis).unwrap();

        let alice_kp = Keypair::generate();
        let bob_kp = Keypair::generate();
        let alice_id = short_id(&alice_kp.public_key());
        let bob_id = short_id(&bob_kp.public_key());

        let reg_tx = Tx::sign(
            TxBody::Register {
                nonce: 0,
                pubkey: alice_kp.public_key(),
            },
            &alice_kp,
        )
        .unwrap();
        state.apply_tx(&reg_tx).unwrap();
        assert!(state.account(&alice_id).is_some());

        let alice_acc = state.account_mut(&alice_id).unwrap();
        alice_acc.balance = 10 * ONE_MESH;

        let reg_bob = Tx::sign(
            TxBody::Register {
                nonce: 0,
                pubkey: bob_kp.public_key(),
            },
            &bob_kp,
        )
        .unwrap();
        state.apply_tx(&reg_bob).unwrap();

        let transfer_tx = Tx::sign(
            TxBody::Transfer {
                nonce: 1,
                from: alice_id,
                to: bob_id,
                amount: 3 * ONE_MESH,
                fee: 0,
            },
            &alice_kp,
        )
        .unwrap();
        state.apply_tx(&transfer_tx).unwrap();

        assert_eq!(state.account(&alice_id).unwrap().balance, 7 * ONE_MESH);
        assert_eq!(state.account(&bob_id).unwrap().balance, 3 * ONE_MESH);
    }

    #[test]
    fn test_pq_binding_strict_mode() {
        let genesis = test_genesis();
        let mut state = ChainState::from_genesis(&genesis).unwrap();

        let alice_kp = Keypair::generate();
        let bob_kp = Keypair::generate();
        let alice_id = short_id(&alice_kp.public_key());
        let bob_id = short_id(&bob_kp.public_key());

        let reg_alice = Tx::sign(
            TxBody::Register {
                nonce: 0,
                pubkey: alice_kp.public_key(),
            },
            &alice_kp,
        )
        .unwrap();
        state.apply_tx(&reg_alice).unwrap();

        let reg_bob = Tx::sign(
            TxBody::Register {
                nonce: 0,
                pubkey: bob_kp.public_key(),
            },
            &bob_kp,
        )
        .unwrap();
        state.apply_tx(&reg_bob).unwrap();

        let alice_acc = state.account_mut(&alice_id).unwrap();
        alice_acc.balance = 500 * ONE_MESH;

        let pq_kp = PqKeypair::generate().unwrap();

        let large_transfer = Tx::sign_with_pq(
            TxBody::Transfer {
                nonce: 1,
                from: alice_id,
                to: bob_id,
                amount: 150 * ONE_MESH,
                fee: 0,
            },
            &alice_kp,
            &pq_kp,
        )
        .unwrap();
        state.apply_tx(&large_transfer).unwrap();
        assert!(state.account(&alice_id).unwrap().pq_pk.is_some());

        let small_transfer = Tx::sign(
            TxBody::Transfer {
                nonce: 2,
                from: alice_id,
                to: bob_id,
                amount: 5 * ONE_MESH,
                fee: 0,
            },
            &alice_kp,
        )
        .unwrap();
        assert!(state.apply_tx(&small_transfer).is_err());
    }
}
