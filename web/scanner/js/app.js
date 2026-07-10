/**
 * MeshChain public scanner (Vercel static).
 * Loads /scanner/data/chain_state.json snapshot.
 * Live mode optional: ?api=https://host:8787 uses Rust scanner API when available.
 */
import { meshNameFromShortHex, tipHashHex, bytesToHex } from "./meshname.js";

const params = new URLSearchParams(location.search);
const LIVE_API = (params.get("api") || "").replace(/\/$/, "");
const DATA_BASE = "/scanner/data";

const $ = (id) => document.getElementById(id);

function fmtMesh(n) {
  return (Number(n) / 1e6).toLocaleString(undefined, { maximumFractionDigits: 6 });
}

async function loadJson(url) {
  const r = await fetch(url, { cache: "no-cache" });
  if (!r.ok) throw new Error(`${url} → ${r.status}`);
  return r.json();
}

async function getStatus(chain, meta) {
  if (LIVE_API) {
    try {
      return await loadJson(`${LIVE_API}/api/v1/status`);
    } catch (_) {
      /* fall through to snapshot */
    }
  }
  return {
    ok: true,
    service: "meshchain-scanner-vercel",
    auth_mode: "open",
    chain_id: chain.chain_id,
    height: chain.height,
    tip_hash_hex: tipHashHex(chain.tip_hash),
    total_supply: chain.total_supply,
    total_supply_tmesh: chain.total_supply / 1e6,
    account_count: Object.keys(chain.accounts || {}).length,
    block_count: (chain.applied || []).length,
    validators: (chain.validators || []).length,
    is_testnet: true,
    warning: "TESTNET ONLY — tMESH has no cash value",
    snapshot: meta,
    mesh_2fa: {
      enforced: false,
      status: "available_not_enforced",
      note: "Use live Rust scanner with --auth mesh2fa later",
    },
  };
}

function accountRows(chain, limit = 50) {
  const rows = Object.entries(chain.accounts || {}).map(([hex, a]) => {
    let mesh_name = hex;
    try {
      mesh_name = meshNameFromShortHex(hex);
    } catch (_) {}
    const pk = Array.isArray(a.pubkey) ? bytesToHex(Uint8Array.from(a.pubkey)) : a.pubkey_hex || "";
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

function blockRows(chain, limit = 30) {
  const applied = chain.applied || [];
  return applied
    .slice()
    .reverse()
    .slice(0, limit)
    .map((b) => ({
      height: b.height,
      hash_hex: b.hash_hex,
      tx_count: b.tx_count,
    }));
}

function validatorRows(chain) {
  return (chain.validators || []).map((pkArr, i) => {
    const pubkey_hex = Array.isArray(pkArr) ? bytesToHex(Uint8Array.from(pkArr)) : String(pkArr);
    // short id = first 8 of sha256(pubkey) — approximate display via account map lookup
    let mesh_name = `validator-${i}`;
    let short_id_hex = "";
    for (const [hex, acc] of Object.entries(chain.accounts || {})) {
      const apk = Array.isArray(acc.pubkey) ? bytesToHex(Uint8Array.from(acc.pubkey)) : "";
      if (apk === pubkey_hex) {
        short_id_hex = hex;
        try {
          mesh_name = meshNameFromShortHex(hex);
        } catch (_) {}
        break;
      }
    }
    return { index: i, pubkey_hex, mesh_name, short_id_hex };
  });
}

function search(q, chain) {
  q = (q || "").trim();
  if (!q) return { kind: "empty", message: "Enter a mesh name, short hex, or block height" };
  if (/^\d+$/.test(q)) {
    const h = Number(q);
    const b = (chain.applied || []).find((x) => x.height === h);
    if (b) return { kind: "block", block: b };
  }
  const qUp = q.toUpperCase().replace(/-/g, "");
  for (const row of accountRows(chain, 10000)) {
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
  $("authBadge").textContent = s.mesh_2fa?.enforced
    ? "auth: mesh2fa"
    : "auth: open (public · Vercel snapshot)";
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
        `<div class="stat"><div class="l">${l}</div><div class="v">${escapeHtml(String(v))}</div></div>`
    )
    .join("");
  if (s.snapshot?.snapshot_unix) {
    $("snapMeta").textContent =
      "Snapshot: " +
      new Date(s.snapshot.snapshot_unix * 1000).toISOString() +
      (LIVE_API ? ` · live API: ${LIVE_API}` : " · static Vercel data (re-deploy to refresh)");
  }
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
      <td>${a.balance_tmesh.toFixed(6)} tMESH</td>
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

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

function fmtMesh(n) {
  return fmtMeshNumber(n);
}
function fmtMeshNumber(n) {
  return (Number(n) / 1e6).toLocaleString(undefined, { maximumFractionDigits: 6 });
}

let CHAIN = null;

function doSearch() {
  const q = $("q").value.trim();
  const out = $("searchOut");
  if (!CHAIN) return;
  const r = search(q, CHAIN);
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
}

async function loadAll() {
  try {
    const [chain, meta] = await Promise.all([
      loadJson(`${DATA_BASE}/chain_state.json`),
      loadJson(`${DATA_BASE}/meta.json`).catch(() => ({})),
    ]);
    CHAIN = chain;
    const status = await getStatus(chain, meta);
    status.snapshot = meta;
    renderStats(status);
    renderBlocks(blockRows(chain));
    renderAccounts(accountRows(chain));
    renderValidators(validatorRows(chain));
    $("errBanner").textContent = "";
  } catch (e) {
    console.error(e);
    $("errBanner").textContent = String(e);
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
      note: "Mesh 2FA challenge requires the live Rust scanner (--auth mesh2fa).",
      vercel: "Static Vercel scanner is public (open). Point ?api=https://your-host:8787 for live 2FA.",
      example: "https://meshchain-sigma.vercel.app/scanner/?api=https://YOUR_VPS:8787",
    },
    null,
    2
  );
});

loadAll();
