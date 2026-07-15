//! Scanner command.

use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

pub fn cmd_scanner(dir: &Path, listen: &str, auth: &str) -> Result<()> {
    println!("Starting MeshChain scanner…");
    println!("Auth mode: {auth} (use mesh2fa later for mesh identity gate)");
    let status = Command::new("meshchain-scanner")
        .args([
            "--data-dir",
            dir.to_str().unwrap_or("./data"),
            "--listen",
            listen,
            "--auth",
            auth,
        ])
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(_) => bail!("scanner exited with error"),
        Err(_) => {
            // Try the sibling binary in the same target dir as mesh
            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    let bin = parent.join("meshchain-scanner");
                    let st = Command::new(bin)
                        .args([
                            "--data-dir",
                            dir.to_str().unwrap_or("./data"),
                            "--listen",
                            listen,
                            "--auth",
                            auth,
                        ])
                        .status()?;
                    if !st.success() {
                        bail!("scanner failed — build with: cargo build -p meshchain-scanner");
                    }
                    return Ok(());
                }
            }
            bail!("meshchain-scanner not found. cargo build -p meshchain-scanner");
        }
    }
    Ok(())
}
