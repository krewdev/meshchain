# Multi-validator (lab)

> **Prefer:** [MULTI_OPERATOR.md](MULTI_OPERATOR.md) for public / multi-host operators.  
> This file is the short lab pointer.

## Local N validators

```bash
cargo build -p meshchain-node
./target/debug/meshchain-node init --data-dir ./data --validators 3
# or: scripts/run_local_validators.sh
./scripts/e2e_multi_node.sh   # multi-process PoA smoke
```

Finality: ≥ `ceil(2N/3)` **signed** BlockAcks. Leader: `height % N`.

See [STATUS.md](STATUS.md) for live testnet integrity posture.
