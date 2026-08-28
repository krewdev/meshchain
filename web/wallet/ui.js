import * as W from "./wallet.js";

function el(html) {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

function logBox() {
  return document.getElementById("walletLog");
}

function log(msg) {
  const box = logBox();
  if (!box) return;
  const line = typeof msg === "string" ? msg : JSON.stringify(msg, null, 2);
  box.textContent = line;
}

function endpoints() {
  return {
    faucet: (document.getElementById("epFaucet")?.value || W.DEFAULTS.faucet).trim(),
    scanner: (document.getElementById("epScanner")?.value || W.DEFAULTS.scanner).trim(),
    radio: (document.getElementById("epRadio")?.value || W.DEFAULTS.radio).trim(),
  };
}

function pathMode() {
  const checked = document.querySelector('input[name="path"]:checked');
  return checked ? checked.value : "net";
}

async function paintAccount(root, file) {
  const info = await W.describeWallet(file);
  root.querySelector("#wName").textContent = info.mesh_name;
  root.querySelector("#wHex").textContent = info.short_id_hex;
  root.querySelector("#wPk").textContent = info.public_hex;
  return info;
}

async function refreshBalance(file) {
  const info = await W.describeWallet(file);
  const ep = endpoints();
  const path = pathMode();
  try {
    if (path === "radio") {
      const j = await W.radioBalance(ep.radio, info.mesh_name);
      document.getElementById("wBal").textContent =
        (j.balance_tmesh != null ? j.balance_tmesh : (j.balance || 0) / 1e6) + " tMESH";
      document.getElementById("wNonce").textContent = String(j.nonce ?? "—");
      document.getElementById("wOnchain").textContent = j.found ? "yes (radio)" : "not on chain yet";
      log("Radio balance\n" + JSON.stringify(j, null, 2));
      return j;
    }
    const acc = await W.scannerAccount(ep.scanner, info.mesh_name);
    document.getElementById("wBal").textContent = (acc.balance_tmesh ?? acc.balance / 1e6) + " tMESH";
    document.getElementById("wNonce").textContent = String(acc.nonce ?? 0);
    document.getElementById("wOnchain").textContent = "yes (net)";
    log("Net balance\n" + JSON.stringify(acc, null, 2));
    return acc;
  } catch (e) {
    document.getElementById("wBal").textContent = "—";
    document.getElementById("wNonce").textContent = "—";
    document.getElementById("wOnchain").textContent = "unknown";
    log(String(e.message || e));
    return null;
  }
}

function render(root) {
  root.innerHTML = "";
  const panel = el(`
    <div class="wallet-shell">
      <div class="wallet-path">
        <label><input type="radio" name="path" value="net" checked /> Net (public seed)</label>
        <label><input type="radio" name="path" value="radio" /> Radio (local relay)</label>
      </div>
      <div class="live-stats" id="pathStatus">
        <div class="live-stats-head">
          <span class="live-dot" aria-hidden="true"></span>
          <span class="live-stats-label">Path</span>
          <span class="live-stats-meta" id="pathMeta">net · public faucet + scanner</span>
        </div>
        <p class="wallet-hint">
          <strong>Net</strong> uses HTTPS faucet + scanner, then submits signed txs to the seed.
          <strong>Radio</strong> talks to a local <code>mesh_radio_relay</code> HTTP port
          (<code>127.0.0.1:9299</code>) which bridges LoRa ↔ validators.
        </p>
      </div>

      <div class="wallet-grid">
        <div class="panel form-panel">
          <h3>1. Create / load</h3>
          <p class="wallet-hint">Ed25519 spend key. Same JSON shape as <code>mesh new-wallet</code>.</p>
          <div class="cta-row">
            <button class="btn btn-primary" id="btnCreate" type="button">Create wallet</button>
            <button class="btn btn-ghost" id="btnDownload" type="button">Download JSON</button>
            <label class="btn btn-ghost" for="fileIn">Import JSON</label>
            <input id="fileIn" type="file" accept="application/json,.json" hidden />
            <button class="btn btn-ghost" id="btnForget" type="button">Forget browser copy</button>
          </div>
          <div class="wallet-id">
            <div><span class="form-label">Mesh name</span><code id="wName">—</code></div>
            <div><span class="form-label">Short id</span><code id="wHex">—</code></div>
            <div><span class="form-label">Public key</span><code id="wPk" class="break">—</code></div>
          </div>
        </div>

        <div class="panel form-panel">
          <h3>2. Register + faucet</h3>
          <p class="wallet-hint">First drip sends your pubkey so the minter can create the account. Register also submits a signed on-chain bind.</p>
          <div class="live-stats-grid" style="margin-bottom:1rem">
            <div class="live-stat"><span class="live-stat-value" id="wBal">—</span><span class="live-stat-key">Balance</span></div>
            <div class="live-stat"><span class="live-stat-value" id="wNonce">—</span><span class="live-stat-key">Nonce</span></div>
            <div class="live-stat"><span class="live-stat-value" id="wOnchain">—</span><span class="live-stat-key">Registered</span></div>
            <div class="live-stat"><span class="live-stat-value" id="fDrip">—</span><span class="live-stat-key">Faucet drip</span></div>
          </div>
          <div class="cta-row">
            <button class="btn btn-primary" id="btnFaucet" type="button">Claim faucet</button>
            <button class="btn btn-ghost" id="btnRegister" type="button">Register on-chain</button>
            <button class="btn btn-ghost" id="btnRefresh" type="button">Refresh balance</button>
          </div>
        </div>
      </div>

      <div class="panel form-panel" style="margin-top:1rem">
        <h3>3. Send tMESH</h3>
        <label class="form-label" for="toName">To (mesh name or hex short id)</label>
        <input id="toName" class="form-input" placeholder="M3SQRT-XTA1Y-ZJ6" autocomplete="off" spellcheck="false" />
        <div class="wallet-send-row">
          <div>
            <label class="form-label" for="amt">Amount (tMESH)</label>
            <input id="amt" class="form-input" value="1" inputmode="decimal" />
          </div>
          <div>
            <label class="form-label" for="fee">Priority fee</label>
            <input id="fee" class="form-input" value="0" inputmode="decimal" />
          </div>
        </div>
        <button class="btn btn-primary" id="btnSend" type="button" style="width:100%;border:none">Send</button>
      </div>

      <details class="faucet-advanced">
        <summary>Endpoints</summary>
        <label class="form-label" for="epFaucet">Faucet (net)</label>
        <input id="epFaucet" class="form-input" value="${W.DEFAULTS.faucet}" />
        <label class="form-label" for="epScanner">Scanner (net)</label>
        <input id="epScanner" class="form-input" value="${W.DEFAULTS.scanner}" />
        <label class="form-label" for="epRadio">Radio relay HTTP</label>
        <input id="epRadio" class="form-input" value="${W.DEFAULTS.radio}" />
      </details>
      <pre id="walletLog" class="form-output">Create a wallet to start.</pre>
    </div>
  `);
  root.appendChild(panel);

  let file = W.loadStoredWallet();

  const syncId = async () => {
    if (!file) {
      panel.querySelector("#wName").textContent = "—";
      panel.querySelector("#wHex").textContent = "—";
      panel.querySelector("#wPk").textContent = "—";
      return null;
    }
    return paintAccount(panel, file);
  };

  const setPathMeta = () => {
    const meta = document.getElementById("pathMeta");
    const box = document.getElementById("pathStatus");
    if (pathMode() === "radio") {
      meta.textContent = "radio · " + endpoints().radio;
      box.classList.add("is-stale");
      box.classList.remove("is-live");
    } else {
      meta.textContent = "net · faucet + scanner";
      box.classList.add("is-live");
      box.classList.remove("is-stale");
    }
  };

  panel.querySelectorAll('input[name="path"]').forEach((r) => {
    r.addEventListener("change", async () => {
      setPathMeta();
      if (file) await refreshBalance(file);
    });
  });

  document.getElementById("btnCreate").onclick = async () => {
    file = W.generateWallet();
    W.storeWallet(file);
    await syncId();
    W.downloadWallet(file, "mesh-wallet.json");
    log("Wallet created and saved in this browser.\nDownload the JSON — it is the only backup.\nMesh name: " + (await W.describeWallet(file)).mesh_name);
  };

  document.getElementById("btnDownload").onclick = () => {
    if (!file) return log("No wallet in this browser.");
    W.downloadWallet(file, "mesh-wallet.json");
  };

  document.getElementById("fileIn").onchange = async (ev) => {
    const f = ev.target.files?.[0];
    if (!f) return;
    try {
      const j = JSON.parse(await f.text());
      if (!j.secret_hex || !j.public_hex) throw new Error("need secret_hex + public_hex");
      file = { secret_hex: j.secret_hex, public_hex: j.public_hex };
      W.storeWallet(file);
      await syncId();
      await refreshBalance(file);
      log("Imported " + (await W.describeWallet(file)).mesh_name);
    } catch (e) {
      log("Import failed: " + e.message);
    }
  };

  document.getElementById("btnForget").onclick = () => {
    W.clearStoredWallet();
    file = null;
    syncId();
    log("Browser copy cleared. Your download JSON still works if you kept it.");
  };

  document.getElementById("btnRefresh").onclick = async () => {
    if (!file) return log("Create or import a wallet first.");
    await refreshBalance(file);
  };

  document.getElementById("btnFaucet").onclick = async () => {
    if (!file) return log("Create a wallet first.");
    const info = await W.describeWallet(file);
    log("Requesting faucet drip…");
    try {
      const j = await W.faucetDrip(endpoints().faucet, info.mesh_name, info.public_hex);
      log("Faucet sent " + (j.amount_tmesh ?? 100) + " tMESH to " + info.mesh_name + "\n" + JSON.stringify(j, null, 2));
      setTimeout(() => refreshBalance(file), 2500);
    } catch (e) {
      log("Faucet: " + e.message);
    }
  };

  document.getElementById("btnRegister").onclick = async () => {
    if (!file) return log("Create a wallet first.");
    const info = await W.describeWallet(file);
    let nonce = 0;
    try {
      const acc = await W.scannerAccount(endpoints().scanner, info.mesh_name);
      nonce = acc.nonce || 0;
    } catch {
      nonce = 0;
    }
    const tx = W.buildRegisterTx(file, nonce, info);
    log("Submitting Register via " + pathMode() + "…");
    try {
      const j = await W.submitTx(endpoints(), tx, pathMode());
      log("Registered " + info.mesh_name + "\n" + JSON.stringify(j, null, 2));
      setTimeout(() => refreshBalance(file), 2500);
    } catch (e) {
      log(
        "Register submit: " +
          e.message +
          "\nFaucet drip with your public key also creates the account if the seed faucet is up."
      );
    }
  };

  document.getElementById("btnSend").onclick = async () => {
    if (!file) return log("Create a wallet first.");
    const info = await W.describeWallet(file);
    let to8;
    try {
      to8 = W.parseRecipient(document.getElementById("toName").value);
    } catch (e) {
      return log(e.message);
    }
    let amount;
    let fee;
    try {
      amount = W.parseAmountTmesh(document.getElementById("amt").value);
      const feeRaw = document.getElementById("fee").value.trim();
      fee = feeRaw && Number(feeRaw) !== 0 ? W.parseAmountTmesh(feeRaw) : 0;
    } catch (e) {
      return log(e.message);
    }
    let nonce = 0;
    try {
      if (pathMode() === "radio") {
        const b = await W.radioBalance(endpoints().radio, info.mesh_name);
        nonce = Number(b.nonce || 0);
      } else {
        const acc = await W.scannerAccount(endpoints().scanner, info.mesh_name);
        nonce = Number(acc.nonce || 0);
      }
    } catch {
      return log("Cannot read nonce — register / faucet first so the account exists.");
    }
    const tx = W.buildTransferTx(file, info, to8, amount, nonce, fee);
    log("Sending " + amount / 1e6 + " tMESH via " + pathMode() + " (nonce " + nonce + ")…");
    try {
      const j = await W.submitTx(endpoints(), tx, pathMode());
      log("Submitted.\n" + JSON.stringify(j, null, 2));
      setTimeout(() => refreshBalance(file), 3000);
    } catch (e) {
      log(e.message);
    }
  };

  (async () => {
    setPathMeta();
    await syncId();
    try {
      const info = await W.faucetInfo(endpoints().faucet);
      document.getElementById("fDrip").textContent = (info.amount_tmesh ?? 100) + " tMESH";
    } catch {
      document.getElementById("fDrip").textContent = "offline";
    }
    if (file) await refreshBalance(file);
  })();
}

function bindNav() {
  const toggle = document.getElementById("navToggle");
  const nav = document.getElementById("navLinks");
  if (!toggle || !nav) return;
  toggle.addEventListener("click", () => {
    const open = nav.classList.toggle("open");
    toggle.setAttribute("aria-expanded", open);
  });
}

const root = document.getElementById("wallet-root");
if (root) {
  bindNav();
  render(root);
}

export function mountWallet(selector) {
  const node = document.querySelector(selector);
  if (node) render(node);
}
