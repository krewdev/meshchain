---
description: Start or debug the MeshChain blockchain scanner
---

Help the user run the MeshChain scanner:

1. Ensure `data/chain_state.json` exists (suggest `mesh testnet-setup` + demo if missing).
2. Build: `cargo build -p meshchain-scanner`
3. Run: `./target/debug/meshchain-scanner --data-dir ./data --listen 0.0.0.0:8787 --auth open`
4. Curl `http://127.0.0.1:8787/api/v1/status` to verify.
5. Explain open vs mesh2fa auth.

Do not claim the scanner is on Vercel — it must run as a process next to chain data.
