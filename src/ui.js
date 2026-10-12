import { invoke } from "@tauri-apps/api/core";
const hex = (n) => "#" + (n >>> 0).toString(16).padStart(6, "0").slice(-6);
const lum = (n) => (0.2126 * ((n >> 16) & 255) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255)) / 255;
export async function applyTheme() {
  const c = await invoke("settings_get", { key: "themeColors" }).catch(() => null);
  const r = document.documentElement;
  const ok = c && Number.isFinite(c.bg) && Number.isFinite(c.fg);
  ok ? (r.style.setProperty("--bg", hex(c.bg)), r.style.setProperty("--accent", hex(c.fg)), r.style.setProperty("--on-accent", lum(c.fg) > 0.6 ? "#0b0b0f" : "#fff"), r.dataset.scheme = lum(c.bg) > 0.5 ? "light" : "dark") : (["--bg", "--accent", "--on-accent"].forEach((k) => r.style.removeProperty(k)), delete r.dataset.scheme);
}
export function watchTheme() {
  applyTheme();
  addEventListener("focus", applyTheme);
  document.addEventListener("visibilitychange", () => document.hidden || applyTheme());
}
export function renderServers(list, entries, t, pick) {
  list.replaceChildren(...[...entries].sort((a, b) => (b.lastConnected || 0) - (a.lastConnected || 0)).map((entry) => {
    const item = document.createElement("button");
    const info = document.createElement("span");
    const name = document.createElement("span");
    const url = document.createElement("span");
    const go = document.createElement("span");
    let host = entry.url;
    try { host = new URL(entry.url).hostname; } catch {}
    item.type = "button";
    item.className = "recent-server-item";
    info.className = "recent-server-info";
    name.className = "recent-server-name";
    url.className = "recent-server-url";
    go.className = "recent-server-action";
    name.textContent = entry.name && entry.name !== entry.url ? entry.name : host;
    url.textContent = entry.url;
    go.textContent = `${t("serverPicker.connect")} →`;
    info.append(name, url);
    item.append(info, go);
    item.addEventListener("click", () => pick(entry, item));
    return item;
  }));
}
