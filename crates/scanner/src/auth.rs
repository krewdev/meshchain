//! Scanner access control.
//!
//! - `Open`: internet-public (current testnet default)
//! - `Mesh2fa`: require MeshChain signed challenge (scaffolded for later)

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Anyone on the internet can browse (testnet default).
    Open,
    /// Future: require mesh identity 2FA (signed challenge).
    Mesh2fa,
}

impl AuthMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "open" | "public" | "none" => Ok(Self::Open),
            "mesh2fa" | "mesh" | "2fa" => Ok(Self::Mesh2fa),
            other => bail!("unknown --auth {other} (use open|mesh2fa)"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshChallenge {
    pub challenge_id: String,
    pub message: String,
    pub expires_unix: u64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshChallengeResponse {
    pub challenge_id: String,
    /// Hex-encoded ed25519 pubkey (32 bytes)
    pub pubkey_hex: String,
    /// Hex-encoded signature over `message` bytes
    pub signature_hex: String,
}

/// Create a challenge for mesh 2FA login (stateless id = hash of message).
pub fn issue_challenge(now: u64, ttl_secs: u64) -> MeshChallenge {
    let expires = now.saturating_add(ttl_secs);
    let nonce = format!("{now}-{}", rand_token());
    let message =
        format!("MESHCHAIN-SCANNER-2FA\nchain=meshchain-testnet-1\nexp={expires}\nnonce={nonce}");
    let mut h = Sha256::new();
    h.update(message.as_bytes());
    let challenge_id = hex::encode(h.finalize());
    MeshChallenge {
        challenge_id,
        message,
        expires_unix: expires,
        note: "Sign message with your mesh wallet ed25519 key, then POST /api/v1/auth/verify. Not enforced while auth=open.".into(),
    }
}

fn rand_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{t:x}")
}

/// Verify mesh 2FA response (ed25519 over challenge message).
pub fn verify_challenge(resp: &MeshChallengeResponse, expected_message: &str) -> Result<()> {
    use meshchain_proto::crypto::{PublicKey, Signature, SignatureBytes};

    let pk_bytes = hex::decode(resp.pubkey_hex.trim()).map_err(|e| anyhow::anyhow!(e))?;
    if pk_bytes.len() != 32 {
        bail!("pubkey must be 32 bytes hex");
    }
    let mut pk: PublicKey = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let sig_bytes = hex::decode(resp.signature_hex.trim()).map_err(|e| anyhow::anyhow!(e))?;
    if sig_bytes.len() != 64 {
        bail!("signature must be 64 bytes hex");
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&sig_bytes);
    let sig = SignatureBytes(sig);

    Signature::verify(&pk, expected_message.as_bytes(), &sig)
        .map_err(|e| anyhow::anyhow!("mesh 2fa verify failed: {e}"))?;
    Ok(())
}
