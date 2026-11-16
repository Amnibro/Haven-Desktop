import { invoke } from "@tauri-apps/api/core";
import { createTranslator } from "./i18n/index.js";
import { renderServers, watchTheme } from "./ui.js";
watchTheme();

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

function bindRun(el, run) {
  el.setAttribute("aria-busy", "true");
  Promise.resolve().then(run).catch((err) => { console.error("[Haven] connection-error action failed", err); el.removeAttribute("aria-busy"); });
}
function bindAction(el, run) {
  el?.addEventListener("click", (event) => { event.preventDefault(); bindRun(el, run); });
}

// 5 s, 10 s, 20 s, 40 s, then once a minute. The attempt count survives this
// page being reloaded by a failed try, so the delay keeps growing.
function autoRetry(url, t) {
  const status = document.getElementById("retry-status");
  const key = "haven_retry_" + url;
  // A failure more than five minutes after the last try is a new outage.
  let attempt = 0;
  try {
    const saved = JSON.parse(sessionStorage.getItem(key) || "null");
    if (saved && Date.now() - saved.at < 5 * 60 * 1000) attempt = Number(saved.attempt) || 0;
  } catch {}
  const delay = Math.min(60, 5 * 2 ** Math.min(attempt, 4)) + Math.floor(Math.random() * 3);
  let left = delay;
  const show = () => {
    if (!status) return;
    status.hidden = false;
    status.textContent = left > 0
      ? (t("connection.retryingIn", { seconds: left }) || `Trying again in ${left}s…`)
      : (t("connection.retryingNow") || "Trying again…");
  };
  show();
  const timer = setInterval(() => {
    left -= 1;
    show();
    if (left > 0) return;
    clearInterval(timer);
    try { sessionStorage.setItem(key, JSON.stringify({ attempt: attempt + 1, at: Date.now() })); } catch {}
    // Opens the app when the server answers; otherwise this page stays (or
    // is reloaded) and the next, longer wait starts.
    invoke("nav_open_app", { serverUrl: url })
      .catch((err) => console.warn("[Haven] retry failed", err))
      .finally(() => autoRetry(url, t));
  }, 1000);
  addEventListener("pagehide", () => clearInterval(timer), { once: true });
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
  if (titleEl) titleEl.textContent = info.title || t("connection.problemTitle") || "Connection Problem";

  const msgEl = document.getElementById("error-message");
  if (msgEl) {
    msgEl.textContent = (info.message || t("connection.problemMessage", { url: info.url || "" }) ||
      "Haven couldn't load that server. It may be down, the address may be wrong, or the network dropped.") + (info.note ? "\n\n" + info.note : "");
  }
  const signIn = document.getElementById("btn-open-signin");
  signIn.hidden = !info.openLabel;
  signIn.textContent = info.openLabel || "";

  const urlEl = document.getElementById("error-url");
  urlEl.textContent = info.url || "";
  urlEl.hidden = !info.url || msgEl.textContent.includes(info.url);
  document.getElementById("back-top-text").textContent = t("connection.goBackWelcome") || "Go Back to Welcome";
  bindAction(document.getElementById("btn-back-top"), () => invoke("nav_back_to_welcome"));
  const retry = document.getElementById("btn-retry");
  retry.textContent = t("welcome.tryAgain") || "Try Again";
  bindAction(retry, () => invoke("nav_open_app", { serverUrl: info.url }));
  // A server that is down or a network that dropped usually comes back on its
  // own, so keep trying with a growing delay instead of waiting for a click.
  // Sign-in portals, redirects and certificate problems need the person.
  if (info.url && !info.openLabel && !info.message) autoRetry(info.url, t);
  const failedOrigin = normalizeUrl(info.url);
  const backServer = document.getElementById("btn-back-server");
  const canBack = info.canGoBackServer && info.primary && normalizeUrl(info.primary) !== failedOrigin;
  backServer.hidden = !canBack;
  backServer.textContent = t("connection.goBackServer") || "Go Back to My Server";
  canBack && bindAction(backServer, () => invoke("nav_switch_server", { serverUrl: info.primary }));
  document.getElementById("picker-title-text").textContent = t("serverPicker.switch") || "Switch Server";
  document.getElementById("btn-connect-custom").textContent = t("serverPicker.connect") || "Connect";
  const recentList = document.getElementById("recent-servers-list");
  const history = await invoke("server_history_get").catch(() => []);
  const filtered = (history || []).filter((h) => { const o = normalizeUrl(h.url); return o && o !== failedOrigin; });
  recentList.hidden = !filtered.length;
  renderServers(recentList, filtered, t, (entry, item) => bindRun(item, () => invoke("nav_change_primary_server", { serverUrl: entry.url })));
  // Custom Server Connect Form
  const form = document.getElementById("custom-server-form");
  const customInput = document.getElementById("custom-server-url");
  const customError = document.getElementById("custom-server-error");

  if (form && customInput) {
    form.onsubmit = async (e) => {
      e.preventDefault();
      customError.hidden = true;
      const cleaned = normalizeUrl(customInput.value);
      if (!cleaned) {
        customError.textContent = t("welcome.error.invalidUrl");
        customError.hidden = false;
        return;
      }
      try {
        const btn = document.getElementById("btn-connect-custom");
        if (btn) btn.setAttribute("aria-busy", "true");
        await invoke("nav_change_primary_server", { serverUrl: cleaned });
      } catch (err) {
        customError.textContent = String(err?.message || err || t("serverPicker.error.connectionFailed"));
        customError.hidden = false;
      }
    };
  }
}

boot().catch((err) => {
  console.error("[Haven] connection-error boot failed", err);
});

