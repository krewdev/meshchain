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

function promptPass(msg) {
  return window.prompt(msg || "Wallet passphrase");
}

async function unlockIfNeeded(file) {
  if (file?.secret_hex) return file;
  if (!file?.wrapped && !file?.locked) throw new Error("Create or import a wallet first.");
  const pass = promptPass("This key is wrapped. Enter passphrase:");
  if (!pass) throw new Error("passphrase required");
  return W.unwrapSecret(file, pass);
}

async function paintAccount(root, file) {
  const spend = file.secret_hex ? file : { public_hex: file.public_hex, secret_hex: "00".repeat(32) };
  let info;
  try {
    info = await W.describeWallet(file.secret_hex ? file : { secret_hex: "00".repeat(32), public_hex: file.public_hex });
    if (!file.secret_hex) {
      const pk = W.hexToBytes(file.public_hex);
      const sid = await W.shortIdFromPubkey(pk);
      info = {
        public_hex: file.public_hex,
        short_id_hex: W.bytesToHex(sid),
        mesh_name: W.meshNameFromShort(sid),
        short_id: sid,
        pubkey: pk,
      };
    }
  } catch {
    const pk = W.hexToBytes(file.public_hex);
    const sid = await W.shortIdFromPubkey(pk);
    info = {
      public_hex: file.public_hex,
      short_id_hex: W.bytesToHex(sid),
      mesh_name: W.meshNameFromShort(sid),
      short_id: sid,
      pubkey: pk,
    };
  }
  root.querySelector("#wName").textContent = info.mesh_name;
  root.querySelector("#wHex").textContent = info.short_id_hex;
  root.querySelector("#wPk").textContent = info.public_hex;
  drawQr(root.querySelector("#wQr"), info.mesh_name);
  return info;
}

function drawQr(node, text) {
  if (!node) return;
  if (typeof qrcode === "function") {
    try {
      const q = qrcode(0, "M");
      q.addData(text);
      q.make();
      node.innerHTML = q.createSvgTag(5, 2);
      node.setAttribute("title", text);
      return;
    } catch {
      /* fall through */
    }
  }
  node.innerHTML = `<code class="break">${text}</code>`;
}

