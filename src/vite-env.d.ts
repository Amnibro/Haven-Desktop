/// <reference types="vite/client" />

interface HavenWindowApi {
  minimize: () => void | Promise<void>;
  maximize: () => void | Promise<void>;
  close: () => void | Promise<void>;
}

interface HavenI18nApi {
  getState: () => Record<string, unknown>;
  t: (key: string, values?: Record<string, string | number>) => string;
  setLanguage: (preference: string) => Promise<Record<string, unknown>>;
  onChanged: (callback: (state: Record<string, unknown>) => void) => () => void;
}

interface HavenApi {
  platform: string;
  server: {
    detect: () => Promise<unknown>;
    start: (dir: string) => Promise<unknown>;
    stop: () => Promise<unknown>;
    browse: () => Promise<string | null>;
    browseFile: () => Promise<string | null>;
    getStatus: () => Promise<unknown>;
    onLog: (cb: (m: string) => void) => void;
  };
  settings: {
    get: (key: string) => Promise<unknown>;
    set: (key: string, val: unknown) => Promise<unknown>;
  };
  window: HavenWindowApi;
  nav: {
    openApp: (serverUrl: string) => void | Promise<void>;
  };
  update: {
    download: () => Promise<{ errorKey?: string; error?: string }>;
    install: () => void;
  };
  i18n: HavenI18nApi;
  openExternal: (url: string) => void | Promise<void>;
  getVersion: () => Promise<string>;
}

interface Window {
  haven?: HavenApi;
  havenDesktop?: Record<string, unknown>;
  __TAURI__?: {
    core?: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
    event?: {
      listen: (
        event: string,
        handler: (e: { payload: unknown }) => void,
      ) => Promise<() => void>;
    };
  };
}
