//! MeshChain ledger: accounts, apply txs/blocks, genesis, persistence (JSON).

pub mod error;
pub mod genesis;
pub mod state;

pub use error::LedgerError;
pub use genesis::{GenesisConfig, GenesisAccount};
pub use state::{Account, ChainState, AppliedBlock};
pub mod registry;
