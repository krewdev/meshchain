use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::net::TcpStream;
use std::time::Duration;

pub fn cmd_doctor(dir: &Path) -> Result<()> {
    println!("MeshChain Diagnostics System (\"Doctor\")");
    println!("=========================================");

    let mut checks_passed = 0;
    let mut checks_failed = 0;
    let mut warnings = 0;

    // 1. Python 3 Check
    print!("[*] Checking Python 3... ");
    let py_status = Command::new("python3").arg("--version").output();
    match py_status {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("PASS ({})", ver);
            checks_passed += 1;
        }
        _ => {
            println!("FAIL (python3 not found on PATH)");
            checks_failed += 1;
        }
    }

    // 2. Python Meshtastic Package Check
    print!("[*] Checking Python 'meshtastic' library... ");
    let py_mesh = Command::new("python3")
        .args(&["-c", "import meshtastic"])
        .output();
    match py_mesh {
        Ok(out) if out.status.success() => {
            println!("PASS");
            checks_passed += 1;
        }
        _ => {
            println!("WARNING ('meshtastic' python library is not installed)");
            println!("    └─ Run: pip install meshtastic");
            warnings += 1;
        }
    }

    // 3. Serial Ports Scan
    print!("[*] Scanning for potential LoRa/USB serial devices... ");
    let mut serial_devices = Vec::new();
    if let Ok(entries) = fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("cu.usbserial") || name.starts_with("cu.usbmodem") ||
               name.starts_with("ttyUSB") || name.starts_with("ttyACM") {
                serial_devices.push(format!("/dev/{}", name));
            }
        }
    }
    if serial_devices.is_empty() {
        println!("NONE FOUND (No active USB serial radios connected)");
        warnings += 1;
    } else {
        println!("FOUND {} device(s):", serial_devices.len());
        for dev in &serial_devices {
            println!("    └─ {}", dev);
        }
        checks_passed += 1;
    }

    // 4. Configuration Check
    print!("[*] Loading default zero-config 'data/config.json'... ");
    let config_path = dir.join("config.json");
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            match serde_json::from_str::<crate::config::MeshConfig>(&content) {
                Ok(cfg) => {
                    println!("PASS");
                    println!("    ├─ Default Wallet: {}", cfg.default_wallet);
                    println!("    ├─ Radio Port:     {}", cfg.radio_port.as_deref().unwrap_or("None"));
                    println!("    ├─ Tx Delay:       {} ms", cfg.tx_delay_ms);
                    println!("    └─ PortNum:        {}", cfg.portnum);
                    checks_passed += 1;
                }
                Err(e) => {
                    println!("FAIL (Invalid JSON: {})", e);
                    checks_failed += 1;
                }
            }
        } else {
            println!("FAIL (Could not read file)");
            checks_failed += 1;
        }
    } else {
        println!("WARNING (config.json does not exist, using defaults)");
        warnings += 1;
    }

    // 5. Genesis and Keys Check
    print!("[*] Checking genesis.json and validator key files... ");
    let genesis_path = dir.join("genesis.json");
    if genesis_path.exists() {
        let keys_dir = dir.join("keys");
        let keys_exist = keys_dir.exists() && keys_dir.is_dir();
        println!("PASS");
        println!("    ├─ Genesis file: FOUND");
        println!("    └─ Keys directory: {}", if keys_exist { "FOUND" } else { "NOT FOUND" });
        checks_passed += 1;
    } else {
        println!("WARNING (No network files found. Run: mesh setup)");
        warnings += 1;
    }

    // 6. Validator Connectivity Check
    print!("[*] Checking local validator node connectivity (TCP 127.0.0.1:9100)... ");
    match TcpStream::connect_timeout(
        &"127.0.0.1:9100".parse().unwrap(),
        Duration::from_millis(500),
    ) {
        Ok(_) => {
            println!("PASS (Validator node is online)");
            checks_passed += 1;
        }
        Err(_) => {
            println!("DISCONNECTED (No validator node detected at default port)");
            warnings += 1;
        }
    }

    println!("\nDiagnostics Summary:");
    println!("--------------------");
    println!("  Checks Passed: {}", checks_passed);
    println!("  Checks Failed: {}", checks_failed);
    println!("  Warnings:      {}", warnings);

    if checks_failed > 0 {
        println!("\n❌ Doctor recommends resolving the failed checks above.");
    } else if warnings > 0 {
        println!("\n⚠️  Doctor recommends addressing the warnings for a complete setup.");
    } else {
        println!("\n✅ All checks passed! Your MeshChain node is in perfect health.");
    }

    Ok(())
}
