use crate::state::ChainState;
use meshchain_proto::address::mesh_name;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct Registry {
    pub names: HashMap<String, String>,
}

impl Registry {
    pub fn from_state(state: &ChainState) -> Self {
        let mut names = HashMap::new();
        for (sid, _acc) in &state.accounts {
            if let Ok(bytes) = hex::decode(sid) {
                if bytes.len() == 8 {
                    let mut short = [0u8; 8];
                    short.copy_from_slice(&bytes);
                    let name = mesh_name(&short);
                    names.insert(name, sid.clone());
                }
            }
        }
        Self { names }
    }
}
