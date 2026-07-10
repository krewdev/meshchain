use meshchain_proto::address::{short_id, Address, ShortId};
use meshchain_proto::crypto::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MESH uses 6 decimals: 1_000_000 = 1.0 MESH
pub const DECIMALS: u32 = 6;
pub const ONE_MESH: u64 = 1_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAccount {
    pub public_key_hex: String,
    pub balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    pub chain_id: String,
    pub validators: Vec<String>,
    /// Block reward in base units paid to producer each non-genesis block.
    pub block_reward: u64,
    /// Initial balances.
    pub allocations: Vec<GenesisAccount>,
    /// Authorized minter pubkeys (hex). Validators included by default at load.
    pub minters: Vec<String>,
    /// Slot duration hint (seconds) for clients.
    pub slot_secs: u64,
    /// Transfers/burns at or above this many base units require a valid ML-DSA-65 signature.
    /// Default: 100 MESH. Set 0 to require PQ on all spends (strict cold mode).
    #[serde(default = "default_pq_threshold")]
    pub pq_required_above: u64,
}

fn default_pq_threshold() -> u64 {
    100 * ONE_MESH
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            chain_id: "meshchain-dev".into(),
            validators: vec![],
            block_reward: 100_000, // 0.1 MESH
            allocations: vec![],
            minters: vec![],
            slot_secs: 30,
            pq_required_above: default_pq_threshold(),
        }
    }
}

impl GenesisConfig {
    pub fn parse_pubkey(hex_str: &str) -> Result<PublicKey, String> {
        let bytes = hex::decode(hex_str.trim()).map_err(|e| e.to_string())?;
        if bytes.len() != 32 {
            return Err(format!("pubkey must be 32 bytes, got {}", bytes.len()));
        }
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&bytes);
        Ok(pk)
    }

    pub fn validator_keys(&self) -> Result<Vec<PublicKey>, String> {
        self.validators
            .iter()
            .map(|h| Self::parse_pubkey(h))
            .collect()
    }

    pub fn minter_set(&self) -> Result<std::collections::HashSet<PublicKey>, String> {
        let mut set = std::collections::HashSet::new();
        for h in &self.minters {
            set.insert(Self::parse_pubkey(h)?);
        }
        for h in &self.validators {
            set.insert(Self::parse_pubkey(h)?);
        }
        Ok(set)
    }

    pub fn initial_accounts(&self) -> Result<HashMap<ShortId, (Address, u64)>, String> {
        let mut map = HashMap::new();
        for a in &self.allocations {
            let pk = Self::parse_pubkey(&a.public_key_hex)?;
            let sid = short_id(&pk);
            if map.contains_key(&sid) {
                return Err("short id collision in genesis".into());
            }
            map.insert(sid, (pk, a.balance));
        }
        Ok(map)
    }
}