async function refreshBalance(file) {
  const info = await paintAccount(document.getElementById("wallet-root") || document.body, file);
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

async function loadActivity(file) {
  const list = document.getElementById("wActivity");
  if (!list) return;
  try {
    const info = await W.describeWallet(
      file.secret_hex ? file : { secret_hex: "00".repeat(32), public_hex: file.public_hex }
    );
    const name = file.secret_hex
      ? info.mesh_name
      : W.meshNameFromShort(await W.shortIdFromPubkey(W.hexToBytes(file.public_hex)));
    const j = await W.scannerActivity(endpoints().scanner, name, 12);
    if (!j.txs?.length) {
      list.innerHTML = `<li class="wallet-hint">${j.note || "No archived txs yet. Faucet and send will show here after inclusion."}</li>`;
      return;
    }
    list.innerHTML = j.txs
      .map((t) => {
        const amt = t.amount_tmesh != null ? `${t.amount_tmesh} tMESH` : "";
        return `<li><span class="act-kind">${t.kind}</span> ${amt} <span class="act-h">h${t.height}</span></li>`;
      })
      .join("");
  } catch (e) {
    list.innerHTML = `<li class="wallet-hint">${e.message || "activity offline"}</li>`;
  }
}

async function probeRadio() {
  const meta = document.getElementById("pathMeta");
  const box = document.getElementById("pathStatus");
  const send = document.getElementById("btnSend");
  const hint = document.getElementById("radioCmd");
  if (pathMode() !== "radio") {
    meta.textContent = "net · faucet + scanner";
    box.classList.add("is-live");
    box.classList.remove("is-stale");
    if (send) send.disabled = false;
    if (hint) hint.hidden = true;
    return;
  }
  if (hint) hint.hidden = false;
  meta.textContent = "radio · probing " + endpoints().radio;
  try {
    const h = await W.radioHealth(endpoints().radio);
    if (h.ok) {
      meta.textContent = "radio · relay up · " + (h.listen || endpoints().radio);
      box.classList.add("is-live");
      box.classList.remove("is-stale");
      if (send) send.disabled = false;
    } else {
      throw new Error(h.error || "relay not ok");
    }
  } catch (e) {
    meta.textContent = "radio · relay down — start local HTTP on :9299";
    box.classList.add("is-stale");
    box.classList.remove("is-live");
    if (send) send.disabled = true;
    log(
      "Radio relay unreachable at " +
        endpoints().radio +
        "\n" +
        String(e.message || e) +
        "\n\nStart it from the repo:\n" +
        W.RADIO_START_HINT
    );
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
        <pre id="radioCmd" class="wallet-cmd" hidden>${W.RADIO_START_HINT}</pre>
      </div>

      <div class="wallet-grid">
        <div class="panel form-panel">
          <h3>1. Create / load</h3>
          <p class="wallet-hint">Ed25519 spend key. Same JSON shape as <code>mesh new-wallet</code>. Download before claiming faucet.</p>
          <div class="cta-row">
            <button class="btn btn-primary" id="btnCreate" type="button">Create wallet</button>
            <button class="btn btn-ghost" id="btnDownload" type="button">Download JSON</button>
            <label class="btn btn-ghost" for="fileIn">Import JSON</label>
            <input id="fileIn" type="file" accept="application/json,.json" hidden />
            <button class="btn btn-ghost" id="btnWrap" type="button">Wrap passphrase</button>
            <button class="btn btn-ghost" id="btnForget" type="button">Forget this key</button>
          </div>
          <label class="form-label" for="wSelect">Keys in this browser</label>
          <select id="wSelect" class="form-input"></select>
          <div class="wallet-id">
            <div><span class="form-label">Mesh name</span><code id="wName">—</code></div>
            <div><span class="form-label">Short id</span><code id="wHex">—</code></div>
            <div><span class="form-label">Public key</span><code id="wPk" class="break">—</code></div>
          </div>
          <div class="wallet-qr-wrap">
            <div id="wQr" class="wallet-qr" aria-label="Receive QR"></div>
            <button class="btn btn-ghost" id="btnCopy" type="button">Copy mesh name</button>
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
          <p class="wallet-hint" id="includeNote"></p>
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

      <div class="panel form-panel" style="margin-top:1rem">
        <h3>Activity</h3>
        <ul id="wActivity" class="wallet-activity"><li class="wallet-hint">Load a wallet to see archived txs.</li></ul>
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
      <pre id="walletLog" class="form-output">Create a wallet to start. Download the JSON — it is the only backup.</pre>
    </div>
  `);
  root.appendChild(panel);

  let file = W.loadStoredWallet();

  const fillSelect = () => {
    const sel = document.getElementById("wSelect");
    const list = W.listWallets();
    sel.innerHTML = list
      .map((w) => {
        const tag = w.wrapped ? " (wrapped)" : "";
        const short = w.public_hex.slice(0, 8);
        return `<option value="${w.public_hex}">${w.label || short}${tag}</option>`;
      })
      .join("");
    if (file?.public_hex) sel.value = file.public_hex;
    if (!list.length) sel.innerHTML = "<option value=''>no keys in this browser</option>";
  };

  const syncId = async () => {
    fillSelect();
    if (!file) {
      panel.querySelector("#wName").textContent = "—";
      panel.querySelector("#wHex").textContent = "—";
      panel.querySelector("#wPk").textContent = "—";
      panel.querySelector("#wQr").innerHTML = "";
      return null;
    }
    return paintAccount(panel, file);
  };

  const afterSubmit = async (info, before, label) => {
    const note = document.getElementById("includeNote");
    note.textContent = "Waiting for inclusion…";
    if (pathMode() === "radio") {
      setTimeout(() => refreshBalance(file), 2500);
      note.textContent = "Submitted over radio. Refresh balance after the next air ACK.";
      return;
    }
    const inc = await W.waitForInclusion(endpoints().scanner, info.mesh_name, before);
    if (inc.included) {
      note.textContent = `${label} included after ${inc.waited_ms} ms`;
      const url = W.scannerAccountUrl(endpoints().scanner, info.mesh_name);
      log(`${label} included.\nScanner: ${url}\n` + JSON.stringify(inc.account, null, 2));
    } else {
      note.textContent = `${label} submitted, not seen yet (${inc.error || "timeout"})`;
      log(`${label} not visible on scanner yet.\n` + JSON.stringify(inc, null, 2));
    }
    await refreshBalance(file);
    await loadActivity(file);
  };

  panel.querySelectorAll('input[name="path"]').forEach((r) => {
    r.addEventListener("change", async () => {
      await probeRadio();
      if (file) await refreshBalance(file);
    });
  });

  document.getElementById("wSelect").onchange = async (ev) => {
    const pk = ev.target.value;
    if (!pk) return;
    W.setActiveWallet(pk);
    file = W.loadStoredWallet();
    await syncId();
    if (file) {
      await refreshBalance(file);
      await loadActivity(file);
    }
  };

  document.getElementById("btnCreate").onclick = async () => {
    const created = W.generateWallet();
    const ok = window.confirm(
      "Download this JSON now. If this browser is cleared, the key is gone.\n\nContinue?"
    );
    if (!ok) return;
    file = created;
    W.storeWallet(file);
    await syncId();
    W.downloadWallet(file, "mesh-wallet.json");
    log(
      "Wallet created and saved in this browser.\nDownload the JSON — it is the only backup.\nMesh name: " +
        (await W.describeWallet(file)).mesh_name
    );
  };

  document.getElementById("btnDownload").onclick = () => {
    if (!file) return log("No wallet in this browser.");
    W.downloadWallet(file, "mesh-wallet.json");
  };

  document.getElementById("btnCopy").onclick = async () => {
    const name = document.getElementById("wName").textContent;
    if (!name || name === "—") return;
    try {
      await navigator.clipboard.writeText(name);
      log("Copied " + name);
    } catch {
      log(name);
    }
  };

  document.getElementById("btnWrap").onclick = async () => {
    if (!file) return log("Create or import first.");
    try {
      const unlocked = await unlockIfNeeded(file);
      const pass = promptPass("New passphrase (min 6 chars). You will need it to send.");
      if (!pass) return;
      const wrapped = await W.wrapSecret(unlocked, pass);
      file = wrapped;
      W.storeWallet(file);
      W.downloadWallet(file, "mesh-wallet-wrapped.json");
      log("Wrapped with passphrase. Download the wrapped JSON. Send will prompt for the passphrase.");
      await syncId();
    } catch (e) {
      log("Wrap: " + e.message);
    }
  };

  document.getElementById("fileIn").onchange = async (ev) => {
    const f = ev.target.files?.[0];
    if (!f) return;
    try {
      const j = JSON.parse(await f.text());
      if (j.wrapped && j.public_hex) {
        file = j;
        W.storeWallet(file);
        await syncId();
        log("Imported wrapped key " + j.public_hex.slice(0, 12) + "… Unlock when you send.");
        return;
      }
      if (!j.secret_hex || !j.public_hex) throw new Error("need secret_hex + public_hex");
      file = { secret_hex: j.secret_hex, public_hex: j.public_hex };
      W.storeWallet(file);
      await syncId();
      await refreshBalance(file);
      await loadActivity(file);
      log("Imported " + (await W.describeWallet(file)).mesh_name);
    } catch (e) {
      log("Import failed: " + e.message);
    }
  };

  document.getElementById("btnForget").onclick = () => {
    W.clearStoredWallet();
    file = W.loadStoredWallet();
    syncId();
    log("Removed this key from the browser. Your download JSON still works if you kept it.");
  };

  document.getElementById("btnRefresh").onclick = async () => {
    if (!file) return log("Create or import a wallet first.");
    await refreshBalance(file);
    await loadActivity(file);
  };

  document.getElementById("btnFaucet").onclick = async () => {
    if (!file) return log("Create a wallet first.");
    const info = file.secret_hex
      ? await W.describeWallet(file)
      : await paintAccount(panel, file);
    log("Requesting faucet drip…");
    try {
      const before = { nonce: null, balance: null };
      try {
        const acc = await W.scannerAccount(endpoints().scanner, info.mesh_name);
        before.nonce = acc.nonce;
        before.balance = acc.balance;
      } catch {
        before.nonce = -1;
      }
      const j = await W.faucetDrip(endpoints().faucet, info.mesh_name, info.public_hex);
      log("Faucet accepted " + (j.amount_tmesh ?? 100) + " tMESH to " + info.mesh_name + "\n" + JSON.stringify(j, null, 2));
      await afterSubmit(info, before, "Faucet");
    } catch (e) {
      log("Faucet: " + e.message);
    }
  };

  document.getElementById("btnRegister").onclick = async () => {
    if (!file) return log("Create a wallet first.");
    let spend;
    try {
      spend = await unlockIfNeeded(file);
    } catch (e) {
      return log(e.message);
    }
    const info = await W.describeWallet(spend);
    let nonce = 0;
    let before = { nonce: 0, balance: 0 };
    try {
      const acc = await W.scannerAccount(endpoints().scanner, info.mesh_name);
      nonce = acc.nonce || 0;
      before = { nonce: acc.nonce, balance: acc.balance };
    } catch {
      nonce = 0;
    }
    const tx = W.buildRegisterTx(spend, nonce, info);
    log("Submitting Register via " + pathMode() + "…");
    try {
      const j = await W.submitTx(endpoints(), tx, pathMode());
      log("Register submitted for " + info.mesh_name + "\n" + JSON.stringify(j, null, 2));
      await afterSubmit(info, before, "Register");
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
    let spend;
    try {
      spend = await unlockIfNeeded(file);
    } catch (e) {
      return log(e.message);
    }
    const info = await W.describeWallet(spend);
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
    let before = { nonce: 0, balance: 0 };
    try {
      if (pathMode() === "radio") {
        const b = await W.radioBalance(endpoints().radio, info.mesh_name);
        nonce = Number(b.nonce || 0);
        before = { nonce, balance: b.balance };
      } else {
        const acc = await W.scannerAccount(endpoints().scanner, info.mesh_name);
        nonce = Number(acc.nonce || 0);
        before = { nonce: acc.nonce, balance: acc.balance };
      }
    } catch {
      return log("Cannot read nonce — register / faucet first so the account exists.");
    }
    const tx = W.buildTransferTx(spend, info, to8, amount, nonce, fee);
    log("Sending " + amount / 1e6 + " tMESH via " + pathMode() + " (nonce " + nonce + ")…");
    try {
      const j = await W.submitTx(endpoints(), tx, pathMode());
      log("Submitted.\n" + JSON.stringify(j, null, 2));
      await afterSubmit(info, before, "Transfer");
    } catch (e) {
      log(e.message);
    }
  };

  (async () => {
    const vec = W.verifySignVectors();
    if (!vec.transfer_bytes || !vec.register_bytes) {
      log("Sign-byte fixture mismatch — do not send until this is fixed.\n" + JSON.stringify(vec));
    }
    await probeRadio();
    await syncId();
    try {
      const info = await W.faucetInfo(endpoints().faucet);
      document.getElementById("fDrip").textContent = (info.amount_tmesh ?? 100) + " tMESH";
    } catch {
      document.getElementById("fDrip").textContent = "offline";
    }
    if (file) {
      await refreshBalance(file);
      await loadActivity(file);
    }
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
