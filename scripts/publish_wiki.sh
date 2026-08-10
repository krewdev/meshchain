#!/usr/bin/env bash
# Automatically publish docs/ to GitHub Wiki using the parent repo's authentication format.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 1. Determine wiki repository URL based on parent remote
ORIGIN=$(git config --get remote.origin.url || echo "")
if [[ -z "$ORIGIN" ]]; then
  echo "Error: remote.origin.url not found. Make sure you are in a git repository."
  exit 1
fi
WIKI_URL="${ORIGIN%.git}.wiki.git"

WIKI_DIR="$ROOT/target/meshchain-wiki"
rm -rf "$WIKI_DIR"

echo "Cloning wiki repository: $WIKI_URL"
if ! git clone "$WIKI_URL" "$WIKI_DIR"; then
  echo ""
  echo "⚠️  Clone failed. If the wiki repository is completely empty, you must first"
  echo "   enable/create at least one wiki page on the GitHub Web UI to initialize it."
  echo "   Once initialized, re-run this script."
  exit 1
fi

# 2. Copy docs md files
echo "Copying documentation pages..."
cp docs/*.md "$WIKI_DIR/"

# 3. Create Home.md if it doesn't exist
echo "Generating Home.md index page..."
cat << 'EOF' > "$WIKI_DIR/Home.md"
# Welcome to the MeshChain Wiki

MeshChain is a high-performance PoA blockchain built for Meshtastic radio networks.

## Documentation Index

### Getting Started
* [[Getting Started Guide | GETTING_STARTED]]
* [[Run a Node | RUN_A_NODE]]
* [[Blockchain Scanner | SCANNER]]
* [[Scanner Auto-Updates | SCANNER_AUTO_UPDATE]]

### Architecture & Protocols
* [[Protocol Specification | PROTOCOL]]
* [[Meshtastic Integration | MESHTASTIC]]
* [[Hybrid Locking Mechanisms | HYBRID_LOCK]]
* [[Truth Model | TRUTH_MODEL]]

### Bridges & Vaults
* [[Solana Bridge | SOLANA_BRIDGE]]
* [[Bitcoin Vault | BTC_VAULT]]
* [[Quantum Cold Storage | QUANTUM_COLD_STORAGE]]

### Network & Infrastructure
* [[Cloud Deployment | CLOUD]]
* [[Multi-Validator Lab | MULTI_VALIDATOR]]
* [[Multi-Operator Operations | MULTI_OPERATOR]]
* [[Host Operations | HOST_OPS]]
* [[Public VPS | VPS_PUBLIC]]
* [[Audit and Test Suite | AUDIT_AND_TEST]]

### Community & Safety
* [[Discord Setup | DISCORD]]
* [[Security and Hardening | SECURITY_HARDENING]]
* [[General Security Policy | SECURITY]]
* [[Status & Roadmap | STATUS]]
* [[Donate to the Project | DONATE]]
EOF

# 4. Create _Sidebar.md for navigation sidebar
echo "Generating _Sidebar.md navigation sidebar..."
cat << 'EOF' > "$WIKI_DIR/_Sidebar.md"
### Navigation
* [[Home]]

### Guides
* [[Getting Started | GETTING_STARTED]]
* [[Run a Node | RUN_A_NODE]]
* [[Scanner | SCANNER]]

### Integration
* [[Meshtastic | MESHTASTIC]]
* [[Solana Bridge | SOLANA_BRIDGE]]
* [[BTC Vault | BTC_VAULT]]

### Core Spec
* [[Protocol | PROTOCOL]]
* [[Hybrid Lock | HYBRID_LOCK]]
* [[Quantum Cold Storage | QUANTUM_COLD_STORAGE]]

### Operations
* [[Cloud Deploy | CLOUD]]
* [[Security Hardening | SECURITY_HARDENING]]
* [[Audit & Test | AUDIT_AND_TEST]]
EOF

# 5. Commit and push wiki
cd "$WIKI_DIR"
git add .
if git diff-index --quiet HEAD --; then
  echo "No changes to commit in wiki."
else
  git commit -m "docs: sync wiki pages from main repository"
  echo "Pushing updates to GitHub Wiki..."
  git push origin main || git push origin master
  echo "Wiki published successfully!"
fi
rm -rf "$WIKI_DIR"
