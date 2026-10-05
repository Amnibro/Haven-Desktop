import { invoke } from "@tauri-apps/api/core";
import { createTranslator } from "./i18n/index.js";

function paramsUrl() {
  return new URLSearchParams(location.search).get("url") || "";
}

function normalizeUrl(raw) {
  if (!raw) return "";
  let trimmed = raw.trim();
  if (!/^https?:\/\//i.test(trimmed)) {
    trimmed = "https://" + trimmed;
  }
  try {
    const parsed = new URL(trimmed);
    return parsed.origin;
  } catch {
    return "";
  }
}

function bindAction(el, run) {
  if (!el) return;
  el.addEventListener("click", (event) => {
    event.preventDefault();
    el.setAttribute("aria-busy", "true");
    Promise.resolve()
      .then(run)
      .catch((err) => {
        console.error("[Haven] connection-error action failed", err);
        el.removeAttribute("aria-busy");
      });
  });
}

async function boot() {
  let locale = "en";
  try {
    const state = await invoke("i18n_get_state");
    locale = state?.locale || "en";
  } catch {}
  const t = createTranslator(locale);

  let info = { url: paramsUrl(), primary: "", canGoBackServer: false };
  try {
    info = { ...info, ...(await invoke("nav_connection_info")) };
  } catch {}
  if (!info.url) info.url = paramsUrl();

  const titleEl = document.getElementById("error-title");
  if (titleEl) titleEl.textContent = t("connection.problemTitle") || "Connection Problem";

  const msgEl = document.getElementById("error-message");
  if (msgEl) {
    msgEl.textContent = t("connection.problemMessage", { url: info.url || "" }) ||
      "Haven couldn't load that server. It may be down, the address may be wrong, or the network dropped.";
  }

  const urlEl = document.getElementById("error-url");
  if (urlEl) {
    urlEl.textContent = info.url || "";
    urlEl.hidden = !info.url;
  }

  // Top Back Button
  const backTop = document.getElementById("btn-back-top");
  bindAction(backTop, () => invoke("nav_back_to_welcome"));

  // Retry
  const retry = document.getElementById("btn-retry");
  if (retry) {
    retry.textContent = t("welcome.tryAgain") || "Try Again";
    bindAction(retry, () => invoke("nav_open_app", { serverUrl: info.url }));
  }

  // Pick a Different Server action button
  const pickBtn = document.getElementById("btn-pick-server");
  if (pickBtn) {
    pickBtn.onclick = () => {
      const section = document.getElementById("server-picker-section");
      if (section) {
        section.scrollIntoView({ behavior: "smooth" });
        const input = document.getElementById("custom-server-url");
        if (input) input.focus();
      }
    };
  }

  // Go to Welcome Screen
  const welcome = document.getElementById("btn-welcome");
  if (welcome) {
    welcome.textContent = t("connection.goBackWelcome") || "Welcome Screen";
    bindAction(welcome, () => invoke("nav_back_to_welcome"));
  }

  // Go Back to My Server (if primary server exists)
  const backServer = document.getElementById("btn-back-server");
  if (backServer && info.canGoBackServer && info.primary) {
    backServer.hidden = false;
    backServer.textContent = t("connection.goBackServer") || "Go Back to My Server";
    bindAction(backServer, () => invoke("nav_switch_server", { serverUrl: info.primary }));
  }

  // Populate Server History & Picker
  const recentList = document.getElementById("recent-servers-list");
  const failedOrigin = normalizeUrl(info.url);

  try {
    const history = await invoke("server_history_get").catch(() => []);
    const filtered = (history || []).filter((h) => {
      const origin = normalizeUrl(h.url);
      return origin && origin !== failedOrigin;
    });

    if (recentList) {
      recentList.innerHTML = "";
      if (filtered.length > 0) {
        filtered.sort((a, b) => (b.lastConnected || 0) - (a.lastConnected || 0));
        filtered.forEach((entry) => {
          const item = document.createElement("div");
          item.className = "recent-server-item";

          let hostName = entry.url;
          try { hostName = new URL(entry.url).hostname; } catch {}
          const displayName = (entry.name && entry.name !== entry.url) ? entry.name : hostName;

          item.innerHTML = `
            <div class="recent-server-info">
              <span class="recent-server-name">${displayName}</span>
              <span class="recent-server-url">${entry.url}</span>
            </div>
            <span class="recent-server-action">Connect →</span>
          `;

          bindAction(item, () => invoke("nav_change_primary_server", { serverUrl: entry.url }));
          recentList.appendChild(item);
        });
      } else {
        recentList.style.display = "none";
      }
    }
  } catch (err) {
    console.warn("[Haven] Failed to load server history", err);
  }

  // Custom Server Connect Form
  const form = document.getElementById("custom-server-form");
  const customInput = document.getElementById("custom-server-url");
  const customError = document.getElementById("custom-server-error");

  if (form && customInput) {
    form.onsubmit = async (e) => {
      e.preventDefault();
      if (customError) customError.style.display = "none";
      const cleaned = normalizeUrl(customInput.value);
      if (!cleaned) {
        if (customError) {
          customError.textContent = "Please enter a valid server URL (e.g. https://haven.example.com)";
          customError.style.display = "block";
        }
        return;
      }
      try {
        const btn = document.getElementById("btn-connect-custom");
        if (btn) btn.setAttribute("aria-busy", "true");
        await invoke("nav_change_primary_server", { serverUrl: cleaned });
      } catch (err) {
        if (customError) {
          customError.textContent = String(err?.message || err || "Could not connect to that server");
          customError.style.display = "block";
        }
      }
    };
  }
}

boot().catch((err) => {
  console.error("[Haven] connection-error boot failed", err);
});

