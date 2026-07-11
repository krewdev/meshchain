# MeshChain Validator Automation

Comprehensive package to **bootstrap, run, monitor, heal, and operate** a local or host validator set for MeshChain (`meshchain-testnet-1`).

```text
validator-automation/
  mesh-validator          # main CLI
  lib/common.sh           # shared helpers
  config/                 # lab.env, host.env.example, …
  systemd/                # optional units + watchdog timer
  README.md
```

## Quick start (lab)

```bash
cd ~/meshchain

# One-shot setup + start 3 validators on :9100–:9102
./validator-automation/mesh-validator bootstrap
./validator-automation/mesh-validator start
./validator-automation/mesh-validator status
./validator-automation/mesh-validator health
```

Stop:

```bash
./validator-automation/mesh-validator stop
```

## What it automates

| Concern | Command |
|---------|---------|
| Build node if missing | `bootstrap` / `start` (`AUTO_BUILD=1`) |
| Genesis + key trees `v0..vN` | `bootstrap` |
| Start/stop/restart | `start` `stop` `restart` |
| Port conflict detection | `start` |
| PID + meta tracking | `data/*/logs/validators.pids` |
| Live status (ports, heights) | `status` |
| Health for monitors (exit code) | `health` |
| Log tails | `logs [idx\|all]` |
| State sync / CLI snapshot | `sync` `promote-lab` |
| Key/genesis diagnostics | `doctor` |
| Auto-restart dead nodes | `watchdog` |
| Submit signed tx | `submit path.json` |
| systemd always-on | `systemd/*` |

## Commands

```bash
./validator-automation/mesh-validator help
```

### Bootstrap

Creates/uses genesis, prepares `DATA_ROOT/v{i}` trees with matching validator keys.

```bash
./validator-automation/mesh-validator bootstrap
./validator-automation/mesh-validator --release --mode host bootstrap
```

### Operate

```bash
./validator-automation/mesh-validator start
./validator-automation/mesh-validator status
./validator-automation/mesh-validator logs 0          # tail v0
./validator-automation/mesh-validator logs all
./validator-automation/mesh-validator restart
./validator-automation/mesh-validator stop
```

### Health & watchdog

```bash
# CI / cron / monitoring (exit 0 healthy, 1 not)
./validator-automation/mesh-validator health

# Foreground heal loop
./validator-automation/mesh-validator watchdog
```

### Wallet CLI alignment

Validators write state under `data/v0/chain_state.json`. The mesh CLI reads `./data/chain_state.json`.

```bash
# After payments finalize, promote live height for mesh balance/status
./validator-automation/mesh-validator promote-lab
./target/debug/mesh status
```

### Submit a payment

```bash
./target/debug/mesh send <MESH_NAME> 5 --wallet data/keys/t2mesh.json --fee 0.1
./validator-automation/mesh-validator submit data/last_payment.json
./validator-automation/mesh-validator promote-lab
```

## Configuration

Defaults load from:

1. `--config PATH`, or  
2. `config/local.env` (your overrides), or  
3. `config/lab.env`

| Variable | Default | Meaning |
|----------|---------|---------|
| `CHAIN_MODE` | `lab` | `lab` → `./data`, `host` → `./data/host` |
| `VALIDATOR_COUNT` | `3` | Number of PoA validators |
| `BASE_PORT` | `9100` | First TCP gossip port |
| `SLOT_MS` | `100` | Node poll interval |
| `BUILD_PROFILE` | `debug` | `debug` or `release` |
| `AUTO_BUILD` | `1` | cargo build if binary missing |
| `WATCHDOG_INTERVAL_SECS` | `15` | watchdog loop |

CLI flags override env:

```bash
./validator-automation/mesh-validator --mode host --count 3 --base-port 9100 --release start
```

## Layout after start

```text
data/                    # lab mode
  genesis.json
  keys/validator-*.json
  v0/  v1/  v2/          # per-validator trees
  logs/
    validators.pids
    validators.meta
    validators/v0.log …
```

## systemd (VPS)

```bash
sudo cp validator-automation/systemd/meshchain-validators.service /etc/systemd/system/
sudo cp validator-automation/systemd/meshchain-watchdog.service /etc/systemd/system/
sudo cp validator-automation/systemd/meshchain-watchdog.timer /etc/systemd/system/
# edit paths/user if not /opt/meshchain + meshchain user
sudo systemctl daemon-reload
sudo systemctl enable --now meshchain-validators
sudo systemctl enable --now meshchain-watchdog.timer
```

## Relation to older scripts

| Old script | Package equivalent |
|------------|--------------------|
| `scripts/run_local_validators.sh` | `mesh-validator start` (background + pidfile) |
| `scripts/host_bootstrap.sh` | `mesh-validator --mode host --release bootstrap` |
| `scripts/start_testnet_host.sh` | `mesh-validator --mode host start` (+ faucet still separate) |
| `scripts/stop_testnet_host.sh` | `mesh-validator stop` (validators only) |

Faucet / scanner / Cloudflare tunnel remain in `scripts/*` — this package focuses on **validators**.

## Priority fees (MEV tips)

With the node supporting priority fees:

```bash
./target/debug/mesh send <TO> 10 --fee 0.5 --wallet data/keys/t2mesh.json
./validator-automation/mesh-validator submit data/last_payment.json
```

Higher `--fee` → preferred mempool order and earlier inclusion.

## Safety

- Testnet / lab only unless you know what you are doing  
- `sync` while validators run can race — prefer `stop` → sync → `start` for hard resets  
- Never expose validator keys; only open gossip ports to trusted peers  

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| port in use | `mesh-validator stop` or `lsof -iTCP:9100-9102` |
| height skew | `health`; `restart`; or stop → `sync` → start |
| mesh balance stale | `promote-lab` |
| key mismatch | `doctor` then re-`bootstrap` on fresh data dir |
| binary missing | `bootstrap` or `cargo build -p meshchain-node` |
