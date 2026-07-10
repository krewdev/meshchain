//! Embedded scanner web UI (single page).

pub const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>MeshChain Scanner — Testnet Explorer</title>
  <style>
    :root {
      --bg:#070b12; --card:#111827; --border:rgba(148,163,184,.15);
      --text:#e5eefc; --muted:#94a3b8; --accent:#38bdf8; --good:#34d399; --warn:#fbbf24;
    }
    *{box-sizing:border-box}
    body{margin:0;font-family:system-ui,sans-serif;background:radial-gradient(900px 400px at 10% -10%,rgba(56,189,248,.12),transparent),var(--bg);color:var(--text);min-height:100vh}
    a{color:var(--accent);text-decoration:none}
    .banner{background:linear-gradient(90deg,#854d0e,#a16207);color:#fffbeb;text-align:center;padding:.5rem 1rem;font-weight:600;font-size:.9rem}
    .wrap{max-width:1100px;margin:0 auto;padding:1rem 1.25rem 3rem}
    header{display:flex;flex-wrap:wrap;gap:1rem;justify-content:space-between;align-items:center;margin:1rem 0 1.5rem}
    h1{margin:0;font-size:1.5rem;letter-spacing:-.02em}
    .badge{font-size:.75rem;color:var(--good);border:1px solid rgba(52,211,153,.3);background:rgba(52,211,153,.1);padding:.2rem .55rem;border-radius:999px}
    .stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:.75rem;margin-bottom:1.25rem}
    .stat{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:.9rem}
    .stat .l{color:var(--muted);font-size:.75rem;text-transform:uppercase;letter-spacing:.04em}
    .stat .v{font-size:1.25rem;font-weight:700;margin-top:.25rem;word-break:break-all}
    .panel{background:var(--card);border:1px solid var(--border);border-radius:14px;padding:1rem;margin-bottom:1rem}
    .panel h2{margin:0 0 .75rem;font-size:1.05rem}
    input,button{font:inherit}
    .search{display:flex;gap:.5rem;flex-wrap:wrap}
    .search input{flex:1;min-width:200px;background:#0a0f18;border:1px solid var(--border);border-radius:10px;padding:.65rem .8rem;color:var(--text)}
    button{background:linear-gradient(135deg,#0ea5e9,#22c55e);border:0;border-radius:10px;padding:.65rem 1rem;font-weight:600;cursor:pointer;color:#041018}
    button.ghost{background:transparent;border:1px solid var(--border);color:var(--text)}
    table{width:100%;border-collapse:collapse;font-size:.9rem}
    th,td{text-align:left;padding:.55rem .4rem;border-bottom:1px solid var(--border);vertical-align:top}
    th{color:var(--muted);font-size:.75rem;text-transform:uppercase}
    code{font-family:ui-monospace,Menlo,monospace;font-size:.85em;background:rgba(148,163,184,.1);padding:.1rem .3rem;border-radius:4px}
    .muted{color:var(--muted);font-size:.9rem}
    .err{color:#f87171}
    .ok{color:var(--good)}
    .row-actions{display:flex;gap:.5rem;flex-wrap:wrap;margin-top:.75rem}
  </style>
</head>
<body>
  <div class="banner">TESTNET SCANNER · meshchain-testnet-1 · tMESH has no cash value · Internet open now · Mesh 2FA later</div>
  <div class="wrap">
    <header>
      <div>
        <h1>MeshChain Scanner</h1>
        <span class="badge" id="authBadge">loading…</span>
      </div>
      <div class="row-actions">
        <button class="ghost" type="button" onclick="loadAll()">Refresh</button>
        <a class="ghost" href="/api/v1/status" style="padding:.65rem 1rem;border:1px solid var(--border);border-radius:10px">API</a>
      </div>
    </header>

    <div class="stats" id="stats"></div>

    <div class="panel">
      <h2>Search</h2>
      <div class="search">
        <input id="q" placeholder="Mesh name (M4K7X-…), short hex, or block height" onkeydown="if(event.key==='Enter')doSearch()" />
        <button type="button" onclick="doSearch()">Search</button>
      </div>
      <div id="searchOut" class="muted" style="margin-top:.75rem"></div>
    </div>

    <div class="panel">
      <h2>Recent blocks</h2>
      <div id="blocks" class="muted">Loading…</div>
    </div>

    <div class="panel">
      <h2>Top accounts</h2>
      <div id="accounts" class="muted">Loading…</div>
    </div>

    <div class="panel">
      <h2>Validators</h2>
      <div id="validators" class="muted">Loading…</div>
    </div>

    <div class="panel">
      <h2>Mesh 2FA (later)</h2>
      <p class="muted">Scanner is <strong>internet-open</strong> for testnet. When you flip to <code>--auth mesh2fa</code>, clients must sign a challenge with their mesh wallet before browsing private routes.</p>
      <div class="row-actions">
        <button class="ghost" type="button" onclick="fetchChallenge()">Get challenge</button>
      </div>
      <pre id="challenge" class="muted" style="white-space:pre-wrap;font-size:.8rem"></pre>
    </div>

    <p class="muted">API base: <code>/api/v1</code> · Data reloads from validator <code>chain_state.json</code></p>
  </div>
  <script>
    async function j(path) {
      const r = await fetch(path);
      if (!r.ok) throw new Error(path + ' ' + r.status);
      return r.json();
    }
    function fmt(n) {
      return (Number(n) / 1e6).toLocaleString(undefined, { maximumFractionDigits: 6 });
    }
    async function loadStatus() {
      const s = await j('/api/v1/status');
      document.getElementById('authBadge').textContent =
        s.mesh_2fa.enforced ? 'auth: mesh2fa' : 'auth: open (public)';
      document.getElementById('stats').innerHTML = [
        ['Height', s.height],
        ['Supply (tMESH)', fmt(s.total_supply)],
        ['Accounts', s.account_count],
        ['Blocks', s.block_count],
        ['Validators', s.validators],
        ['Chain', s.chain_id],
      ].map(([l,v]) => `<div class="stat"><div class="l">${l}</div><div class="v">${v}</div></div>`).join('');
    }
    async function loadBlocks() {
      const d = await j('/api/v1/blocks?limit=30');
      if (!d.blocks.length) {
        document.getElementById('blocks').textContent = 'No blocks yet.';
        return;
      }
      document.getElementById('blocks').innerHTML = `<table>
        <tr><th>Height</th><th>Txs</th><th>Hash</th></tr>
        ${d.blocks.map(b => `<tr>
          <td><a href="#" onclick="showBlock(${b.height});return false">${b.height}</a></td>
          <td>${b.tx_count}</td>
          <td><code>${b.hash_hex.slice(0,16)}…</code></td>
        </tr>`).join('')}
      </table>`;
    }
    async function loadAccounts() {
      const d = await j('/api/v1/accounts?limit=50');
      document.getElementById('accounts').innerHTML = `<table>
        <tr><th>Mesh name</th><th>Balance</th><th>Nonce</th><th>Cold</th></tr>
        ${d.accounts.map(a => `<tr>
          <td><a href="#" onclick="showAccount('${a.mesh_name}');return false"><code>${a.mesh_name}</code></a></td>
          <td>${a.balance_tmesh.toFixed(6)} tMESH</td>
          <td>${a.nonce}</td>
          <td>${a.has_cold_key ? 'yes' : '—'}</td>
        </tr>`).join('') || '<tr><td colspan="4">No accounts</td></tr>'}
      </table>`;
    }
    async function loadValidators() {
      const d = await j('/api/v1/validators');
      document.getElementById('validators').innerHTML = `<table>
        <tr><th>#</th><th>Mesh name</th><th>Pubkey</th></tr>
        ${d.validators.map(v => `<tr>
          <td>${v.index}</td>
          <td><code>${v.mesh_name}</code></td>
          <td><code>${v.pubkey_hex.slice(0,16)}…</code></td>
        </tr>`).join('')}
      </table>`;
    }
    async function doSearch() {
      const q = document.getElementById('q').value.trim();
      const out = document.getElementById('searchOut');
      if (!q) { out.textContent = ''; return; }
      try {
        const r = await j('/api/v1/search?q=' + encodeURIComponent(q));
        if (r.kind === 'account' && r.account) {
          const a = r.account;
          out.innerHTML = `<span class="ok">Account</span> <code>${a.mesh_name}</code><br>
            Balance: <strong>${a.balance_tmesh.toFixed(6)} tMESH</strong> · nonce ${a.nonce}<br>
            Hex: <code>${a.short_id_hex}</code>`;
        } else if (r.kind === 'block' && r.block) {
          const b = r.block;
          out.innerHTML = `<span class="ok">Block</span> #${b.height} · ${b.tx_count} tx · <code>${b.hash_hex}</code>`;
        } else {
          out.innerHTML = `<span class="err">${r.message || 'Not found'}</span>`;
        }
      } catch (e) {
        out.innerHTML = `<span class="err">${e}</span>`;
      }
    }
    async function showBlock(h) {
      document.getElementById('q').value = String(h);
      doSearch();
    }
    async function showAccount(name) {
      document.getElementById('q').value = name;
      doSearch();
    }
    async function fetchChallenge() {
      const c = await j('/api/v1/auth/challenge');
      document.getElementById('challenge').textContent = JSON.stringify(c, null, 2);
    }
    async function loadAll() {
      try {
        await Promise.all([loadStatus(), loadBlocks(), loadAccounts(), loadValidators()]);
      } catch (e) {
        console.error(e);
        document.getElementById('stats').innerHTML = `<div class="stat"><div class="l">Error</div><div class="v err">${e}</div></div>`;
      }
    }
    loadAll();
    setInterval(loadAll, 10000);
  </script>
</body>
</html>
"##;
