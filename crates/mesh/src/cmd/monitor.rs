use anyhow::Result;
use meshchain_ledger::state::ChainState;

use std::path::Path;
use std::thread;
use std::time::Duration;
use std::net::TcpStream;
use crate::helpers::{best_chain_state_path, fmt_mesh};

pub fn cmd_monitor(dir: &Path) -> Result<()> {
    let state_path = best_chain_state_path(dir);
    println!("Starting MeshChain Live Monitor...");

    loop {
        // Clear screen and reset cursor to home (1,1)
        print!("{}[2J{}[1;1H", 27 as char, 27 as char);

        println!("==========================================================");
        println!("               MESHCHAIN LIVE BLOCKCHAIN MONITOR         ");
        println!("==========================================================");

        // Load Chain State
        if state_path.exists() {
            match ChainState::load_json(&state_path) {
                Ok(state) => {
                    println!(" Chain ID:       {}", state.chain_id);
                    println!(" Block Height:   {}", state.height);
                    println!(" Tip Hash:       {}", hex::encode(state.tip_hash));
                    println!(" Total Supply:   {} MESH", fmt_mesh(state.total_supply));
                    println!(" Total Accounts: {}", state.accounts.len());
                    println!(" Total Blocks:   {}", state.applied.len());

                    let n = state.validators.len();
                    if n > 0 {
                        let leader_idx = (state.height % n as u64) as usize;
                        let leader_pk = state.validators[leader_idx];
                        let leader_hex = hex::encode(leader_pk);
                        println!(" Validators:     {}", n);
                        println!(" Current Leader: {} (index {})", &leader_hex[..16], leader_idx);
                    }
                }
                Err(e) => {
                    println!(" ❌ Error reading chain_state.json: {}", e);
                }
            }
        } else {
            println!(" ⚠️  No chain state found at: {}", state_path.display());
            println!("    (Run: 'mesh setup && mesh demo' to initialize the chain)");
        }

        println!("----------------------------------------------------------");
        println!(" RADIO BRIDGE CONFIGURATION                               ");
        println!("----------------------------------------------------------");
        
        let cfg = crate::config::MeshConfig::load_or_default(dir);
        println!(" Default Wallet:   {}", cfg.default_wallet);
        println!(" Radio Port:       {}", cfg.radio_port.as_deref().unwrap_or("None"));
        println!(" Transmit Delay:   {} ms", cfg.tx_delay_ms);
        println!(" Meshtastic App:   PortNum {}", cfg.portnum);
        println!(" Frame Compression: {}", if cfg.compression { "Enabled" } else { "Disabled" });

        println!("----------------------------------------------------------");
        println!(" LOCAL PROCESS HEALTH                                     ");
        println!("----------------------------------------------------------");
        
        // Check local validator status
        let local_val = match TcpStream::connect_timeout(
            &"127.0.0.1:9100".parse().unwrap(),
            Duration::from_millis(200),
        ) {
            Ok(_) => "ONLINE (TCP 9100)",
            Err(_) => "OFFLINE / NOT RUNNING",
        };
        println!(" Validator Node:   {}", local_val);

        // Check local relay status
        let local_relay = match TcpStream::connect_timeout(
            &"127.0.0.1:9199".parse().unwrap(),
            Duration::from_millis(200),
        ) {
            Ok(_) => "ONLINE (TCP 9199)",
            Err(_) => "OFFLINE / NOT RUNNING",
        };
        println!(" Radio Relay:      {}", local_relay);

        println!("==========================================================");
        println!(" Press Ctrl+C to exit. Refreshing every 1.5 seconds...");
        
        std::io::Write::flush(&mut std::io::stdout())?;
        thread::sleep(Duration::from_millis(1500));
    }
}
