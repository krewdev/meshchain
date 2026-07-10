/**
 * MeshChain public scanner (Vercel).
 *
 * Auto-update strategies (in priority order):
 * 1. ?api=https://host:8787  — force live Rust scanner
 * 2. config.json live_api    — always-on host (no Vercel redeploy)
 * 3. Snapshot JSON under /scanner/data/ — updated via GitHub Action / sync script
 */
import { meshNameFromShortHex, tipHashHex, bytesToHex } from "./meshname.js";

const params = new URLSearchParams(location.search);
const DATA_BASE = "/scanner/data";

const $ = (id) => document.getElementById(id);

let LIVE_API = (params.get("api") || "").replace(/\/$/, "");
let POLL_SECS = 15;
let CHAIN = null;
let MODE = "snapshot"; // snapshot | live

async function loadJson(url) {
  const r = await fetch(url, { cache: "no-cache" });
  if (!r.ok) throw new Error(`${url} → ${r.status}`);
  return r.json();
}

async function loadConfig() {
  try {
    const cfg = await loadJson(`${DATA_BASE}/config.json`);
    if (!LIVE_API && cfg.live_api) {
      LIVE_API = String(cfg.live_api).replace(/\/$/, "");
    }
    if (cfg.poll_secs && Number(cfg.poll_secs) > 0) {
      POLL_SECS = Number(cfg.poll_secs);
    }
    return cfg;
  } catch {
    return {};
  }
}

async function tryLive(path) {
  if (!LIVE_API) return null;
  try {
    return await loadJson(`${LIVE_API}${path}`);
  } catch (e) {
    console.warn("live API failed", path, e);
    return null;
  }
}

function fmtMesh(n) {
  return (Number(n) / 1e6).toLocaleString(undefined, { maximumFractionDigits: 6 });
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c])
  );
}

function accountRowsFromChain(chain, limit = 50) {
  const rows = Object.entries(chain.accounts || {}).map(([hex, a]) => {
    let mesh_name = hex;
    try {
      mesh_name = meshNameFromShortHex(hex);
    } catch (_) {}
    const pk = Array.isArray(a.pubkey)
      ? bytesToHex(Uint8Array.from(a.pubkey))
      : a.pubkey_hex || "";
    return {
      short_id_hex: hex,
      mesh_name,
      balance: a.balance || 0,
      balance_tmesh: (a.balance || 0) / 1e6,
      nonce: a.nonce || 0,
      has_cold_key: !!(a.pq_pk && a.pq_pk.length),
      pubkey_hex: pk,
    };
  });
  rows.sort((a, b) => b.balance - a.balance);
  return rows.slice(0, limit);
}

function blockRowsFromChain(chain, limit = 30) {
  return (chain.applied || [])
    .slice()
    .reverse()
    .slice(0, limit)
    .map((b) => ({
      height: b.height,
      hash_hex: b.hash_hex,
      tx_count: b.tx_count,
    }));
}

function validatorRowsFromChain(chain) {
  return (chain.validators || []).map((pkArr, i) => {
    const pubkey_hex = Array.isArray(pkArr)
      ? bytesToHex(Uint8Array.from(pkArr))
      : String(pkArr);
    let mesh_name = `validator-${i}`;
    for (const [hex, acc] of Object.entries(chain.accounts || {})) {
      const apk = Array.isArray(acc.pubkey)
        ? bytesToHex(Uint8Array.from(acc.pubkey))
        : "";
      if (apk === pubkey_hex) {
        try {
          mesh_name = meshNameFromShortHex(hex);
        } catch (_) {}
        break;
      }
    }
    return { index: i, pubkey_hex, mesh_name };
  });
}

function searchLocal(q, chain) {
  q = (q || "").trim();
  if (!q) return { kind: "empty", message: "Enter a mesh name, short hex, or block height" };
  if (/^\d+$/.test(q)) {
    const h = Number(q);
    const b = (chain.applied || []).find((x) => x.height === h);
    if (b) return { kind: "block", block: b };
  }
  const qUp = q.toUpperCase().replace(/-/g, "");
  for (const row of accountRowsFromChain(chain, 10000)) {
    const nameUp = row.mesh_name.toUpperCase().replace(/-/g, "");
    if (
      row.short_id_hex === q.toLowerCase() ||
      nameUp === qUp ||
      nameUp.includes(qUp) ||
      row.short_id_hex.includes(q.toLowerCase())
    ) {
      return { kind: "account", account: row };
    }
  }
  return { kind: "not_found", message: "No matching account or block" };
}

