import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createTranslator } from "./i18n/index.js";

type I18nState = {
  preference: string;
  locale: string;
  systemLocale: string;
  isPreferenceStored: boolean;
  direction: string;
  supportedLocales: Array<{ code: string; name: string; direction: string }>;
};

function detectPlatform(): string {
  const p = navigator.platform.toLowerCase();
  if (p.includes("win")) return "win32";
  if (p.includes("mac")) return "darwin";
  return "linux";
}

/** Install `window.haven` for the welcome page (Electron preload compatible). */
export async function installWelcomeBridge(): Promise<void> {
  let currentState = await invoke<I18nState>("i18n_get_state");
  let translate = createTranslator(currentState.locale);
  const listeners = new Set<(s: I18nState) => void>();

  await listen<I18nState>("i18n:changed", (event) => {
    currentState = event.payload;
    translate = createTranslator(currentState.locale);
    for (const cb of listeners) cb({ ...currentState });
  });

  window.haven = {
    platform: detectPlatform(),

    server: {
      detect: () => invoke("server_detect"),
      start: (dir: string) => invoke("server_start", { dir }),
      stop: () => invoke("server_stop"),
      browse: () => invoke<string | null>("server_browse"),
      browseFile: () => invoke<string | null>("server_browse"),
      getStatus: () => invoke("server_status"),
      onLog: (cb: (m: string) => void) => {
        void listen<string>("server:log", (e) => cb(e.payload));
      },
    },

    settings: {
      get: (key: string) => invoke("settings_get", { key }),
      set: (key: string, val: unknown) => invoke("settings_set", { key, value: val }),
    },

    window: {
      minimize: () => invoke("window_minimize").then(() => undefined),
      maximize: () => invoke("window_maximize").then(() => undefined),
      close: () => invoke("window_close").then(() => undefined),
    },

    nav: {
      openApp: (serverUrl: string) =>
        invoke("nav_open_app", { serverUrl }).then(() => undefined),
    },

    update: {
      download: async () => ({ errorKey: "update.unavailable" }),
      install: () => {},
    },

    i18n: {
      getState: () => ({ ...currentState }),
      t: (key: string, values?: Record<string, string | number>) => {
        const mapped: Record<string, string> = {};
        if (values) {
          for (const [k, v] of Object.entries(values)) mapped[k] = String(v);
        }
        return translate(key, mapped);
      },
      setLanguage: async (preference: string) => {
        currentState = await invoke<I18nState>("i18n_set_language", { preference });
        translate = createTranslator(currentState.locale);
        return { ...currentState };
      },
      onChanged: (callback) => {
        const wrapped = (s: I18nState) => callback({ ...s });
        listeners.add(wrapped);
        return () => listeners.delete(wrapped);
      },
    },

    openExternal: (url: string) =>
      invoke("open_external", { url }).then(() => undefined),
    getVersion: () => invoke<string>("app_version"),
  };

  const win = getCurrentWindow();
  document.getElementById("titlebar")?.addEventListener("mousedown", (e) => {
    if ((e.target as HTMLElement).closest(".tb-btn")) return;
    win.startDragging().catch(() => {});
  });
}
