// Haven Desktop — Tauri app-window initialization script
// Injected before page scripts (withGlobalTauri). Classic JS, not a module.
(function () {
  'use strict';

  async function invoke(cmd, args = {}) {
    const core = window.__TAURI__?.core;
    if (!core?.invoke) throw new Error('Tauri unavailable');
    return core.invoke(cmd, args);
  }

  async function listen(event, handler) {
    const eventApi = window.__TAURI__?.event;
    if (!eventApi?.listen) return () => {};
    return eventApi.listen(event, (e) => handler(e.payload));
  }

  function detectPlatform() {
    const ua = navigator.userAgent || '';
    if (/Windows/i.test(ua)) return 'win32';
    if (/Mac OS|Macintosh/i.test(ua)) return 'darwin';
    return 'linux';
  }

  // A Tauri webview has no window-open handler, so target="_blank" and
  // window.open() did nothing. Same-server links (message deep links, game
  // pop-outs) navigate in place; anything else goes to the system browser.
  const nativeOpen = window.open;
  window.open = function (url, target, features) {
    try {
      const u = new URL(url, window.location.href);
      if (u.origin === window.location.origin) {
        if (/^\/(app(\.html)?|c\/[A-Za-z0-9]+)/.test(u.pathname) || u.searchParams.has('channel') || u.searchParams.has('message')) {
          window.location.assign(u.href);
          return null;
        }
        return nativeOpen ? nativeOpen.call(window, u.href, target, features) : null;
      }
      if (u.protocol === 'http:' || u.protocol === 'https:') invoke('open_external', { url: u.href }).catch(() => {});
    } catch {}
    return null;
  };
  document.addEventListener('click', (e) => {
    const a = e.target && e.target.closest && e.target.closest('a[href]');
    if (!a || a.target !== '_blank') return;
    e.preventDefault();
    window.open(a.href, '_blank');
  }, true);

  // Themed app icon and title bar, like the mobile launcher icon: send the
  // active theme id and its live colors whenever data-theme changes.
  let lastTheme = '';
  function syncTheme() {
    try {
      const root = document.documentElement;
      let theme = root.getAttribute('data-theme') || '';
      const saved = localStorage.getItem('haven-theme') || '';
      if (saved.startsWith('file:')) theme = saved.slice(5).replace(/\.css$/i, '');
      const cs = getComputedStyle(root);
      const bg = (cs.getPropertyValue('--bg-primary') || '').trim();
      const accent = (cs.getPropertyValue('--accent') || '').trim();
      const key = theme + '|' + bg + '|' + accent;
      if (key === lastTheme) return;
      lastTheme = key;
      invoke('theme_colors', { theme, bg, accent }).catch(() => {});
    } catch {}
  }
  // Init scripts run before the document has a root; observing null throws
  // and would abort the whole bridge (no havenDesktop, server switches leak
  // to the system browser), so attach once the root exists.
  function watchTheme() {
    const root = document.documentElement;
    if (!root) return false;
    new MutationObserver(syncTheme).observe(root, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    syncTheme();
    return true;
  }
  if (!watchTheme()) window.addEventListener('DOMContentLoaded', watchTheme, { once: true });
  window.addEventListener('load', () => { syncTheme(); setTimeout(syncTheme, 1500); });
  window.addEventListener('storage', syncTheme);

  if (document.documentElement) {
    document.documentElement.setAttribute('data-desktop-app', '1');
  } else {
    window.addEventListener('DOMContentLoaded', () => {
      document.documentElement.setAttribute('data-desktop-app', '1');
    }, { once: true });
  }

  // Braid hides the debug status bar; desktop still reserved 35px for it.
  function injectBraidDesktopCss() {
    if (document.getElementById('haven-desktop-braid-gap')) return;
    const el = document.createElement('style');
    el.id = 'haven-desktop-braid-gap';
    el.textContent = 'html[data-braid-layout="1"][data-desktop-app]{--thread-footer-offset:0px}'
      + 'html[data-braid-layout="1"][data-desktop-app] .status-bar,'
      + 'html[data-braid-layout="1"][data-desktop-app] #status-bar{display:none!important;height:0!important;min-height:0!important;padding:0!important;border:0!important;overflow:hidden!important}'
      + 'html[data-braid-layout="1"].braid-status-open[data-desktop-app] .status-bar,'
      + 'html[data-braid-layout="1"].braid-status-open[data-desktop-app] #status-bar{display:flex!important;height:auto!important;min-height:1.75rem!important;padding:.3125rem 1rem!important;overflow:visible!important}'
      + 'html[data-braid-layout="1"] #app-body{flex:1 1 auto!important;height:auto!important;min-height:0}';
    (document.head || document.documentElement).appendChild(el);
  }
  if (document.head) injectBraidDesktopCss();
  else window.addEventListener('DOMContentLoaded', injectBraidDesktopCss, { once: true });

  // ── i18n (passthrough until state loads) ─────────────────
  let i18nState = {
    preference: 'auto',
    locale: 'en',
    systemLocale: 'en',
    isPreferenceStored: false,
    direction: 'ltr',
    supportedLocales: [],
  };

  function t(key) {
    return key;
  }

  function setI18nText(el, key, _values, prefix = '', suffix = '') {
    if (!el) return;
    el.textContent = `${prefix}${t(key)}${suffix}`;
  }

  invoke('i18n_get_state').then((s) => {
    if (s) i18nState = s;
  }).catch(() => {});

  listen('i18n:changed', (state) => {
    if (state) i18nState = state;
  });

  // ── Audio pipeline state ─────────────────────────────────
  let _audioWorkletNode = null;
  let _audioCtx = null;
  let _audioDestination = null;
  let _capturedAudioPid = null;
  let _audioBufferQueue = [];
  let _audioPacketsReceived = 0;
  let _ipcDataCount = 0;
  let _lastNativeStatus = null;

  function pcmFromPayload(pcmData) {
    if (pcmData instanceof Float32Array) return pcmData;
    if (pcmData instanceof ArrayBuffer) return new Float32Array(pcmData);
    if (ArrayBuffer.isView(pcmData)) {
      const bytes = new Uint8Array(pcmData.buffer, pcmData.byteOffset, pcmData.byteLength);
      const aligned = new ArrayBuffer(bytes.length);
      new Uint8Array(aligned).set(bytes);
      return new Float32Array(aligned);
    }
    if (Array.isArray(pcmData)) return new Float32Array(pcmData);
    return null;
  }

  listen('audio:capture-status', (status) => {
    _lastNativeStatus = status;
  });

  listen('audio:capture-data', (pcmData) => {
    const samples = pcmFromPayload(pcmData);
    if (!samples) return;
    _ipcDataCount++;
    if (_audioWorkletNode) {
      _audioWorkletNode.port.postMessage({ type: 'audio-data', samples });
    } else if (window._havenAppAudioPush) {
      window._havenAppAudioPush(samples);
    } else {
      _audioBufferQueue.push(samples);
    }
    _audioPacketsReceived++;
  });

  async function buildAudioPipeline() {
    _audioPacketsReceived = 0;
    _ipcDataCount = 0;
    _audioBufferQueue = [];

    try {
      _audioCtx = new AudioContext({ sampleRate: 48000 });
      if (_audioCtx.state === 'suspended') await _audioCtx.resume();

      const workletSrc = `
        class AppAudioProcessor extends AudioWorkletProcessor {
          constructor() {
            super();
            this._ring = new Float32Array(96000);
            this._wPos = 0;
            this._rPos = 0;
            this._avail = 0;
            this.port.onmessage = (e) => {
              if (e.data.type !== 'audio-data') return;
              const s = e.data.samples;
              for (let i = 0; i < s.length; i++) {
                this._ring[this._wPos] = s[i];
                this._wPos = (this._wPos + 1) % this._ring.length;
              }
              this._avail = Math.min(this._avail + s.length, this._ring.length);
            };
          }
          process(_inputs, outputs) {
            const out = outputs[0];
            if (!out || !out.length) return true;
            const buf = out[0];
            const len = buf.length;
            if (this._avail < len) { buf.fill(0); return true; }
            for (let i = 0; i < len; i++) {
              buf[i] = this._ring[this._rPos];
              this._rPos = (this._rPos + 1) % this._ring.length;
            }
            this._avail -= len;
            for (let ch = 1; ch < out.length; ch++) out[ch].set(buf);
            return true;
          }
        }
        registerProcessor('app-audio-processor', AppAudioProcessor);
      `;
      const blob = new Blob([workletSrc], { type: 'application/javascript' });
      const url = URL.createObjectURL(blob);
      await _audioCtx.audioWorklet.addModule(url);
      URL.revokeObjectURL(url);

      _audioWorkletNode = new AudioWorkletNode(_audioCtx, 'app-audio-processor', {
        numberOfInputs: 0,
        outputChannelCount: [2],
      });
      _audioDestination = _audioCtx.createMediaStreamDestination();
      _audioWorkletNode.connect(_audioDestination);
      const silencer = _audioCtx.createGain();
      silencer.gain.value = 0;
      _audioWorkletNode.connect(silencer);
      silencer.connect(_audioCtx.destination);

      _audioBufferQueue.forEach((buf) =>
        _audioWorkletNode.port.postMessage({ type: 'audio-data', samples: buf })
      );
      _audioBufferQueue = [];

      window._havenAppAudioTrack = _audioDestination.stream.getAudioTracks()[0];
      window._havenAppAudioStream = _audioDestination.stream;
      window._havenAudioCtxMonitor = setInterval(() => {
        if (_audioCtx && _audioCtx.state === 'suspended') _audioCtx.resume().catch(() => {});
      }, 2000);
      return true;
    } catch (err) {
      console.warn('[Haven Desktop] AudioWorklet failed, ScriptProcessor fallback:', err.message);
      _audioWorkletNode = null;
      if (_audioCtx) { _audioCtx.close().catch(() => {}); _audioCtx = null; }
      _audioDestination = null;
    }

    try {
      _audioCtx = new AudioContext({ sampleRate: 48000 });
      if (_audioCtx.state === 'suspended') await _audioCtx.resume();
      const bufSize = 4096;
      const scriptNode = _audioCtx.createScriptProcessor(bufSize, 1, 2);
      const ring = new Float32Array(96000);
      let wPos = 0, rPos = 0, avail = 0;

      window._havenAppAudioPush = (samples) => {
        for (let i = 0; i < samples.length; i++) {
          ring[wPos] = samples[i];
          wPos = (wPos + 1) % ring.length;
        }
        avail = Math.min(avail + samples.length, ring.length);
      };

      scriptNode.onaudioprocess = (e) => {
        const out = e.outputBuffer.getChannelData(0);
        if (avail < out.length) out.fill(0);
        else {
          for (let i = 0; i < out.length; i++) {
            out[i] = ring[rPos];
            rPos = (rPos + 1) % ring.length;
          }
          avail -= out.length;
        }
        e.outputBuffer.getChannelData(1).set(out);
      };

      _audioDestination = _audioCtx.createMediaStreamDestination();
      scriptNode.connect(_audioDestination);
      const driver = _audioCtx.createConstantSource();
      driver.offset.value = 0;
      driver.connect(scriptNode);
      driver.start();
      const silencer = _audioCtx.createGain();
      silencer.gain.value = 0;
      scriptNode.connect(silencer);
      silencer.connect(_audioCtx.destination);

      _audioBufferQueue.forEach((buf) => window._havenAppAudioPush(buf));
      _audioBufferQueue = [];
      window._havenAppAudioTrack = _audioDestination.stream.getAudioTracks()[0];
      window._havenAppAudioStream = _audioDestination.stream;
      window._havenAudioCtxMonitor = setInterval(() => {
        if (_audioCtx && _audioCtx.state === 'suspended') _audioCtx.resume().catch(() => {});
      }, 2000);
      return true;
    } catch (err) {
      console.error('[Haven Desktop] Audio pipeline failed:', err);
      if (_audioCtx) { _audioCtx.close().catch(() => {}); _audioCtx = null; }
      _audioDestination = null;
      window._havenAppAudioPush = null;
      return false;
    }
  }

  function teardownAudioPipeline() {
    invoke('audio_stop_capture').catch(() => {});
    if (window._havenAudioCtxMonitor) {
      clearInterval(window._havenAudioCtxMonitor);
      window._havenAudioCtxMonitor = null;
    }
    try { _audioWorkletNode?.disconnect(); } catch {}
    _audioWorkletNode = null;
    _audioCtx?.close().catch(() => {});
    _audioCtx = null;
    _audioDestination = null;
    _capturedAudioPid = null;
    _audioBufferQueue = [];
    _audioPacketsReceived = 0;
    _ipcDataCount = 0;
    window._havenAppAudioTrack = null;
    window._havenAppAudioStream = null;
    window._havenAppAudioPush = null;
  }

  async function listDevices(kind) {
    if (!navigator.mediaDevices?.enumerateDevices) return [];
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices
      .filter((d) => d.kind === kind)
      .map((d) => ({ deviceId: d.deviceId, label: d.label, groupId: d.groupId }));
  }

  // ── window.havenDesktop ──────────────────────────────────
  window.havenDesktop = {
    platform: detectPlatform(),
    isDesktopApp: true,

    i18n: {
      getState: () => ({ ...i18nState }),
      getLocale: () => i18nState.locale,
      t,
      setLanguage: (preference) => invoke('i18n_set_language', { preference }),
    },

    switchServer: (url) => invoke('nav_switch_server', { serverUrl: url }),
    backToWelcome: () => invoke('nav_back_to_welcome'),

    update: {
      download: async () => ({ errorKey: 'update.unavailable' }),
      install: () => {},
    },

    audio: {
      getApplications: () => invoke('audio_get_apps'),
      startCapture: async (pid) => {
        _capturedAudioPid = pid;
        const ok = await buildAudioPipeline();
        if (!ok) {
          _capturedAudioPid = null;
          return false;
        }
        return invoke('audio_start_capture', { pid, mode: 'include' });
      },
      stopCapture: () => {
        teardownAudioPipeline();
        return invoke('audio_stop_capture');
      },
      isSupported: () => invoke('audio_is_supported'),
      optOutOfDucking: async () => 0,
    },

    devices: {
      getInputs: () => listDevices('audioinput'),
      getOutputs: () => listDevices('audiooutput'),
      setOutput: async (deviceId) => {
        for (const el of document.querySelectorAll('audio, video')) {
          if (el.setSinkId) await el.setSinkId(deviceId);
        }
        return true;
      },
    },

    notify: (title, body, opts = {}) =>
      invoke('notify', { opts: { title, body, ...opts } }),

    shortcuts: {
      getConfig: () => invoke('shortcuts_get'),
      setConfig: (updates) => invoke('shortcuts_register', { updates }),
    },

    setUnreadBadge: (hasUnread) =>
      invoke('notification_badge', { hasUnread: !!hasUnread }),

    settings: {
      get: (key) => invoke('settings_get', { key }),
      set: (key, val) => invoke('settings_set', { key, value: val }),
    },

    window: {
      minimize: () => invoke('window_minimize'),
      maximize: () => invoke('window_maximize'),
      close: () => invoke('window_close'),
    },

    getVersion: () => invoke('app_version'),

    clipboardWriteImage: (payload) => invoke('clipboard_write_image', { payload }),
    clipboardWriteText: (text) => invoke('clipboard_write_text', { text }),
    saveImage: ({ bytes, filename } = {}) =>
      invoke('save_image', { payload: bytes || '', filename: filename || 'haven-image.png' }),

    getServerHistory: () => invoke('server_history_get'),
    addServerHistory: (url, name) => invoke('server_history_add', { url, name }),
    removeServerHistory: (url) => invoke('server_history_remove', { url }),
    initialServerHistory: [],

    prefs: {
      get: () => invoke('desktop_get_prefs'),
      setStartOnLogin: (v) => invoke('desktop_set_start_on_login', { enabled: !!v }),
      setStartHidden: (v) => invoke('desktop_set_start_hidden', { enabled: !!v }),
      setMinimizeToTray: (v) => invoke('desktop_set_minimize_to_tray', { enabled: !!v }),
      setForceSDR: (v) => invoke('desktop_set_force_sdr', { enabled: !!v }),
      setHideMenuBar: (v) => invoke('desktop_set_hide_menu_bar', { enabled: !!v }),
      setDisableGpuVsync: (v) => invoke('desktop_set_disable_gpu_vsync', { enabled: !!v }),
      setUnlimitFrameRate: (v) => invoke('desktop_set_unlimit_frame_rate', { enabled: !!v }),
      setLanguage: (v) => invoke('i18n_set_language', { preference: v }),
    },

    getServerBadges: () => invoke('get_server_badges'),
    reportKnownServerUrls: (urls) => invoke('report_known_server_urls', { urls }),
  };

  invoke('server_history_get').then((h) => {
    window.havenDesktop.initialServerHistory = Array.isArray(h) ? h : [];
  }).catch(() => {});

  // ── Notification override ────────────────────────────────
  class HavenNotification {
    constructor(title, opts = {}) {
      invoke('notify', {
        opts: {
          title,
          body: opts.body || '',
          silent: !!opts.silent,
          channelCode: opts.channelCode,
        },
      }).catch(() => {});
      this._onclick = null;
    }
    set onclick(fn) { this._onclick = fn; }
    get onclick() { return this._onclick; }
    close() {}
    static get permission() { return 'granted'; }
    static requestPermission() { return Promise.resolve('granted'); }
  }
  window.Notification = HavenNotification;

  listen('notification-clicked', (channelCode) => {
    if (channelCode && window.app?.switchChannel) {
      window.app.switchChannel(channelCode);
    }
  });

  listen('server-badge-update', (badgeMap) => {
    window.dispatchEvent(new CustomEvent('haven-server-badges', { detail: badgeMap }));
  });

  // ── Dialog overrides (async via invoke; callers should await when possible) ──
  window.alert = (message) => {
    void invoke('dialog_alert', { message: String(message ?? '') });
  };
  window.confirm = (message) =>
    invoke('dialog_confirm', { message: String(message ?? '') });
  window.prompt = (message, defaultValue) =>
    invoke('dialog_prompt', {
      message: String(message ?? ''),
      defaultValue: defaultValue == null ? null : String(defaultValue),
    });

  // ── Fullscreen overrides ─────────────────────────────────
  (function patchFullscreen() {
    let _fullscreenEl = null;

    function injectStyle() {
      const style = document.createElement('style');
      style.textContent = `
        .haven-manual-fullscreen {
          position: fixed !important; top: 0 !important; left: 0 !important;
          width: 100vw !important; height: 100vh !important;
          max-width: unset !important; max-height: unset !important;
          z-index: 2147483647 !important; background: #000 !important;
          object-fit: contain !important; margin: 0 !important; padding: 0 !important;
          border: none !important; border-radius: 0 !important;
        }
      `;
      document.head.appendChild(style);
    }
    if (document.head) injectStyle();
    else window.addEventListener('DOMContentLoaded', injectStyle, { once: true });

    function enterFullscreen(el) {
      if (_fullscreenEl) exitFullscreen();
      _fullscreenEl = el;
      el.classList.add('haven-manual-fullscreen');
      invoke('window_enter_fullscreen').catch(() => {});
      document.dispatchEvent(new Event('fullscreenchange'));
    }

    function exitFullscreen() {
      if (_fullscreenEl) {
        _fullscreenEl.classList.remove('haven-manual-fullscreen');
        _fullscreenEl = null;
      }
      invoke('window_leave_fullscreen').catch(() => {});
      document.dispatchEvent(new Event('fullscreenchange'));
    }

    Element.prototype.requestFullscreen = function () {
      enterFullscreen(this);
      return Promise.resolve();
    };
    if (Element.prototype.webkitRequestFullscreen) {
      Element.prototype.webkitRequestFullscreen = function () { enterFullscreen(this); };
    }
    Document.prototype.exitFullscreen = function () {
      exitFullscreen();
      return Promise.resolve();
    };
    Object.defineProperty(Document.prototype, 'fullscreenElement', {
      get() { return _fullscreenEl; }, configurable: true,
    });
    Object.defineProperty(Document.prototype, 'webkitFullscreenElement', {
      get() { return _fullscreenEl; }, configurable: true,
    });
    Object.defineProperty(Document.prototype, 'fullscreenEnabled', {
      get() { return true; }, configurable: true,
    });
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && _fullscreenEl) {
        e.preventDefault();
        exitFullscreen();
      }
    }, true);
  })();

  // ── Voice shortcut events ────────────────────────────────
  function clickVoice(id) {
    document.getElementById(id)?.click();
  }

  function pttSetTalking(shouldTalk) {
    const btn = document.getElementById('voice-mute-btn');
    if (!btn) return;
    const pressed = btn.getAttribute('aria-pressed');
    let isMuted;
    if (pressed === 'true' || pressed === 'false') isMuted = pressed === 'true';
    else isMuted = btn.classList.contains('muted') || btn.classList.contains('is-muted');
    if (shouldTalk ? isMuted : !isMuted) btn.click();
  }

  listen('voice:mute-toggle', () => clickVoice('voice-mute-btn'));
  listen('voice:deafen-toggle', () => clickVoice('voice-deafen-btn'));
  listen('voice:ptt-toggle', () => clickVoice('voice-mute-btn'));
  listen('voice:ptt-down', () => pttSetTalking(true));
  listen('voice:ptt-up', () => pttSetTalking(false));

  // ── getDisplayMedia override ─────────────────────────────
  function installGetDisplayMediaOverride() {
    if (!navigator.mediaDevices?.getDisplayMedia) {
      setTimeout(installGetDisplayMediaOverride, 100);
      return;
    }
    const orig = navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices);
    navigator.mediaDevices.getDisplayMedia = async function (constraints) {
      _lastNativeStatus = null;
      const stream = await orig(constraints);

      if (_capturedAudioPid) {
        const timeoutMs = 8000;
        const start = Date.now();
        while (Date.now() - start < timeoutMs) {
          if (_audioPacketsReceived > 0) break;
          if (_lastNativeStatus && _lastNativeStatus.kind === 'failed') break;
          await new Promise((r) => setTimeout(r, 100));
        }
        if (_audioPacketsReceived > 0 && window._havenAppAudioTrack) {
          stream.getAudioTracks().forEach((t) => {
            try { stream.removeTrack(t); t.stop(); } catch {}
          });
          stream.addTrack(window._havenAppAudioTrack);
        }
      } else if (window._havenAppAudioTrack) {
        stream.getAudioTracks().forEach((t) => {
          try { stream.removeTrack(t); t.stop(); } catch {}
        });
        stream.addTrack(window._havenAppAudioTrack);
      }

      stream.getVideoTracks().forEach((t) =>
        t.addEventListener('ended', () => teardownAudioPipeline())
      );
      return stream;
    };
  }

  // ── Simplified screen picker (if main emits screen:show-picker) ──
  function showScreenPicker(sources, audioApps, requestId) {
    document.getElementById('haven-screen-picker')?.remove();
    const overlay = document.createElement('div');
    overlay.id = 'haven-screen-picker';
    overlay.innerHTML = `
      <style>
        #haven-screen-picker{position:fixed;inset:0;background:rgba(0,0,0,.88);z-index:999999;display:flex;align-items:center;justify-content:center;font-family:system-ui,sans-serif}
        .hsp-box{background:#1a1a2e;border-radius:14px;padding:24px;max-width:820px;width:92%;max-height:82vh;display:flex;flex-direction:column;border:1px solid rgba(107,79,219,.3)}
        .hsp-title{color:#e0e0e0;font-size:18px;font-weight:700;margin-bottom:4px}
        .hsp-sub{color:#888;font-size:13px;margin-bottom:12px}
        .hsp-scroll{flex:1;overflow-y:auto;min-height:0}.hsp-sec{margin-bottom:12px}
        .hsp-sec-title{color:#aaa;font-size:11px;text-transform:uppercase;letter-spacing:1px;margin-bottom:8px}
        .hsp-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(160px,1fr));gap:8px}
        .hsp-src{background:#16213e;border-radius:8px;padding:8px;cursor:pointer;border:2px solid transparent}
        .hsp-src.sel{border-color:#6b4fdb}
        .hsp-src-name{color:#ccc;font-size:12px;text-align:center;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
        .hsp-apps{display:flex;flex-wrap:wrap;gap:8px}
        .hsp-app{background:#16213e;border-radius:6px;padding:8px 12px;cursor:pointer;border:2px solid transparent;color:#ccc;font-size:13px}
        .hsp-app.sel{border-color:#6b4fdb}
        .hsp-btns{display:flex;justify-content:flex-end;gap:10px;margin-top:14px}
        .hsp-btn{padding:8px 18px;border-radius:6px;border:none;font-size:14px;cursor:pointer;font-weight:600}
        .hsp-cancel{background:#333;color:#ccc}.hsp-go{background:#6b4fdb;color:#fff}.hsp-go:disabled{opacity:.45;cursor:not-allowed}
      </style>
      <div class="hsp-box">
        <div class="hsp-title">Share Your Screen</div>
        <div class="hsp-sub">Pick a screen or window, optionally isolate app audio.</div>
        <div class="hsp-scroll">
          <div class="hsp-sec"><div class="hsp-sec-title">Screens</div><div class="hsp-grid" id="hsp-screens"></div></div>
          <div class="hsp-sec"><div class="hsp-sec-title">Windows</div><div class="hsp-grid" id="hsp-windows"></div></div>
        </div>
        <div class="hsp-sec"><div class="hsp-sec-title">Application Audio</div><div class="hsp-apps" id="hsp-audio-apps"></div></div>
        <div class="hsp-btns">
          <button class="hsp-btn hsp-cancel" id="hsp-cancel">Cancel</button>
          <button class="hsp-btn hsp-go" id="hsp-go" disabled>Share</button>
        </div>
      </div>`;
    document.body.appendChild(overlay);

    let selSource = null;
    let selAudioPid = 'system';
    const screensEl = document.getElementById('hsp-screens');
    const windowsEl = document.getElementById('hsp-windows');
    const appsEl = document.getElementById('hsp-audio-apps');
    const goBtn = document.getElementById('hsp-go');

    (sources || []).forEach((src) => {
      const el = document.createElement('div');
      el.className = 'hsp-src';
      const name = document.createElement('div');
      name.className = 'hsp-src-name';
      name.textContent = src.name || src.id;
      el.appendChild(name);
      el.onclick = () => {
        overlay.querySelectorAll('.hsp-src.sel').forEach((s) => s.classList.remove('sel'));
        el.classList.add('sel');
        selSource = src.id;
        goBtn.disabled = false;
      };
      (String(src.id || '').startsWith('screen:') ? screensEl : windowsEl).appendChild(el);
    });

    function addApp(label, pid, selected) {
      const el = document.createElement('div');
      el.className = 'hsp-app' + (selected ? ' sel' : '');
      el.textContent = label;
      el.onclick = () => {
        appsEl.querySelectorAll('.sel').forEach((a) => a.classList.remove('sel'));
        el.classList.add('sel');
        selAudioPid = pid;
      };
      appsEl.appendChild(el);
    }
    addApp('🔇 No Audio', 'none', false);
    addApp('🔊 System Audio', 'system', true);
    (audioApps || []).forEach((a) => addApp(a.name || `PID ${a.pid}`, a.pid, false));

    let dismissed = false;
    const dismiss = async (cancelled) => {
      if (dismissed) return;
      dismissed = true;
      overlay.remove();
      document.removeEventListener('keydown', escHandler, true);

      if (!cancelled && selAudioPid && selAudioPid !== 'none' && typeof selAudioPid === 'number') {
        _capturedAudioPid = selAudioPid;
        const ok = await buildAudioPipeline();
        if (ok) await invoke('audio_start_capture', { pid: selAudioPid, mode: 'include' }).catch(() => {});
        else _capturedAudioPid = null;
      }

      try {
        await invoke('screen_picker_result', {
          requestId,
          cancelled: !!cancelled,
          sourceId: selSource,
          audioAppPid: selAudioPid,
        });
      } catch {
        // Optional command — Tauri may rely on native getDisplayMedia instead.
      }
    };

    document.getElementById('hsp-cancel').onclick = () => dismiss(true);
    goBtn.onclick = () => dismiss(false);
    overlay.addEventListener('mousedown', (e) => { if (e.target === overlay) dismiss(true); });
    const escHandler = (e) => {
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); dismiss(true); }
    };
    document.addEventListener('keydown', escHandler, true);
  }

  listen('screen:show-picker', (data) => {
    showScreenPicker(data?.sources || [], data?.audioApps || [], data?.requestId || null);
  });

  // ── DOMContentLoaded: voice state, login switcher, GDM ───
  function normalizeServerUrl(input) {
    let value = String(input || '').trim();
    if (!value) return '';
    if (!/^https?:\/\//i.test(value)) value = 'https://' + value;
    try {
      const parsed = new URL(value);
      parsed.hash = '';
      parsed.search = '';
      let pathname = parsed.pathname || '/';
      pathname = pathname.replace(/\/+$/, '') || '/';
      pathname = pathname.replace(/\/app(?:\.html)?$/i, '') || '/';
      pathname = pathname.replace(/\/+$/, '') || '/';
      return pathname === '/' ? parsed.origin : parsed.origin + pathname;
    } catch {
      return value.replace(/\/+$/, '');
    }
  }

  window.addEventListener('DOMContentLoaded', () => {
    try { localStorage.removeItem('haven_voice_channel'); } catch {}
    installGetDisplayMediaOverride();

    const css = document.createElement('style');
    css.textContent = `
      #haven-switch-server-btn{position:fixed;bottom:32px;left:50%;transform:translateX(-50%);z-index:9998;padding:8px 24px;
        background:var(--bg-card,#1a1a2e);border:1px solid var(--border,#444);border-radius:8px;color:var(--text-secondary,#aaa);
        font-size:13px;cursor:pointer;box-shadow:0 2px 8px rgba(0,0,0,.3)}
      #haven-switch-server-btn:hover{background:var(--bg-hover,rgba(255,255,255,.08));color:var(--text-primary,#fff);border-color:var(--accent,#6b4fdb)}
      #haven-server-picker-overlay{position:fixed;inset:0;background:rgba(0,0,0,.7);z-index:99999;display:flex;align-items:center;justify-content:center}
      #haven-server-picker{background:var(--bg-card,#1a1a2e);border:1px solid var(--border,#444);border-radius:12px;padding:24px;width:400px;max-width:90vw;max-height:80vh;overflow-y:auto;box-shadow:0 8px 32px rgba(0,0,0,.5)}
      #haven-server-picker h3{margin:0 0 16px;color:var(--text-primary,#fff);font-size:18px;text-align:center}
      .hsp-form{display:flex;gap:8px}
      .hsp-form input{flex:1;padding:8px 12px;border-radius:6px;border:1px solid var(--border,#444);background:var(--bg-primary,#0d0d1a);color:var(--text-primary,#fff);font-size:13px}
      .hsp-form button{padding:8px 16px;border-radius:6px;border:none;background:var(--accent,#6b4fdb);color:#fff;font-size:13px;cursor:pointer}
      .hsp-error{color:#ef4444;font-size:12px;margin-top:8px;text-align:center}
      .hsp-divider-label{color:var(--text-muted,#666);font-size:11px;text-transform:uppercase;margin:16px 0 8px;padding-bottom:4px;border-bottom:1px solid var(--border,#333)}
      .hsp-server-item{display:flex;align-items:center;padding:8px 10px;border-radius:6px;cursor:pointer}
      .hsp-server-item:hover{background:var(--bg-hover,rgba(255,255,255,.05))}
      .hsp-server-info{flex:1;min-width:0;display:flex;flex-direction:column;gap:2px}
      .hsp-server-name{color:var(--text-primary,#fff);font-size:13px;font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
      .hsp-server-url-label{color:var(--text-muted,#666);font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
      .hsp-remove-btn{background:transparent;border:none;color:var(--text-muted,#666);font-size:18px;cursor:pointer;padding:4px 8px}
      .hsp-cancel{display:block;width:100%;margin-top:16px;padding:8px;background:transparent;border:1px solid var(--border,#444);border-radius:6px;color:var(--text-secondary,#aaa);font-size:13px;cursor:pointer}
    `;
    document.head.appendChild(css);

    const currentUrl = normalizeServerUrl(window.location.href);
    fetch('/api/public-config').then((r) => r.json()).then((d) => {
      if (d.server_title) {
        invoke('server_history_update_name', { url: currentUrl, name: d.server_title }).catch(() => {});
      }
    }).catch(() => {});

    if (!document.querySelector('.auth-page') || !document.querySelector('.auth-container')) return;

    const switchBtn = document.createElement('button');
    switchBtn.id = 'haven-switch-server-btn';
    switchBtn.textContent = '⬡ Switch Server';
    document.body.appendChild(switchBtn);

    const overlay = document.createElement('div');
    overlay.id = 'haven-server-picker-overlay';
    overlay.style.display = 'none';
    overlay.innerHTML = `
      <div id="haven-server-picker">
        <h3>Switch Server</h3>
        <div class="hsp-form">
          <input type="text" id="hsp-url-input" placeholder="https://haven.example.com" spellcheck="false" autocomplete="off">
          <button id="hsp-connect-btn">Connect</button>
        </div>
        <div id="hsp-error" class="hsp-error" style="display:none"></div>
        <div id="hsp-recent-section" style="display:none">
          <div class="hsp-divider-label">Recent Servers</div>
          <div id="hsp-recent-list"></div>
        </div>
        <button id="hsp-cancel-btn" class="hsp-cancel">Cancel</button>
      </div>`;
    document.body.appendChild(overlay);

    const urlInput = document.getElementById('hsp-url-input');
    const connectBtn = document.getElementById('hsp-connect-btn');
    const errorEl = document.getElementById('hsp-error');

    async function loadRecentServers() {
      const history = await invoke('server_history_get').catch(() => []);
      const recentSection = document.getElementById('hsp-recent-section');
      const recentList = document.getElementById('hsp-recent-list');
      const filtered = (history || []).filter((h) => normalizeServerUrl(h.url) !== currentUrl);
      if (!filtered.length) {
        recentSection.style.display = 'none';
        return;
      }
      recentSection.style.display = 'block';
      recentList.innerHTML = '';
      filtered.sort((a, b) => (b.lastConnected || 0) - (a.lastConnected || 0));
      filtered.forEach((entry) => {
        const item = document.createElement('div');
        item.className = 'hsp-server-item';
        const info = document.createElement('div');
        info.className = 'hsp-server-info';
        let displayName;
        try {
          displayName = (entry.name && entry.name !== entry.url) ? entry.name : new URL(entry.url).hostname;
        } catch {
          displayName = entry.url;
        }
        const nameSpan = document.createElement('span');
        nameSpan.className = 'hsp-server-name';
        nameSpan.textContent = displayName;
        const urlSpan = document.createElement('span');
        urlSpan.className = 'hsp-server-url-label';
        urlSpan.textContent = entry.url;
        info.appendChild(nameSpan);
        info.appendChild(urlSpan);
        info.onclick = () => invoke('nav_change_primary_server', { serverUrl: entry.url });
        const removeBtn = document.createElement('button');
        removeBtn.className = 'hsp-remove-btn';
        removeBtn.textContent = '\u00d7';
        removeBtn.onclick = async (e) => {
          e.stopPropagation();
          await invoke('server_history_remove', { url: entry.url });
          await loadRecentServers();
        };
        item.appendChild(info);
        item.appendChild(removeBtn);
        recentList.appendChild(item);
      });
    }

    switchBtn.onclick = async () => {
      overlay.style.display = 'flex';
      urlInput.value = '';
      errorEl.style.display = 'none';
      urlInput.focus();
      await loadRecentServers();
    };
    document.getElementById('hsp-cancel-btn').onclick = () => { overlay.style.display = 'none'; };
    overlay.onclick = (e) => { if (e.target === overlay) overlay.style.display = 'none'; };
    urlInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !connectBtn.disabled) connectBtn.click();
    });

    connectBtn.onclick = async () => {
      let url = urlInput.value.trim();
      errorEl.style.display = 'none';
      if (!url) return;
      url = normalizeServerUrl(url);
      if (!url || !/^https?:\/\//i.test(url)) {
        errorEl.textContent = 'Please enter a valid URL.';
        errorEl.style.display = 'block';
        return;
      }
      connectBtn.disabled = true;
      connectBtn.textContent = 'Connecting...';
      try {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 8000);
        const res = await fetch(url + '/api/health', { signal: controller.signal }).catch(() => null);
        clearTimeout(timeout);
        if (!res || !res.ok) {
          errorEl.textContent = 'Could not reach server.';
          errorEl.style.display = 'block';
          return;
        }
        await invoke('nav_change_primary_server', { serverUrl: url });
      } catch {
        errorEl.textContent = 'Connection failed.';
        errorEl.style.display = 'block';
      } finally {
        connectBtn.disabled = false;
        connectBtn.textContent = 'Connect';
      }
    };
  });

  console.log('[Haven Desktop] Tauri app-bridge ready');
})();
