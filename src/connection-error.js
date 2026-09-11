import { invoke } from "@tauri-apps/api/core";
import { createTranslator } from "./i18n/index.js";

function paramsUrl() {
  return new URLSearchParams(location.search).get("url") || "";
}

function bindAction(el, run) {
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

  document.getElementById("error-title").textContent = t("connection.problemTitle");
  document.getElementById("error-message").textContent = t("connection.problemMessage", {
    url: info.url || "",
  });
  const urlEl = document.getElementById("error-url");
  urlEl.textContent = info.url || "";
  urlEl.hidden = !info.url;

  const retry = document.getElementById("btn-retry");
  retry.textContent = t("welcome.tryAgain");
  bindAction(retry, () => invoke("nav_open_app", { serverUrl: info.url }));

  const welcome = document.getElementById("btn-welcome");
  welcome.textContent = t("connection.goBackWelcome");
  bindAction(welcome, () => invoke("nav_back_to_welcome"));

  const back = document.getElementById("btn-back-server");
  if (info.canGoBackServer && info.primary) {
    back.hidden = false;
    back.textContent = t("connection.goBackServer");
    bindAction(back, () => invoke("nav_switch_server", { serverUrl: info.primary }));
  }
}

boot().catch((err) => {
  console.error("[Haven] connection-error boot failed", err);
});