function renderStats(s) {
  $("authBadge").textContent =
    MODE === "live"
      ? `live · ${s.auth_mode || "open"}`
      : "snapshot · open (public)";
  $("stats").innerHTML = [
    ["Height", s.height],
    ["Supply (tMESH)", fmtMesh(s.total_supply)],
    ["Accounts", s.account_count],
    ["Blocks", s.block_count],
    ["Validators", s.validators],
    ["Chain", s.chain_id],
  ]
    .map(
      ([l, v]) =>
        `<div class="stat"><div class="l">${l}</div><div class="v">${escapeHtml(
          String(v)
        )}</div></div>`
    )
    .join("");
}

function renderBlocks(blocks) {
  if (!blocks.length) {
    $("blocks").textContent = "No blocks yet.";
    return;
  }
  $("blocks").innerHTML = `<table>
    <tr><th>Height</th><th>Txs</th><th>Hash</th></tr>
    ${blocks
      .map(
        (b) => `<tr>
      <td><a href="#" data-q="${b.height}">${b.height}</a></td>
      <td>${b.tx_count}</td>
      <td><code>${(b.hash_hex || "").slice(0, 18)}…</code></td>
    </tr>`
      )
      .join("")}
  </table>`;
}

function renderAccounts(rows) {
  $("accounts").innerHTML = `<table>
    <tr><th>Mesh name</th><th>Balance</th><th>Nonce</th><th>Cold</th></tr>
    ${
      rows
        .map(
          (a) => `<tr>
      <td><a href="#" data-q="${a.mesh_name}"><code>${a.mesh_name}</code></a></td>
      <td>${Number(a.balance_tmesh).toFixed(6)} tMESH</td>
      <td>${a.nonce}</td>
      <td>${a.has_cold_key ? "yes" : "—"}</td>
    </tr>`
        )
        .join("") || "<tr><td colspan='4'>No accounts</td></tr>"
    }
  </table>`;
}

function renderValidators(rows) {
  $("validators").innerHTML = `<table>
    <tr><th>#</th><th>Mesh name</th><th>Pubkey</th></tr>
    ${rows
      .map(
        (v) => `<tr>
      <td>${v.index}</td>
      <td><code>${v.mesh_name}</code></td>
      <td><code>${(v.pubkey_hex || "").slice(0, 18)}…</code></td>
    </tr>`
      )
      .join("")}
  </table>`;
}

async function loadLive() {
  const status = await tryLive("/api/v1/status");
  if (!status) return false;

  const [blocksRes, accountsRes, validatorsRes] = await Promise.all([
    tryLive("/api/v1/blocks?limit=30"),
    tryLive("/api/v1/accounts?limit=50"),
    tryLive("/api/v1/validators"),
  ]);
  if (!blocksRes || !accountsRes) return false;

  MODE = "live";
  CHAIN = null; // search uses live API
  renderStats(status);
  renderBlocks(blocksRes.blocks || []);
  renderAccounts(accountsRes.accounts || []);
  if (validatorsRes?.validators) renderValidators(validatorsRes.validators);

  $("snapMeta").textContent =
    `Live · ${LIVE_API} · auto-refresh every ${POLL_SECS}s · mesh2fa: ${
      status.mesh_2fa?.status || "n/a"
    }`;
  $("errBanner").textContent = "";
  return true;
}

