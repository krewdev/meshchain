//! MeshChain ledger: accounts, apply txs/blocks, genesis, persistence (JSON).

pub mod error;
pub mod genesis;
pub mod state;

pub use error::LedgerError;
pub use genesis::{GenesisAccount, GenesisConfig};
pub use state::{Account, AppliedBlock, ChainState};
pub mod registry;
