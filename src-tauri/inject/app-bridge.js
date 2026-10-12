// Haven Desktop — Tauri app-window initialization script
// Injected before page scripts (withGlobalTauri). Classic JS, not a module.
(function () {
  'use strict';

  async function invoke(cmd, args = {}) {
    const core = window.__TAURI__?.core;
    if (!core?.invoke) throw new Error('Tauri unavailable');
    return core.invoke(cmd, args);
  }
  invoke('nav_page_ready').catch(() => {});
  const userActedJustNow = (() => { try { const get = Object.getOwnPropertyDescriptor(UserActivation.prototype, 'isActive').get, act = navigator.userActivation, apply = Reflect.apply; return () => apply(get, act, []) === true; } catch { return () => false; } })();
  const listAction = (opts) => ({ user: !!(opts && typeof opts === 'object' && opts.user === true), fresh: userActedJustNow() });

  // ── Loading indicator overlay (eliminates theme & customization setup flicker) ──
  (function initLoadingOverlay() {
    try {
      const p = window.location.pathname || '';
      const h = window.location.host || '';
      if (p.includes('connection-error.html') || p.includes('splash.html') || h.includes('tauri.localhost')) {
        return;
      }
    } catch {
      return;
    }

    // Fast-path redirect for unauthenticated sessions accessing /app:
    // If we land on /app without a token, Haven's app.js will unconditionally
    // redirect to '/' via window.location.href = '/'. Doing it immediately here
    // avoids downloading/parsing 380 KB of app scripts and eliminates the
    // double-navigation flicker before reaching the login screen.
    try {
      const p = window.location.pathname || '';
      const isAppPath = p === '/app' || p.startsWith('/app/') || p.includes('app.html');
      const hasToken = !!localStorage.getItem('haven_token');
      if (isAppPath && !hasToken) {
        window.location.replace('/');
        return;
      }
    } catch {}

    const style = document.createElement('style');
    style.id = 'haven-desktop-loader-style';
    style.textContent = `
      html.haven-loading-active,
      html.haven-loading-active body {
        overflow: hidden !important;
      }
      #haven-desktop-loading-overlay {
        position: fixed !important;
        top: 0 !important; left: 0 !important; right: 0 !important; bottom: 0 !important;
        width: 100vw !important; height: 100vh !important;
        background: #0d0d1a !important;
        z-index: 2147483647 !important;
        display: flex !important;
        flex-direction: column !important;
        align-items: center !important;
        justify-content: center !important;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif !important;
        color: #e0e0e0 !important;
        user-select: none !important;
        pointer-events: all !important;
        opacity: 1 !important;
        visibility: visible !important;
        transition: opacity 0.28s cubic-bezier(0.4, 0, 0.2, 1), visibility 0.28s !important;
      }
      #haven-desktop-loading-overlay.hdl-fade-out {
        opacity: 0 !important;
        visibility: hidden !important;
        pointer-events: none !important;
      }
      .hdl-container {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 16px;
        text-align: center;
        padding: 24px;
      }
      .hdl-hex {
        font-size: 54px;
        line-height: 1;
        color: #8b6ce7;
        animation: hdlPulse 1.6s ease-in-out infinite;
      }
      .hdl-title {
        font-size: 20px;
        font-weight: 700;
        letter-spacing: 0.02em;
        color: #fff;
        margin: 0;
      }
      .hdl-spinner {
        width: 26px;
        height: 26px;
        border: 2.5px solid rgba(139, 108, 231, 0.22);
        border-top-color: #8b6ce7;
        border-radius: 50%;
        animation: hdlSpin 0.75s linear infinite;
      }
      .hdl-status {
        font-size: 13px;
        color: #9a96b0;
        margin: 0;
        min-height: 18px;
        transition: color 0.2s ease;
      }
      .hdl-back-btn {
        margin-top: 10px;
        background: #1a1a34;
        color: #e0e0e0;
        border: 1px solid #2a2a50;
        border-radius: 6px;
        padding: 8px 16px;
        font-size: 13px;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s ease;
      }
      .hdl-back-btn:hover {
        background: #2a2a50;
        border-color: #6b4fdb;
        color: #fff;
      }
      @keyframes hdlPulse {
        0%, 100% { opacity: 0.55; transform: scale(0.96); }
        50% { opacity: 1; transform: scale(1.04); }
      }
      @keyframes hdlSpin {
        to { transform: rotate(360deg); }
      }
    `;

    const overlay = document.createElement('div');
    overlay.id = 'haven-desktop-loading-overlay';
    overlay.innerHTML = `
      <div class="hdl-container">
        <div class="hdl-hex" aria-hidden="true">⬡</div>
        <h1 class="hdl-title">Haven</h1>
        <div class="hdl-spinner" aria-label="Loading"></div>
        <p class="hdl-status" id="hdl-status-text">Loading Haven…</p>
        <button class="hdl-back-btn" id="hdl-back-btn" style="display:none">← Pick a Different Server</button>
      </div>
    `;

    function tryMount() {
      const root = document.documentElement || document.body;
      if (!root) return false;
      if (!document.getElementById('haven-desktop-loader-style')) {
        root.appendChild(style);
      }
      if (!document.getElementById('haven-desktop-loading-overlay')) {
        root.appendChild(overlay);
        root.classList.add('haven-loading-active');
        const btn = overlay.querySelector('#hdl-back-btn');
        if (btn) {
          btn.onclick = (e) => {
            e.stopPropagation();
            invoke('nav_back_to_welcome').catch(() => {});
          };
        }
      }
      return true;
    }

    if (!tryMount()) {
      const rootObserver = new MutationObserver(() => {
        if (tryMount()) {
          rootObserver.disconnect();
        }
      });
      rootObserver.observe(document, { childList: true });
    }
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', tryMount, { once: true });
    }
    window.addEventListener('load', tryMount, { once: true });

    // If server load is slow or hanging (>2.5s), show back button on the loader
    const slowTimer = setTimeout(() => {
      const statusEl = document.getElementById('hdl-status-text');
      const btn = document.getElementById('hdl-back-btn');
      if (statusEl) statusEl.textContent = 'Server is taking longer than usual to respond…';
      if (btn) btn.style.display = 'inline-block';
    }, 2500);

    let dismissed = false;
    let dismissing = false;
    const startTime = Date.now();

    function dismissLoader() {
      if (dismissed) return;
      dismissed = true;
      clearTimeout(slowTimer);
      const root = document.documentElement;
      if (root) root.classList.remove('haven-loading-active');
      const el = document.getElementById('haven-desktop-loading-overlay');
      if (el) {
        el.classList.add('hdl-fade-out');
        setTimeout(() => {
          try { el.remove(); } catch {}
          try { style.remove(); } catch {}
        }, 300);
      }
    }

    function areThemeStylesheetsReady() {
      const sheets = document.querySelectorAll(
        'link[id^="haven-theme-"], link[id="haven-theme-file-early"], link[href*="/themes/"], link[id^="haven-plugin-style-"], link[data-theme-loading]'
      );
      for (const link of sheets) {
        if (link.getAttribute('data-theme-loading') === '1') return false;
        if (!link.sheet) return false;
        try {
          if (link.sheet.cssRules && link.sheet.cssRules.length === 0 && !link.sheet.href) {
            return false;
          }
        } catch {}
      }
      return true;
    }

    function isPluginLoaderReady() {
      if (document.documentElement.hasAttribute('data-haven-customizations-ready')) return true;
      const loader = window.HavenPluginLoader;
      if (!loader) return false;
      if (typeof loader.isReady === 'boolean') return loader.isReady;
      if (loader.loadedThemes && loader.loadedThemes.size > 0) return true;
      if (document.documentElement.hasAttribute('data-haven-safe-mode')) return true;
      return false;
    }

    function areSocketPrefsSettled() {
      const hasToken = !!(
        (window.app && window.app.token) ||
        (() => { try { return localStorage.getItem('token'); } catch { return null; } })()
      );
      if (!hasToken) return true;
      if (window.app) {
        if (window.app._userPrefs !== undefined) return true;
        if (window.app.socket && window.app.socket.disconnected && window.app.socket.active === false) {
          return true;
        }
      }
      return false;
    }

    function areFontsReady() {
      if (document.fonts && document.fonts.status === 'loading') return false;
      return true;
    }

    function isAuthPage() {
      return !!(
        document.querySelector('.auth-page') ||
        document.querySelector('.auth-container') ||
        document.querySelector('#login-form') ||
        document.querySelector('#register-form')
      );
    }

    function isAppPage() {
      return !!(
        document.getElementById('app') ||
        document.getElementById('app-body') ||
        document.querySelector('.sidebar') ||
        document.querySelector('.channel-list')
      );
    }

    function isAuthPageReady(elapsed) {
      if (document.readyState === 'loading') return false;

      // Check if i18n has translated the login submit button
      const submitBtn = document.querySelector('button[type="submit"]');
      if (submitBtn && submitBtn.textContent && submitBtn.textContent.includes('auth.login.')) {
        return false;
      }

      // Check if auth theme bar / version has loaded from /api/themes and /api/public-config
      const versionEl = document.getElementById('auth-version');
      const themeBar = document.getElementById('auth-theme-bar');
      const themeBarPopulated = !!(themeBar && themeBar.children && themeBar.children.length > 0);
      const versionLoaded = !!(versionEl && versionEl.textContent && versionEl.textContent.trim().length > 0);

      // Give auth.js up to 700ms to fetch /api/themes and /api/public-config
      if (!themeBarPopulated && !versionLoaded && elapsed < 700) {
        return false;
      }

      // Check if theme stylesheets are still downloading / loading
      if (!areThemeStylesheetsReady()) return false;

      // Check if fonts are ready
      if (!areFontsReady() && elapsed < 1200) return false;

      return true;
    }

    function isAppPageReady(elapsed) {
      if (document.readyState === 'loading') return false;

      // Check if theme is still pending
      if (document.documentElement.hasAttribute('data-haven-theme-pending')) return false;

      // Check theme stylesheets downloading/parsing
      if (!areThemeStylesheetsReady()) return false;

      // Check plugin/theme loader
      if (!isPluginLoaderReady() && elapsed < 2600) return false;

      // Check socket preferences
      if (!areSocketPrefsSettled() && elapsed < 2000) return false;

      // Check web fonts
      if (!areFontsReady() && elapsed < 1500) return false;

      return true;
    }

    function finishAndDismiss() {
      if (dismissed || dismissing) return;
      dismissing = true;
      const statusEl = document.getElementById('hdl-status-text');
      if (statusEl) statusEl.textContent = 'Ready';

      // Double requestAnimationFrame guarantees that style recalculation, layout reflow,
      // and paint to the GPU backbuffer have fully executed before revealing the window.
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          setTimeout(dismissLoader, 70);
        });
      });
    }

    function checkReady() {
      if (dismissed || dismissing) return;
      const elapsed = Date.now() - startTime;

      if (isAuthPage()) {
        if (isAuthPageReady(elapsed)) {
          finishAndDismiss();
          return;
        }
      } else if (isAppPage()) {
        if (isAppPageReady(elapsed)) {
          finishAndDismiss();
          return;
        }
      }

      // Hard safety timeout: after 4.5s, always dismiss
      if (elapsed > 4500) {
        dismissLoader();
      }
    }

    const interval = setInterval(() => {
      checkReady();
      if (dismissed) clearInterval(interval);
    }, 35);

    // Listen for custom readiness events
    window.addEventListener('haven:customizations-ready', checkReady);
    window.addEventListener('haven:theme-changed', checkReady);
    window.addEventListener('haven:plugins-loaded', checkReady);
    if (document.fonts && document.fonts.ready) {
      document.fonts.ready.then(checkReady).catch(() => {});
    }

    // Safety timeout
    setTimeout(dismissLoader, 4800);
  })();

  // ── Suppress false STUN/TURN unreachable warnings ──────────
  // WebKitGTK compiled without WebRTC has undefined RTCPeerConnection, causing false
  // "All STUN servers unreachable" errors across remote servers (e.g. haven.moviethingy.xyz).
  // 1. Purge any false STUN toasts from DOM immediately via MutationObserver.
  const noRtc = typeof window.RTCPeerConnection === 'undefined';
  const isStunWarning = (text) => {
    return noRtc && typeof text === 'string' && (
      text.includes('Voice connection servers (STUN) are unreachable') ||
      text.includes('Configured voice servers (STUN) are unreachable') ||
      text.includes('STUN servers failed probe')
    );
  };

  const killStunToast = (node) => {
    try {
      if (node && node.nodeType === 1) {
        if (isStunWarning(node.textContent)) {
          console.info('[haven-desktop] Purged false STUN toast from DOM');
          node.remove();
        }
      }
    } catch {}
  };
  try {
    const toastObs = new MutationObserver((mutations) => {
      for (const m of mutations) {
        for (const n of m.addedNodes) {
          killStunToast(n);
          if (n.querySelectorAll) n.querySelectorAll('.toast').forEach(killStunToast);
        }
      }
    });
    const root = document.documentElement || document;
    toastObs.observe(root, { childList: true, subtree: true });
  } catch {}

  // 2. Synchronously intercept window.app and patch _showToast and voice.
  let _havenApp = undefined;
  function patchApp(inst) {
    if (!inst || inst.__havenAppPatched) return;
    inst.__havenAppPatched = true;
    try {
      // Ensure channel template methods never throw TypeError on click
      if (typeof inst._channelTemplateExtras !== 'function') {
        inst._channelTemplateExtras = function () {
          return {
            topic: '',
            readOnly: false,
            announcement: false,
            slowMode: 0,
            mediaEnabled: true,
            voiceEnabled: true
          };
        };
      }
      if (typeof inst._resetChannelTemplate !== 'function') {
        inst._resetChannelTemplate = function () {};
      }

      const origProto = Object.getPrototypeOf(inst);
      const origToast = (origProto && origProto._showToast) || inst._showToast;
      const safeToast = function (msg, ...args) {
        if (isStunWarning(msg)) {
          console.info('[haven-desktop] Dropped false STUN toast:', msg);
          return;
        }
        return origToast ? origToast.call(this, msg, ...args) : undefined;
      };
      inst._showToast = safeToast;
      if (origProto) origProto._showToast = safeToast;

      const sanitizeVoice = (v) => {
        if (!v || v.__havenVoicePatched || !noRtc) return;
        v.__havenVoicePatched = true;
        v.onConnectivityWarning = () => {};
        v._probeDefaultStun = () => Promise.resolve();
        v._probeConfiguredStun = () => Promise.resolve();
      };
      if (inst.voice) sanitizeVoice(inst.voice);
      let _v = inst.voice;
      try {
        Object.defineProperty(inst, 'voice', {
          configurable: true,
          enumerable: true,
          get() { return _v; },
          set(v) { _v = v; if (v) sanitizeVoice(v); }
        });
      } catch {}

      const hookChannelSocket = (s) => {
        if (!s || s.__havenChannelHooked) return;
        s.__havenChannelHooked = true;
        s.on('channel-joined', (ch) => {
          if (!ch || !ch.code) return;
          // Haven re-emits join-channel on reconnect/refresh, so only react
          // to a join the user just asked for through the rescue below.
          const pending = window.__havenJoinPending;
          if (!pending) return;
          const fresh = Date.now() - pending.at < 5000;
          if (pending.code !== ch.code && !fresh) return;
          window.__havenJoinPending = null;
          try {
            if (inst.channels && !inst.channels.some((c) => c.code === ch.code)) {
              inst.channels.push(ch);
              inst._renderChannels?.();
            }
            inst._showToast?.(`Joined #${ch.name || ch.code}`, 'success');
            setTimeout(() => {
              inst.switchChannel?.(ch.code);
            }, 60);
          } catch (err) {
            console.warn('[haven-desktop] channel-joined auto-switch error:', err);
          }
        });
      };
      if (inst.socket) hookChannelSocket(inst.socket);
      let _s = inst.socket;
      try {
        Object.defineProperty(inst, 'socket', {
          configurable: true,
          enumerable: true,
          get() { return _s; },
          set(s) { _s = s; if (s) hookChannelSocket(s); }
        });
      } catch {}
    } catch (e) {
      console.warn('[haven-desktop] patchApp error:', e);
    }
  }

  // ── Universal Channel Join & Create Rescue ─────────────────
  // Resolves non-working "Join" and "Create" / "+" channel buttons across all OSs
  // and layout plugins (standard sidebar, mod mode, ChannelTabsLayout modal).
  // 1. Uncaps input maxlength and parses full invite URLs (https://.../c/<code>, ?invite=<code>)
  // 2. Safely extracts payloads and spreads channel template options without throwing
  // 3. Emits join-channel and create-channel directly through the active socket
  // 4. Provides clear toast notifications and auto-switches to newly joined channels
  (function initChannelControlsRescue() {
    function sanitizeChannelCode(raw) {
      if (!raw || typeof raw !== 'string') return '';
      let s = raw.trim();
      const matchC = s.match(/(?:\/c\/|\/join\/)([a-zA-Z0-9_-]+)/i);
      if (matchC) return matchC[1];
      const matchInvite = s.match(/[?&]invite=([a-zA-Z0-9_-]+)/i);
      if (matchInvite) return matchInvite[1];
      try {
        if (s.startsWith('http://') || s.startsWith('https://')) {
          const u = new URL(s);
          const parts = u.pathname.split('/').filter(Boolean);
          if (parts.length > 0) return parts[parts.length - 1];
        }
      } catch {}
      if (/^[a-fA-F0-9]{8}$/.test(s)) return s.toLowerCase();
      return s;
    }

    function ensureInputAttributes() {
      const codeInput = document.getElementById('channel-code-input');
      if (codeInput) {
        if (codeInput.hasAttribute('maxlength') && codeInput.getAttribute('maxlength') === '8') {
          codeInput.setAttribute('maxlength', '256');
        }
        if (!codeInput.__havenPasteHooked) {
          codeInput.__havenPasteHooked = true;
          codeInput.addEventListener('paste', (e) => {
            const pasted = e.clipboardData?.getData('text/plain');
            if (pasted) {
              const cleaned = sanitizeChannelCode(pasted);
              if (cleaned && cleaned !== pasted) {
                e.preventDefault();
                codeInput.value = cleaned;
                codeInput.dispatchEvent(new Event('input', { bubbles: true }));
              }
            }
          });
        }
      }
    }

    function executeJoinChannel() {
      const codeInput = document.getElementById('channel-code-input');
      if (!codeInput) return false;
      const raw = codeInput.value;
      const code = sanitizeChannelCode(raw);
      if (!code) {
        window.app?._showToast?.('Please enter a channel code or invite link', 'error');
        return false;
      }
      const sock = window.app?.socket || window.havenSocket;
      if (!sock) {
        window.app?._showToast?.('Not connected to server', 'error');
        return false;
      }
      codeInput.value = '';
      window.app?._showToast?.(`Joining channel (${code})...`, 'info');
      window.__havenJoinPending = { code, at: Date.now() };
      sock.emit('join-channel', { code });

      const modal = document.getElementById('join-create-modal');
      if (modal && modal.style.display !== 'none') {
        modal.style.display = 'none';
        const channelsTab = document.querySelector('[data-channel-tab="channels"]');
        if (channelsTab) channelsTab.click();
      }
      return true;
    }

    function executeCreateChannel() {
      const nameInput = document.getElementById('new-channel-name');
      if (!nameInput) return false;
      const name = nameInput.value.trim();
      if (!name) {
        window.app?._showToast?.('Please enter a channel name', 'error');
        return false;
      }
      const sock = window.app?.socket || window.havenSocket;
      if (!sock) {
        window.app?._showToast?.('Not connected to server', 'error');
        return false;
      }

      const isPrivate = document.getElementById('new-channel-private')?.checked || false;
      const temporary = document.getElementById('new-channel-temporary')?.checked || false;
      const duration = parseInt(document.getElementById('new-channel-duration')?.value, 10) || 24;
      const addAllMembers = document.getElementById('new-channel-add-all')?.checked || false;
      const isForum = document.getElementById('new-channel-forum')?.checked || false;

      let extras = {};
      if (typeof window.app?._channelTemplateExtras === 'function') {
        try { extras = window.app._channelTemplateExtras() || {}; } catch {}
      }

      const payload = {
        name,
        isPrivate,
        temporary,
        duration,
        addAllMembers,
        isForum,
        ...extras
      };

      sock.emit('create-channel', payload);
      if (typeof window.app?._resetChannelTemplate === 'function') {
        try { window.app._resetChannelTemplate(); } catch {}
      }
      nameInput.value = '';
      const pvt = document.getElementById('new-channel-private');
      if (pvt) pvt.checked = false;
      const tmp = document.getElementById('new-channel-temporary');
      if (tmp) tmp.checked = false;
      const all = document.getElementById('new-channel-add-all');
      if (all) all.checked = false;
      const durRow = document.getElementById('temp-channel-duration-row');
      if (durRow) durRow.style.display = 'none';

      window.app?._showToast?.(`Creating #${name}...`, 'info');

      const modal = document.getElementById('join-create-modal');
      if (modal && modal.style.display !== 'none') {
        modal.style.display = 'none';
        const channelsTab = document.querySelector('[data-channel-tab="channels"]');
        if (channelsTab) channelsTab.click();
      }
      return true;
    }

    // Delegated capture click listener
    document.addEventListener('click', (e) => {
      if (e.target?.closest?.('#join-section-close')) {
        e.preventDefault();
        e.stopPropagation();
        const body = document.getElementById('join-section-body');
        const arrow = document.getElementById('join-section-arrow');
        body?.classList.add('collapsed');
        arrow?.classList.add('collapsed');
        localStorage.setItem('haven_join_collapsed', '1');
        return;
      }
      if (e.target?.closest?.('#create-section-close')) {
        e.preventDefault();
        e.stopPropagation();
        const body = document.getElementById('create-section-body');
        const arrow = document.getElementById('create-section-arrow');
        body?.classList.add('collapsed');
        arrow?.classList.add('collapsed');
        localStorage.setItem('haven_create_collapsed', '1');
        return;
      }
      if (e.target?.closest?.('#join-create-modal-close, #close-join-create-btn')) {
        e.preventDefault();
        e.stopPropagation();
        const modal = document.getElementById('join-create-modal');
        if (modal) modal.style.display = 'none';
        return;
      }
      if (e.target?.closest?.('#join-channel-btn')) {
        e.preventDefault();
        e.stopPropagation();
        executeJoinChannel();
        return;
      }
      if (e.target?.closest?.('#create-channel-btn')) {
        e.preventDefault();
        e.stopPropagation();
        executeCreateChannel();
        return;
      }
    }, true);

    // Delegated capture keydown listener for Enter key
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        if (e.target?.id === 'channel-code-input') {
          e.preventDefault();
          e.stopPropagation();
          executeJoinChannel();
          return;
        }
        if (e.target?.id === 'new-channel-name') {
          e.preventDefault();
          e.stopPropagation();
          executeCreateChannel();
          return;
        }
      }
    }, true);

    // Periodic & DOM check for input attributes
    setInterval(ensureInputAttributes, 1000);
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', ensureInputAttributes, { once: true });
    } else {
      ensureInputAttributes();
    }
  })();

  try {
    Object.defineProperty(window, 'app', {
      configurable: true,
      enumerable: true,
      get() { return _havenApp; },
      set(inst) {
        _havenApp = inst;
        if (inst) patchApp(inst);
      }
    });
  } catch {}

  let havenFocused = document.hasFocus();
  const patchSocketEmit = () => {
    const proto = window.io?.Socket?.prototype;
    if (!proto || proto.__havenFocusPatched) return !!proto;
    const orig = proto.emit;
    proto.emit = function (ev, data, ...rest) { return orig.call(this, ev, ev === 'visibility-change' && data && typeof data.visible === 'boolean' ? { ...data, visible: data.visible && havenFocused } : data, ...rest); };
    proto.__havenFocusPatched = true;
    return true;
  };
  let patchTries = 0;
  const patchTimer = setInterval(() => (patchSocketEmit() || ++patchTries > 480) && clearInterval(patchTimer), 250);
  window.__havenFocus = (f) => { havenFocused = !!f; window.app?.socket?.emit?.('visibility-change', { visible: !document.hidden }); };
  document.addEventListener('DOMContentLoaded', () => { havenFocused = document.hasFocus(); });
  window.addEventListener('focus', () => havenFocused || window.__havenFocus(true));
  // WebKitGTK keeps document.hidden false while the window sits unfocused, and
  // Haven only pops a banner for the open channel when the page is hidden, so
  // those messages played the sound with no banner. Fire it here instead, unless
  // the server's own client already folds focus in (_pageUnseen).
  const hookFocusNotify = () => {
    const a = window.app, s = a?.socket;
    if (!s?.on || !a.user || s.__havenFocusNotify) return;
    s.__havenFocusNotify = true;
    s.on('new-message', (d) => {
      try {
        const m = d?.message;
        if (havenFocused || document.hidden || !m || d.channelCode !== a.currentChannel || m.user_id === a.user?.id) return;
        if (typeof a._pageUnseen === 'function' || typeof a._fireNativeNotification !== 'function' || a._isDesktopBackgroundView?.()) return;
        if (localStorage.getItem('haven_server_muted') === '1' || JSON.parse(localStorage.getItem('haven_muted_channels') || '[]').includes(d.channelCode)) return;
        const c = m.content || '', me = (a.user.username || '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), ch = a.channels?.find((x) => x.code === d.channelCode);
        const mention = (me && new RegExp(`@${me}(?!\\w)`, 'i').test(c)) || /(?<![\w@])@(everyone|here)\b/i.test(c) || a._mentionsMyRole?.(c);
        a._fireNativeNotification(m, d.channelCode, mention ? { isMention: true } : m.replyContext?.user_id === a.user.id ? { isReply: true } : ch?.is_dm ? { isDm: true } : null);
      } catch (e) { console.warn('[haven-desktop] focus notify', e); }
    });
  };
  setInterval(hookFocusNotify, 2000);

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

  const nativeOpen = window.open;
  const popup = (href, target, features) => (nativeOpen && nativeOpen.call(window, href, target, features)) || { closed: false, close() { this.closed = true; }, focus() {}, blur() {}, postMessage() {}, location: { href } };
  window.open = function (url, target, features) {
    try {
      const u = new URL(url, window.location.href);
      if (u.origin === window.location.origin) {
        if (/^\/(app(\.html)?|c\/[A-Za-z0-9]+)/.test(u.pathname) || u.searchParams.has('channel') || u.searchParams.has('message')) {
          window.location.assign(u.href);
          return null;
        }
        return popup(u.href, target, features);
      }
      if (/^\/api\/auth\/SSO$/i.test(u.pathname)) return popup(u.href, target, features);
      if (u.protocol === 'http:' || u.protocol === 'https:') invoke('open_external', { url: u.href }).catch(() => {});
    } catch {}
    return null;
  };
  document.addEventListener('click', (e) => {
    const a = e.target && e.target.closest && e.target.closest('a[href]');
    if (!a || a.target !== '_blank' || a.hasAttribute('download')) return;
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

    switchServer: (url) => invoke('nav_switch_server', { serverUrl: url, action: listAction({ user: true }) }),
    backToWelcome: () => invoke('nav_back_to_welcome'),

    update: {
      download: async () => ({ errorKey: 'update.unavailable' }),
      install: () => {},
    },

    audio: {
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
    clipboardReadImage: () => invoke('clipboard_read_image'),
    clipboardWriteText: (text) => invoke('clipboard_write_text', { text }),
    saveImage: ({ bytes, filename } = {}) =>
      invoke('save_image', { payload: bytes || '', filename: filename || 'haven-image.png' }),

    getServerHistory: () => invoke('server_history_get'),
    serverListGated: true,
    addServerHistory: (url, name, opts) => invoke('server_history_add', { url, name: name == null ? null : String(name), opts: opts && typeof opts === 'object' ? opts : null, action: listAction(opts) }),
    removeServerHistory: (url, opts) => invoke('server_history_remove', { url, action: listAction(opts) }),
    getServerList: () => invoke('server_list_get'),
    setServerOrder: (urls) => invoke('server_list_set_order', { urls: Array.isArray(urls) ? urls : [] }),
    updateServerName: (url, name, opts) => invoke('server_history_update_name', { url, name: String(name ?? ''), opts: opts && typeof opts === 'object' ? opts : null, action: listAction(opts) }),
    initialServerList: null,
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
  invoke('server_list_get').then((l) => { window.havenDesktop.initialServerList = l || null; }).catch(() => {});
  listen('server-list:changed', () => window.dispatchEvent(new CustomEvent('haven-server-list-changed')));

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

  // ── Dialogs ──────────────────────────────────────────────
  // WebKitGTK and WebView2 show alert/confirm/prompt themselves and block the
  // page like a browser does, so confirm() returns a real boolean and
  // prompt() returns what was typed. Routing them through invoke made
  // confirm() return a Promise, which is always truthy: every
  // `if (!confirm(...)) return;` in Haven went ahead without asking, and the
  // native dialog it opened froze the app (removing a server did both).
  // tauri-plugin-dialog also swaps in async versions on every page, so the
  // browser's own functions are taken from a hidden blank iframe, which no
  // init script touches, and put back.
  // Only macOS keeps the invoke path, since WKWebView needs a UI delegate.
  if (!/Mac/.test(navigator.platform)) {
    let host = null;
    const nativeDialog = (name) => {
      if (!host || !host.isConnected || !host.contentWindow) {
        host = document.createElement('iframe');
        host.style.display = 'none';
        host.setAttribute('aria-hidden', 'true');
        (document.body || document.documentElement).appendChild(host);
      }
      return host.contentWindow[name].bind(host.contentWindow);
    };
    window.alert = (message) => nativeDialog('alert')(message);
    window.confirm = (message) => nativeDialog('confirm')(message);
    window.prompt = (message, defaultValue) => nativeDialog('prompt')(message, defaultValue);
  } else {
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
  }

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

  // ── Helper to route queued files directly into the active Haven composer ──
  function queueFilesIntoActiveHavenComposer(files, targetEl) {
    const app = window.app;
    if (!app || !files || files.length === 0) return false;

    const threadPanel = document.getElementById('thread-panel');
    const isThreadOpen = !!(threadPanel && threadPanel.style.display !== 'none' && !threadPanel.classList.contains('hidden'));
    const isThreadTarget = !!(
      targetEl?.id === 'thread-input' ||
      targetEl?.id === 'thread-message-input' ||
      targetEl?.closest?.('#thread-panel') ||
      targetEl?.closest?.('.thread-input-area') ||
      (isThreadOpen && targetEl?.id !== 'message-input' && targetEl?.id !== 'search-input' && !!app._activeThreadParent)
    );

    // 1. Thread panel composer
    if (isThreadTarget && (typeof app._queueThreadFiles === 'function' || typeof app._queueThreadFile === 'function')) {
      if (typeof app._queueThreadFiles === 'function') {
        app._queueThreadFiles(files);
      } else {
        for (const f of files) app._queueThreadFile(f);
      }
      const thInput = document.getElementById('thread-input') || document.getElementById('thread-message-input');
      if (thInput) { try { thInput.focus(); } catch (_) {} }
      return true;
    }

    // 2. DM PiP composer
    const pipContainer = targetEl?.closest?.('#dm-pip-container') || targetEl?.closest?.('#dm-pip-panel');
    if (pipContainer) {
      const code = pipContainer.dataset?.channelCode;
      if (code && typeof app._queueImageForPiP === 'function') {
        for (const f of files) {
          if (f.type && f.type.startsWith('image/')) {
            app._queueImageForPiP(f, code);
          } else if (typeof app._uploadGeneralFile === 'function') {
            app._uploadGeneralFile(f, code);
          }
        }
        return true;
      }
    }

    // 3. Forum post modal
    const forumModal = targetEl?.closest?.('.forum-post-modal');
    if (forumModal && typeof app._forumUploadIntoBody === 'function') {
      const bodyInput = forumModal.querySelector('#forum-post-body');
      const code = app.currentChannel;
      if (bodyInput && code) {
        app._forumUploadIntoBody(files, bodyInput, code);
        return true;
      }
    }

    // 4. Main chat composer
    if (typeof app._queueComposerFiles === 'function') {
      app._queueComposerFiles(files);
      const input = document.getElementById('message-input');
      if (input) { try { input.focus(); } catch (_) {} }
      return true;
    } else if (typeof app._queueImage === 'function' || typeof app._queueGeneralFile === 'function') {
      for (const f of files) {
        if (f.type && f.type.startsWith('image/') && typeof app._queueImage === 'function') {
          app._queueImage(f);
        } else if (typeof app._queueGeneralFile === 'function') {
          app._queueGeneralFile(f);
        } else if (typeof app._queueImage === 'function') {
          app._queueImage(f);
        }
      }
      const input = document.getElementById('message-input');
      if (input) { try { input.focus(); } catch (_) {} }
      return true;
    }

    return false;
  }

  // ── Helper to insert text into input or document ──
  function insertTextIntoTarget(text, targetEl) {
    if (!text) return;
    const threadPanel = document.getElementById('thread-panel');
    const isThreadOpen = !!(threadPanel && threadPanel.style.display !== 'none' && !threadPanel.classList.contains('hidden'));
    const threadInput = document.getElementById('thread-input') || document.getElementById('thread-message-input');
    const mainInput = document.getElementById('message-input');

    let el = targetEl;
    if (!el || el === document.body || el === document.documentElement) {
      if (isThreadOpen && threadInput && window.app && window.app._activeThreadParent) {
        el = threadInput;
      } else {
        el = document.activeElement || mainInput;
      }
    }
    if (!el) el = mainInput;
    if (!el) return;

    if (typeof el.selectionStart === 'number' && typeof el.selectionEnd === 'number') {
      const start = el.selectionStart;
      const end = el.selectionEnd;
      const val = el.value || '';
      el.value = val.slice(0, start) + text + val.slice(end);
      el.selectionStart = el.selectionEnd = start + text.length;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      return;
    }

    if (el.isContentEditable) {
      document.execCommand('insertText', false, text);
      return;
    }

    if (mainInput && mainInput !== el) {
      try { mainInput.focus(); } catch (_) {}
      insertTextIntoTarget(text, mainInput);
    }
  }

  // ── Thread attachment button injection & wiring ──────────
  function setupThreadUploadControls() {
    const threadInputArea = document.querySelector('.thread-input-area');
    if (!threadInputArea) return;

    let uploadBtn = document.getElementById('thread-upload-btn');
    let fileInput = document.getElementById('thread-file-input');

    if (!fileInput) {
      fileInput = document.createElement('input');
      fileInput.type = 'file';
      fileInput.id = 'thread-file-input';
      fileInput.multiple = true;
      fileInput.style.display = 'none';
      threadInputArea.appendChild(fileInput);
    }

    if (!uploadBtn) {
      uploadBtn = document.createElement('button');
      uploadBtn.id = 'thread-upload-btn';
      uploadBtn.className = 'pip-input-emoji-btn';
      uploadBtn.title = 'Upload File';
      uploadBtn.type = 'button';
      uploadBtn.setAttribute('aria-label', 'Upload File');
      uploadBtn.innerHTML = '<span class="tb-icon tb-icon-emoji" aria-hidden="true">📎</span><span class="tb-icon tb-icon-mono" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/></svg></span>';

      const emojiBtn = threadInputArea.querySelector('#thread-emoji-btn');
      if (emojiBtn) {
        threadInputArea.insertBefore(uploadBtn, emojiBtn);
      } else {
        const txt = threadInputArea.querySelector('#thread-input');
        if (txt) threadInputArea.insertBefore(uploadBtn, txt);
        else threadInputArea.appendChild(uploadBtn);
      }
    }

    if (!uploadBtn.__havenWired) {
      uploadBtn.__havenWired = true;
      uploadBtn.addEventListener('click', async (e) => {
        e.preventDefault();
        e.stopPropagation();
        try {
          const res = await invoke('pick_files_for_upload');
          if (Array.isArray(res)) {
            if (res.length > 0) {
              const files = [];
              for (const item of res) {
                if (item && item.dataBase64) {
                  const bin = atob(item.dataBase64);
                  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
                  files.push(new File([bytes], item.name, { type: item.mime }));
                }
              }
              if (files.length > 0) {
                queueFilesIntoActiveHavenComposer(files, document.getElementById('thread-input'));
              }
            }
            return;
          }
        } catch (_) {}
        const fi = document.getElementById('thread-file-input');
        if (fi) fi.click();
      });
    }

    if (fileInput && !fileInput.__havenWired) {
      fileInput.__havenWired = true;
      fileInput.addEventListener('change', () => {
        if (fileInput.files && fileInput.files.length) {
          const files = Array.from(fileInput.files);
          queueFilesIntoActiveHavenComposer(files, document.getElementById('thread-input'));
          fileInput.value = '';
        }
      });
    }
  }

  // Hook into DOM to ensure thread upload button is present
  try {
    const threadBtnObserver = new MutationObserver(() => {
      setupThreadUploadControls();
    });
    const rootEl = document.documentElement || document;
    threadBtnObserver.observe(rootEl, { childList: true, subtree: true });
  } catch (_) {}
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', setupThreadUploadControls);
  } else {
    setupThreadUploadControls();
  }
  window.addEventListener('load', setupThreadUploadControls);
  window.addEventListener('click', (e) => {
    if (e.target?.closest?.('#thread-panel, .thread-btn, [data-thread-id]')) {
      setTimeout(setupThreadUploadControls, 50);
    }
  }, true);

  // ── Paste rescue for WebKitGTK (Files & Pictures) ─────────
  // 1. Files copied in Linux file managers (Nautilus, Dolphin, Thunar) reach
  //    WebKitGTK with text/uri-list: file:///path/to/archive.zip and no items of
  //    kind 'file'. Read the file bytes via the desktop host and queue them
  //    directly into the Haven composer.
  // 2. Bitmaps copied to clipboard (WebKit bug 218519, screenshot tools, Win+V / Klipper)
  //    reach WebKitGTK with an empty or non-file DataTransfer, sometimes accompanied
  //    by a suggested filename (e.g. Screenshot_...png) or dimensions as plain text.
  //    Read the picture via clipboard_read_image and queue it directly into the Haven composer.
  (function () {
    if (detectPlatform() !== 'linux') return;
    let busy = false;
    document.addEventListener('paste', async (e) => {
      if (e.__havenReplay || busy) return;
      const dt = e.clipboardData;
      if (!dt) return;

      const target = e.target;
      const items = Array.from(dt.items || []);
      const realFiles = items
        .filter((i) => i.kind === 'file' && typeof i.getAsFile === 'function')
        .map(i => i.getAsFile())
        .filter(Boolean);

      // 1. If WebKit already gave us real File objects, route directly into active composer
      if (realFiles.length > 0) {
        e.preventDefault();
        queueFilesIntoActiveHavenComposer(realFiles, target);
        return;
      }

      const uriList = dt.getData('text/uri-list') || '';
      const plain = (dt.getData('text/plain') || '').trim();
      const types = Array.from(dt.types || []);

      // Check if clipboard contains file URIs or local file paths from DOM DataTransfer
      let candidateUris = [];
      if (uriList) {
        candidateUris = uriList.split(/[\r\n]+/).map(s => s.trim()).filter(s => s && !s.startsWith('#'));
      } else if (plain.startsWith('file://')) {
        candidateUris = plain.split(/[\r\n]+/).map(s => s.trim()).filter(s => s.startsWith('file://'));
      } else if (plain.startsWith('/') && !plain.includes('\n') && /\.[a-zA-Z0-9]+$/.test(plain)) {
        candidateUris = [plain];
      }

      // Check whether this paste is a candidate for image/file rescue vs normal plain text
      const isCandidate =
        candidateUris.length > 0 ||
        plain === '' ||
        !types.includes('text/plain') ||
        /\.(png|jpe?g|webp|gif|bmp|svg|avif|ico|tiff|zip|tar|gz|7z|rar|pdf|mp3|wav|ogg|mp4|webm|mkv)$/i.test(plain) ||
        /^Screenshot_|^screenshot_/i.test(plain) ||
        /^[\u25ae\u25a0]|\s*\[image/i.test(plain);

      // If it's pure normal text (e.g. typing a message, pasting a sentence), let native text paste happen
      if (!isCandidate) return;

      // Prevent WebKit from dumping file URIs or screenshot filenames into the input synchronously
      e.preventDefault();

      busy = true;
      const guard = setTimeout(() => { busy = false; }, 10000);

      try {
        const files = [];

        // 1. If DOM gave us candidate URIs, try reading them via host command
        if (candidateUris.length > 0) {
          for (const uri of candidateUris) {
            try {
              const res = await invoke('read_file_for_upload', { path: uri });
              if (res && res.ok && res.dataBase64) {
                const bin = atob(res.dataBase64);
                const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
                files.push(new File([bytes], res.name, { type: res.mime }));
              }
            } catch (err) {
              console.warn('[haven-desktop] failed to read file for upload:', uri, err);
            }
          }
        }

        // 2. If no files retrieved from URIs, check if clipboard has a bitmap image
        // (WebKitGTK bug 218519, Spectacle, Flameshot, Klipper/Win+V screenshots)
        if (files.length === 0) {
          try {
            const res = await invoke('clipboard_read_image');
            if (res && res.ok && res.dataUrl) {
              const mime = res.mime || 'image/png';
              const bin = atob(res.dataUrl.slice(res.dataUrl.indexOf(',') + 1));
              const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
              let filename = res.filename;
              if (!filename) {
                if (/\.(png|jpe?g|webp|gif|bmp|svg|avif)$/i.test(plain) && !plain.includes('\n')) {
                  filename = plain.split(/[\/\\]/).pop();
                } else {
                  const ext = mime === 'image/jpeg' ? 'jpg' : mime === 'image/webp' ? 'webp' : mime === 'image/gif' ? 'gif' : 'png';
                  filename = 'pasted-image.' + ext;
                }
              }
              files.push(new File([bytes], filename, { type: mime }));
            }
          } catch (err) {
            console.warn('[haven-desktop] clipboard_read_image failed:', err);
          }
        }

        // 3. If still no files, query native clipboard for copied files
        // (WebKitGTK blocks file access and text/uri-list from DOM DataTransfer during paste events)
        if (files.length === 0) {
          try {
            const res = await invoke('clipboard_read_files');
            if (res && res.ok && Array.isArray(res.files) && res.files.length > 0) {
              for (const item of res.files) {
                if (item && item.dataBase64) {
                  const bin = atob(item.dataBase64);
                  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
                  files.push(new File([bytes], item.name, { type: item.mime }));
                }
              }
            }
          } catch (err) {
            console.warn('[haven-desktop] clipboard_read_files failed:', err);
          }
        }

        // 4. If files or screenshot were found, queue them directly into Haven
        if (files.length > 0) {
          const queued = queueFilesIntoActiveHavenComposer(files, target);
          if (!queued) {
            const replay = new DataTransfer();
            for (const f of files) replay.items.add(f);
            const ev = new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: replay });
            ev.__havenReplay = true;
            (target || document.activeElement || document.getElementById('message-input') || document.body).dispatchEvent(ev);
          }
          return;
        }

        // 5. Fallback: if candidate check triggered but no file/image was found, insert plain text if present
        if (plain) {
          insertTextIntoTarget(plain, target);
        }
      } catch (err) {
        console.warn('[haven-desktop] paste rescue failed:', err);
        if (plain) {
          insertTextIntoTarget(plain, target);
        }
      } finally {
        clearTimeout(guard);
        busy = false;
      }
    }, true);
  })();

  // ── Drag & drop rescue for WebKitGTK ───────────────────────
  // Route any files dropped anywhere on the window (composer, chat history, or
  // background) into Haven's queueFilesIntoActiveHavenComposer and prevent WebKit
  // from navigating to the file URI or inserting text.
  window.addEventListener('dragover', (e) => {
    if (e.dataTransfer?.types?.includes('Files')) {
      e.preventDefault();
      document.getElementById('message-area')?.classList.add('drag-over');
    }
  }, true);

  window.addEventListener('dragleave', (e) => {
    if (!e.relatedTarget) {
      document.getElementById('message-area')?.classList.remove('drag-over');
    }
  }, true);

  window.addEventListener('drop', (e) => {
    if (e.dataTransfer?.files?.length) {
      if (e.target && e.target.closest && e.target.closest('#forum-new-modal, #dm-pip-panel, #import-dropzone')) {
        return;
      }
      e.preventDefault();
      document.getElementById('message-area')?.classList.remove('drag-over');
      const files = Array.from(e.dataTransfer.files);
      queueFilesIntoActiveHavenComposer(files, e.target);
    }
  }, true);

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

    const hasAuth = !!(document.querySelector('.auth-page') || document.querySelector('.auth-container'));
    let switchBtn = null;
    if (hasAuth) {
      switchBtn = document.createElement('button');
      switchBtn.id = 'haven-switch-server-btn';
      switchBtn.textContent = '⬡ Switch Server';
      document.body.appendChild(switchBtn);
    }

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
        <div style="display:flex;gap:8px;margin-top:16px;">
          <button id="hsp-cancel-btn" class="hsp-cancel" style="margin-top:0;flex:1">Cancel</button>
          <button id="hsp-welcome-btn" class="hsp-cancel" style="margin-top:0;flex:1">Welcome Screen</button>
        </div>
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
        info.onclick = () => invoke('nav_change_primary_server', { serverUrl: entry.url, action: listAction({ user: true }) });
        const removeBtn = document.createElement('button');
        removeBtn.className = 'hsp-remove-btn';
        removeBtn.textContent = '\u00d7';
        removeBtn.onclick = async (e) => {
          e.stopPropagation();
          await invoke('server_history_remove', { url: entry.url, action: listAction({ user: true }) });
          await loadRecentServers();
        };
        item.appendChild(info);
        item.appendChild(removeBtn);
        recentList.appendChild(item);
      });
    }

    const openPicker = async () => {
      overlay.style.display = 'flex';
      urlInput.value = '';
      errorEl.style.display = 'none';
      urlInput.focus();
      await loadRecentServers();
    };

    if (switchBtn) switchBtn.onclick = openPicker;

    window.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'S' || e.key === 's')) {
        e.preventDefault();
        openPicker();
      }
    });

    document.getElementById('hsp-cancel-btn').onclick = () => { overlay.style.display = 'none'; };
    document.getElementById('hsp-welcome-btn').onclick = () => {
      overlay.style.display = 'none';
      invoke('nav_back_to_welcome').catch(() => {});
    };
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
        const check = await invoke('server_check_health', { serverUrl: url }).catch(() => null);
        if (!check || !check.reachable) {
          errorEl.textContent = 'Could not reach server.';
          errorEl.style.display = 'block';
          return;
        }
        await invoke('nav_change_primary_server', { serverUrl: check.url || url, action: listAction({ user: true }) });
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
