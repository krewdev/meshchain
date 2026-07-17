//! Minimal HTTP/1.1 server (std only) for scanner API + UI.

use crate::auth::{self, AuthMode};
use crate::model;
use crate::ui;
use crate::AppState;
use anyhow::Result;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Pending mesh2fa challenges: challenge_id → message (in-memory; fine for testnet).
static CHALLENGES: Mutex<Vec<(String, String, u64)>> = Mutex::new(Vec::new());

pub fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    let listener = TcpListener::bind(addr)?;
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let st = state.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_client(s, st) {
                        eprintln!("request error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn handle_client(mut stream: TcpStream, state: AppState) -> Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path_q = parts.next().unwrap_or("/");
    let (path, query) = match path_q.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path_q, ""),
    };

    // Body (for POST)
    let body = if let Some(idx) = req.find("\r\n\r\n") {
        req[idx + 4..].to_string()
    } else {
        String::new()
    };

    // CORS preflight
    if method == "OPTIONS" {
        return write_response(
            &mut stream,
            204,
            "text/plain",
            "",
            &[
                ("Access-Control-Allow-Origin", "*"),
                ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
                (
                    "Access-Control-Allow-Headers",
                    "Content-Type, Authorization",
                ),
            ],
        );
    }

    // Auth gate for API (except public health + challenge endpoints)
    let public = matches!(
        path,
        "/" | "/index.html"
            | "/api/v1/status"
            | "/api/v1/chain_state"
            | "/api/v1/network"
            | "/api/v1/auth/mode"
            | "/api/v1/auth/challenge"
            | "/favicon.ico"
    ) || path.starts_with("/assets/");

    if !public && state.auth_mode == AuthMode::Mesh2fa {
        // Future: check session token from mesh 2FA verify
        // For now return 401 with challenge hint
        let ch = auth::issue_challenge(now(), 300);
        let json = serde_json::json!({
            "error": "mesh_2fa_required",
            "message": "Scanner is in mesh2fa mode. Complete mesh challenge first.",
            "challenge": ch,
        });
        return write_json(&mut stream, 401, &json);
    }

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") | ("GET", "/scanner") | ("GET", "/scanner/") => {
            write_response(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                ui::INDEX_HTML,
                &[],
            )
        }
        ("GET", "/api/v1/status") => {
            let c = state.chain.read().unwrap();
            let body = model::StatusResponse {
                ok: true,
                service: "meshchain-scanner",
                auth_mode: format!("{:?}", state.auth_mode).to_lowercase(),
                chain_id: c.chain_id.clone(),
                height: c.height,
                tip_hash_hex: model::tip_hash_hex(&c),
                total_supply: c.total_supply,
                total_supply_tmesh: c.total_supply as f64 / 1e6,
                account_count: c.accounts.len(),
                block_count: c.applied.len(),
                block_reward: c.block_reward,
                pq_required_above: c.pq_required_above,
                validators: c.validators.len(),
                is_testnet: c.chain_id.contains("testnet"),
                warning: "TESTNET ONLY — tMESH has no cash value",
                uptime_secs: now().saturating_sub(state.started_unix),
                mesh_2fa: model::Mesh2faInfo {
                    enforced: state.auth_mode == AuthMode::Mesh2fa,
                    challenge_path: "/api/v1/auth/challenge",
                    verify_path: "/api/v1/auth/verify",
                    status: if state.auth_mode == AuthMode::Mesh2fa {
                        "enforced"
                    } else {
                        "available_not_enforced"
                    },
                },
            };
            write_json(&mut stream, 200, &body)
        }
        ("GET", "/api/v1/network") => {
            let meta = state.network_meta.read().unwrap().clone();
            write_json(&mut stream, 200, &meta)
        }
        // Full ledger snapshot for light clients / mesh sync-state
        ("GET", "/api/v1/chain_state") => {
            let c = state.chain.read().unwrap();
            write_json(&mut stream, 200, &*c)
        }
        ("GET", "/api/v1/blocks") => {
            let limit = query_param(query, "limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(50usize)
                .min(500);
            let c = state.chain.read().unwrap();
            let blocks = model::block_summaries(&c, limit);
            write_json(
                &mut stream,
                200,
                &serde_json::json!({ "blocks": blocks, "count": blocks.len() }),
            )
        }
        ("GET", p) if p.starts_with("/api/v1/blocks/") => {
            let h = &p["/api/v1/blocks/".len()..];
            let height: u64 = match h.parse() {
                Ok(v) => v,
                Err(_) => {
                    return write_json(&mut stream, 400, &serde_json::json!({"error":"bad height"}))
                }
            };
            let c = state.chain.read().unwrap();
            match model::find_block(&c, height) {
                Some(b) => write_json(&mut stream, 200, &b),
                None => write_json(
                    &mut stream,
                    404,
                    &serde_json::json!({"error":"block not found"}),
                ),
            }
        }
        ("GET", "/api/v1/accounts") => {
            let limit = query_param(query, "limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(100usize)
                .min(1000);
            let min_bal = query_param(query, "min_balance")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0u64);
            let c = state.chain.read().unwrap();
            let accounts = model::list_accounts(&c, limit, min_bal);
            write_json(
                &mut stream,
                200,
                &serde_json::json!({ "accounts": accounts, "count": accounts.len() }),
            )
        }
        ("GET", p) if p.starts_with("/api/v1/accounts/") => {
            let id = &p["/api/v1/accounts/".len()..];
            let id = urlencoding_decode(id);
            let c = state.chain.read().unwrap();
            match model::resolve_account_query(&id, &c) {
                Some(a) => write_json(&mut stream, 200, &a),
                None => write_json(
                    &mut stream,
                    404,
                    &serde_json::json!({"error":"account not found"}),
                ),
            }
        }
        ("GET", "/api/v1/search") => {
            let q = query_param(query, "q").unwrap_or_default();
            let c = state.chain.read().unwrap();
            let res = model::search(&q, &c);
            write_json(&mut stream, 200, &res)
        }
        ("GET", "/api/v1/validators") => {
            let c = state.chain.read().unwrap();
            let vals: Vec<_> = c
                .validators
                .iter()
                .enumerate()
                .map(|(i, pk)| {
                    let sid = meshchain_proto::address::short_id(pk);
                    serde_json::json!({
                        "index": i,
                        "pubkey_hex": hex::encode(pk),
                        "short_id_hex": hex::encode(sid),
                        "mesh_name": meshchain_proto::address::mesh_name(&sid),
                    })
                })
                .collect();
            write_json(&mut stream, 200, &serde_json::json!({ "validators": vals }))
        }
        ("GET", "/api/v1/auth/mode") => write_json(
            &mut stream,
            200,
            &serde_json::json!({
                "mode": format!("{:?}", state.auth_mode).to_lowercase(),
                "mesh2fa_enforced": state.auth_mode == AuthMode::Mesh2fa,
                "note": "Internet open for testnet. Switch to --auth mesh2fa later for mesh identity gate."
            }),
        ),
        ("GET", "/api/v1/auth/challenge") => {
            let ch = auth::issue_challenge(now(), 300);
            if let Ok(mut g) = CHALLENGES.lock() {
                g.retain(|(_, _, exp)| *exp > now());
                g.push((ch.challenge_id.clone(), ch.message.clone(), ch.expires_unix));
            }
            write_json(&mut stream, 200, &ch)
        }
        ("POST", "/api/v1/auth/verify") => {
            let resp: auth::MeshChallengeResponse = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    return write_json(
                        &mut stream,
                        400,
                        &serde_json::json!({"error": format!("bad json: {e}")}),
                    );
                }
            };
            let msg = {
                let g = CHALLENGES.lock().unwrap();
                g.iter()
                    .find(|(id, _, exp)| id == &resp.challenge_id && *exp > now())
                    .map(|(_, m, _)| m.clone())
            };
            let Some(message) = msg else {
                return write_json(
                    &mut stream,
                    400,
                    &serde_json::json!({"error": "unknown or expired challenge"}),
                );
            };
            match auth::verify_challenge(&resp, &message) {
                Ok(()) => {
                    // Future: issue session cookie / JWT bound to mesh short id
                    let b = hex::decode(resp.pubkey_hex.trim()).unwrap_or_default();
                    let mut pk = [0u8; 32];
                    if b.len() == 32 {
                        pk.copy_from_slice(&b);
                    }
                    let sid = meshchain_proto::address::short_id(&pk);
                    write_json(
                        &mut stream,
                        200,
                        &serde_json::json!({
                            "ok": true,
                            "mesh_name": meshchain_proto::address::mesh_name(&sid),
                            "short_id_hex": hex::encode(sid),
                            "session": "stub-session-token",
                            "note": "Session issuance is stubbed; wire cookies when enforcing mesh2fa."
                        }),
                    )
                }
                Err(e) => write_json(
                    &mut stream,
                    401,
                    &serde_json::json!({"error": e.to_string()}),
                ),
            }
        }
        ("GET", "/favicon.ico") => write_response(&mut stream, 204, "text/plain", "", &[]),
        _ => write_json(
            &mut stream,
            404,
            &serde_json::json!({
                "error": "not found",
                "paths": [
                    "/","/api/v1/status","/api/v1/chain_state","/api/v1/network",
                    "/api/v1/blocks","/api/v1/accounts",
                    "/api/v1/search?q=","/api/v1/validators",
                    "/api/v1/auth/challenge","/api/v1/auth/verify"
                ]
            }),
        ),
    }
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(urlencoding_decode(v));
            }
        }
    }
    None
}

fn urlencoding_decode(s: &str) -> String {
    // minimal decode
    let mut out = String::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        if b[i] == b'+' {
            out.push(' ');
        } else {
            out.push(b[i] as char);
        }
        i += 1;
    }
    out
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, status: u16, val: &T) -> Result<()> {
    let body = serde_json::to_string_pretty(val)?;
    write_response(
        stream,
        status,
        "application/json; charset=utf-8",
        &body,
        &[("Access-Control-Allow-Origin", "*")],
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
    extra: &[(&str, &str)],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    Ok(())
}