async function loadSnapshot() {
  const [chain, meta] = await Promise.all([
    loadJson(`${DATA_BASE}/chain_state.json`),
    loadJson(`${DATA_BASE}/meta.json`).catch(() => ({})),
  ]);
  CHAIN = chain;
  MODE = "snapshot";
  const status = {
    ok: true,
    auth_mode: "open",
    chain_id: chain.chain_id,
    height: chain.height,
    total_supply: chain.total_supply,
    account_count: Object.keys(chain.accounts || {}).length,
    block_count: (chain.applied || []).length,
    validators: (chain.validators || []).length,
  };
  renderStats(status);
  renderBlocks(blockRowsFromChain(chain));
  renderAccounts(accountRowsFromChain(chain));
  renderValidators(validatorRowsFromChain(chain));

  const when = meta.snapshot_unix
    ? new Date(meta.snapshot_unix * 1000).toISOString()
    : "unknown";
  $("snapMeta").textContent =
    `Snapshot ${when} · auto-refresh page every ${POLL_SECS}s · ` +
    (LIVE_API
      ? `live API unreachable (${LIVE_API}), using snapshot`
      : "set live_api in config.json for real-time updates without redeploy");
  $("errBanner").textContent = "";
}

async function loadAll() {
  try {
    await loadConfig();
    const liveOk = await loadLive();
    if (!liveOk) await loadSnapshot();
  } catch (e) {
    console.error(e);
    $("errBanner").textContent = String(e);
  }
}

async function doSearch() {
  const q = $("q").value.trim();
  const out = $("searchOut");
  if (!q) {
    out.textContent = "";
    return;
  }
  try {
    if (MODE === "live" && LIVE_API) {
      const r = await loadJson(
        `${LIVE_API}/api/v1/search?q=${encodeURIComponent(q)}`
      );
      if (r.kind === "account" && r.account) {
        const a = r.account;
        out.innerHTML = `<span class="ok">Account</span> <code>${a.mesh_name}</code><br>
          Balance: <strong>${Number(a.balance_tmesh).toFixed(6)} tMESH</strong> · nonce ${a.nonce}<br>
          Hex: <code>${a.short_id_hex}</code>`;
      } else if (r.kind === "block" && r.block) {
        const b = r.block;
        out.innerHTML = `<span class="ok">Block</span> #${b.height} · ${b.tx_count} tx · <code>${b.hash_hex}</code>`;
      } else {
        out.innerHTML = `<span class="err">${r.message || "Not found"}</span>`;
      }
      return;
    }
    const r = searchLocal(q, CHAIN);
    if (r.kind === "account" && r.account) {
      const a = r.account;
      out.innerHTML = `<span class="ok">Account</span> <code>${a.mesh_name}</code><br>
        Balance: <strong>${a.balance_tmesh.toFixed(6)} tMESH</strong> · nonce ${a.nonce}<br>
        Hex: <code>${a.short_id_hex}</code>`;
    } else if (r.kind === "block" && r.block) {
      const b = r.block;
      out.innerHTML = `<span class="ok">Block</span> #${b.height} · ${b.tx_count} tx · <code>${b.hash_hex}</code>`;
    } else {
      out.innerHTML = `<span class="err">${r.message || "Not found"}</span>`;
    }
  } catch (e) {
    out.innerHTML = `<span class="err">${escapeHtml(String(e))}</span>`;
  }
}

document.addEventListener("click", (ev) => {
  const a = ev.target.closest("a[data-q]");
  if (!a) return;
  ev.preventDefault();
  $("q").value = a.getAttribute("data-q");
  doSearch();
});

$("btnSearch").addEventListener("click", doSearch);
$("q").addEventListener("keydown", (e) => {
  if (e.key === "Enter") doSearch();
});
$("btnRefresh").addEventListener("click", loadAll);
$("btnChallenge").addEventListener("click", async () => {
  if (LIVE_API) {
    try {
      const c = await loadJson(`${LIVE_API}/api/v1/auth/challenge`);
      $("challenge").textContent = JSON.stringify(c, null, 2);
      return;
    } catch (_) {}
  }
  $("challenge").textContent = JSON.stringify(
    {
      how_to_auto_update: [
        "1) BEST: Run meshchain-scanner on a VPS and set web/scanner/data/config.json live_api",
        "2) Or open /scanner/?api=https://YOUR_HOST:8787",
        "3) Or GitHub Action syncs snapshot every N minutes (see .github/workflows/scanner-snapshot.yml)",
      ],
      mesh2fa: "Enable on live scanner with --auth mesh2fa",
    },
    null,
    2
  );
});

loadAll().then(() => {
  setInterval(loadAll, POLL_SECS * 1000);
});
