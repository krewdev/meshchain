/* MeshChain docs viewer — loads markdown from /content/ */
(function () {
  const DOCS = [
    { group: "Start" },
    { id: "GETTING_STARTED", title: "Getting started" },
    { id: "TESTNET", title: "Public testnet" },
    { id: "STATUS", title: "Operator status" },
    { id: "RUN_A_NODE", title: "Run a node" },
    { id: "SCANNER", title: "Blockchain scanner" },
    { id: "HARDWARE", title: "Hardware" },
    { id: "MESHTASTIC", title: "Meshtastic air path" },
    { id: "AIR_FINALITY", title: "Air finality" },
    { id: "RADIO_HARDWARE", title: "Radio hardware" },
    { id: "DISCORD", title: "Discord" },
    { id: "DONATE", title: "Donate" },

    { group: "Protocol" },
    { id: "PROTOCOL", title: "Protocol" },
    { id: "SECURITY_HARDENING", title: "Security hardening" },
    { id: "SECURITY", title: "Security summary" },
    { id: "HYBRID_LOCK", title: "Hybrid lock" },
    { id: "QUANTUM_COLD_STORAGE", title: "Quantum cold storage" },
    { id: "TRUTH_MODEL", title: "Truth model" },

    { group: "Ops" },
    { id: "HOST_OPS", title: "Host ops" },
    { id: "MULTI_VALIDATOR", title: "Multi-validator" },
    { id: "MULTI_OPERATOR", title: "Multi-operator" },
    { id: "VPS_PUBLIC", title: "VPS & tunnel" },
    { id: "CLOUD", title: "Cloud" },
    { id: "SCANNER_AUTO_UPDATE", title: "Scanner auto-update" },
    { id: "AUDIT_AND_TEST", title: "Audit & test" },

    { group: "Bridge" },
    { id: "SOLANA_BRIDGE", title: "Solana bridge" },
    { id: "SOLANA_DEVNET", title: "Solana devnet" },
    { id: "HYBRID_SECURITY_REVIEW", title: "Hybrid security brief" },
    { id: "BTC_VAULT", title: "BTC vault design" },
    { id: "E2E_TESTNET", title: "E2E results" },
  ];

  const PAGES = DOCS.filter((d) => d.id);

  function qs(name) {
    return new URLSearchParams(window.location.search).get(name);
  }

  function setActive(id) {
    document.querySelectorAll(".sidebar a").forEach((a) => {
      a.classList.toggle("active", a.dataset.id === id);
    });
  }

  function rewriteDocLinks(html) {
    return html
      .replace(/href="\.\/?([A-Z0-9_]+)\.md"/gi, 'href="/docs/?doc=$1"')
      .replace(/href="([A-Z0-9_]+)\.md"/gi, 'href="/docs/?doc=$1"');
  }

  async function loadDoc(id) {
    const el = document.getElementById("doc");
    const meta = PAGES.find((d) => d.id === id) || PAGES[0];
    id = meta.id;
    setActive(id);
    el.innerHTML = '<p class="loading">Loading…</p>';
    document.title = meta.title + " — MeshChain Docs";

    try {
      const res = await fetch("/content/" + id + ".md", { cache: "no-cache" });
      if (!res.ok) throw new Error("HTTP " + res.status);
      const md = await res.text();
      let html = marked.parse(md, { mangle: false, headerIds: true });
      html = rewriteDocLinks(html);
      el.innerHTML =
        '<p class="doc-meta"><a href="/docs/">Docs</a> / ' +
        meta.title +
        ' · <a href="https://github.com/krewdev/meshchain/blob/main/docs/' +
        id +
        '.md">Edit on GitHub</a></p>' +
        html;
      history.replaceState(null, "", "/docs/?doc=" + id);
      if (location.hash) {
        const t = document.querySelector(location.hash);
        if (t) t.scrollIntoView();
      } else {
        window.scrollTo({ top: 0, behavior: "instant" });
      }
    } catch (e) {
      el.innerHTML =
        '<p class="error">Could not load this page. Try again or open the ' +
        '<a href="https://github.com/krewdev/meshchain/tree/main/docs">GitHub docs folder</a>.</p>';
      console.error(e);
    }
  }

  function buildSidebar() {
    const side = document.getElementById("sidebar");
    side.innerHTML = "";
    DOCS.forEach((d) => {
      if (d.group) {
        const h = document.createElement("h4");
        h.textContent = d.group;
        side.appendChild(h);
        return;
      }
      const a = document.createElement("a");
      a.href = "/docs/?doc=" + d.id;
      a.textContent = d.title;
      a.dataset.id = d.id;
      a.addEventListener("click", (ev) => {
        ev.preventDefault();
        loadDoc(d.id);
      });
      side.appendChild(a);
    });
  }

  document.addEventListener("DOMContentLoaded", () => {
    buildSidebar();
    const id = qs("doc") || "GETTING_STARTED";
    loadDoc(id);
  });
})();
