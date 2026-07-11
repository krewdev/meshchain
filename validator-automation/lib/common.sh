#!/usr/bin/env bash
# Shared helpers for MeshChain validator automation.
# shellcheck disable=SC2034

set -euo pipefail

VA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$VA_ROOT/.." && pwd)"

# Defaults (overridden by config env + CLI)
MESHCHAIN_HOME="${MESHCHAIN_HOME:-$REPO_ROOT}"
VALIDATOR_COUNT="${VALIDATOR_COUNT:-3}"
BASE_PORT="${BASE_PORT:-9100}"
DATA_ROOT="${DATA_ROOT:-}"
SLOT_MS="${SLOT_MS:-100}"
BUILD_PROFILE="${BUILD_PROFILE:-debug}"   # debug | release
CHAIN_MODE="${CHAIN_MODE:-lab}"           # lab (use ./data) | host (./data/host)
AUTO_BUILD="${AUTO_BUILD:-1}"
WATCHDOG_INTERVAL_SECS="${WATCHDOG_INTERVAL_SECS:-15}"
LOG_DIR="${LOG_DIR:-}"
PID_DIR="${PID_DIR:-}"

load_config() {
  local cfg="${1:-}"
  if [[ -z "$cfg" ]]; then
    if [[ -f "$VA_ROOT/config/local.env" ]]; then
      cfg="$VA_ROOT/config/local.env"
    elif [[ -f "$VA_ROOT/config/lab.env" ]]; then
      cfg="$VA_ROOT/config/lab.env"
    fi
  fi
  if [[ -n "${cfg:-}" && -f "$cfg" ]]; then
    # shellcheck disable=SC1090
    set -a
    # shellcheck source=/dev/null
    source "$cfg"
    set +a
  fi

  MESHCHAIN_HOME="${MESHCHAIN_HOME:-$REPO_ROOT}"
  if [[ -z "${DATA_ROOT:-}" ]]; then
    if [[ "$CHAIN_MODE" == "host" ]]; then
      DATA_ROOT="$MESHCHAIN_HOME/data/host"
    else
      DATA_ROOT="$MESHCHAIN_HOME/data"
    fi
  fi
  LOG_DIR="${LOG_DIR:-$DATA_ROOT/logs/validators}"
  PID_DIR="${PID_DIR:-$DATA_ROOT/logs}"
  mkdir -p "$LOG_DIR" "$PID_DIR" "$DATA_ROOT"
}

node_bin() {
  local p
  if [[ "$BUILD_PROFILE" == "release" ]]; then
    p="$MESHCHAIN_HOME/target/release/meshchain-node"
  else
    p="$MESHCHAIN_HOME/target/debug/meshchain-node"
  fi
  if [[ -x "$p" ]]; then
    echo "$p"
    return 0
  fi
  # fallbacks
  for p in \
    "$MESHCHAIN_HOME/target/debug/meshchain-node" \
    "$MESHCHAIN_HOME/target/release/meshchain-node"; do
    if [[ -x "$p" ]]; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

mesh_bin() {
  local p
  if [[ "$BUILD_PROFILE" == "release" ]]; then
    p="$MESHCHAIN_HOME/target/release/mesh"
  else
    p="$MESHCHAIN_HOME/target/debug/mesh"
  fi
  if [[ -x "$p" ]]; then
    echo "$p"
    return 0
  fi
  for p in \
    "$MESHCHAIN_HOME/target/debug/mesh" \
    "$MESHCHAIN_HOME/target/release/mesh"; do
    if [[ -x "$p" ]]; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

ensure_build() {
  if node_bin >/dev/null 2>&1; then
    return 0
  fi
  if [[ "$AUTO_BUILD" != "1" ]]; then
    die "meshchain-node not found. Build: cargo build -p meshchain-node (or set AUTO_BUILD=1)"
  fi
  log "Building meshchain-node + mesh ($BUILD_PROFILE)…"
  (
    cd "$MESHCHAIN_HOME"
    if [[ "$BUILD_PROFILE" == "release" ]]; then
      cargo build -p meshchain-node -p mesh --release
    else
      cargo build -p meshchain-node -p mesh
    fi
  )
}

vdir() {
  local i="$1"
  echo "$DATA_ROOT/v$i"
}

port_for() {
  local i="$1"
  echo $((BASE_PORT + i))
}

pidfile() {
  echo "$PID_DIR/validators.pids"
}

metafile() {
  echo "$PID_DIR/validators.meta"
}

log() { printf '[mesh-validator] %s\n' "$*"; }
warn() { printf '[mesh-validator] WARN: %s\n' "$*" >&2; }
die() { printf '[mesh-validator] ERROR: %s\n' "$*" >&2; exit 1; }

is_pid_alive() {
  local pid="$1"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

port_listening() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1
  elif command -v nc >/dev/null 2>&1; then
    nc -z 127.0.0.1 "$port" >/dev/null 2>&1
  else
    return 1
  fi
}

read_height() {
  local state="$1"
  if [[ ! -f "$state" ]]; then
    echo "-"
    return
  fi
  python3 - "$state" <<'PY' 2>/dev/null || echo "-"
import json, sys
st = json.load(open(sys.argv[1]))
print(st.get("height", "-"))
PY
}

read_chain_id() {
  local g="$1"
  if [[ ! -f "$g" ]]; then
    echo "-"
    return
  fi
  python3 - "$g" <<'PY' 2>/dev/null || echo "-"
import json, sys
print(json.load(open(sys.argv[1])).get("chain_id", "-"))
PY
}

peer_flags_for() {
  local self="$1"
  local i flags=()
  for ((i = 0; i < VALIDATOR_COUNT; i++)); do
    if [[ "$i" -eq "$self" ]]; then
      continue
    fi
    flags+=(--peer "127.0.0.1:$(port_for "$i")")
  done
  printf '%s\n' "${flags[@]}"
}

prepare_validator_tree() {
  local i="$1"
  local d
  d="$(vdir "$i")"
  mkdir -p "$d/keys"
  [[ -f "$DATA_ROOT/genesis.json" ]] || die "missing $DATA_ROOT/genesis.json — run: mesh-validator bootstrap"
  cp -f "$DATA_ROOT/genesis.json" "$d/"
  if [[ -f "$DATA_ROOT/testnet_profile.json" ]]; then
    cp -f "$DATA_ROOT/testnet_profile.json" "$d/"
  fi
  # Shared keys live in DATA_ROOT/keys or host keys
  local keysrc="$DATA_ROOT/keys"
  if [[ ! -d "$keysrc" && -d "$MESHCHAIN_HOME/data/keys" && "$CHAIN_MODE" == "lab" ]]; then
    keysrc="$MESHCHAIN_HOME/data/keys"
  fi
  if [[ -d "$keysrc" ]]; then
    cp -f "$keysrc"/validator-*.json "$d/keys/" 2>/dev/null || true
  fi
  # Prefer existing per-validator state; else seed from DATA_ROOT chain_state
  if [[ ! -f "$d/chain_state.json" && -f "$DATA_ROOT/chain_state.json" ]]; then
    cp -f "$DATA_ROOT/chain_state.json" "$d/"
  fi
  # Faucet key on v0 for mint tools
  if [[ "$i" -eq 0 && -f "$keysrc/faucet.json" ]]; then
    cp -f "$keysrc/faucet.json" "$d/keys/" 2>/dev/null || true
  fi
}
