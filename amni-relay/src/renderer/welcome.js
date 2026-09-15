(function () {
  'use strict';

  const $ = (sel) => document.querySelector(sel);
  const pages = document.querySelectorAll('.page');
  const api = window.amni;

  function showPage(id) {
    pages.forEach((p) => p.classList.remove('active'));
    $(id).classList.add('active');
  }

  $('#btn-min').onclick = () => api.window.minimize();
  $('#btn-close').onclick = () => api.window.close();

  async function refreshSession() {
    const st = await api.sso.status();
    const pill = $('#session-pill');
    if (st.signedIn && st.session) {
      pill.textContent = `Signed in via ${st.session.protocol.toUpperCase()} · ${st.session.email || st.session.subject}`;
    } else {
      pill.textContent = '';
    }
  }

  $('#card-sso').onclick = () => showPage('#page-sso');
  $('#card-join').onclick = () => {
    showPage('#page-join');
    $('#server-url').focus();
  };
  $('#card-host').onclick = () => {
    showPage('#page-host');
    detectServer();
  };

  $('#sso-back').onclick = () => { showPage('#page-choose'); refreshSession(); };
  $('#join-back').onclick = () => showPage('#page-choose');
  $('#host-back').onclick = () => showPage('#page-choose');

  $('#sso-protocol').onchange = () => {
    const oidc = $('#sso-protocol').value === 'oidc';
    $('#oidc-fields').style.display = oidc ? 'contents' : 'none';
    $('#saml-fields').style.display = oidc ? 'none' : 'contents';
  };
  $('#oidc-fields').style.display = 'contents';

  $('#btn-sso').onclick = async () => {
    const protocol = $('#sso-protocol').value;
    const error = $('#sso-error');
    const ok = $('#sso-ok');
    error.style.display = 'none';
    ok.style.display = 'none';
    try {
      const overrides = protocol === 'oidc'
        ? { oidc: { issuer: $('#oidc-issuer').value, clientId: $('#oidc-client').value, redirectUri: 'amni-relay://oidc/callback' } }
        : { saml: { entryPoint: $('#saml-entry').value, issuer: $('#saml-issuer').value } };
      await api.sso.start(protocol, overrides);
      const result = await api.sso.completeStub({ email: $('#sso-email').value || undefined });
      if (!result.ok) throw new Error(result.error || 'SSO failed');
      ok.textContent = `Stub session created for ${result.session.email || result.session.subject}`;
      ok.style.display = 'block';
      await refreshSession();
    } catch (err) {
      error.textContent = err.message || String(err);
      error.style.display = 'block';
    }
  };

  const urlInput = $('#server-url');
  const connectBtn = $('#btn-connect');
  urlInput.addEventListener('input', () => {
    connectBtn.disabled = !urlInput.value.trim();
    $('#join-error').style.display = 'none';
  });
  urlInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !connectBtn.disabled) connectBtn.click();
  });

  connectBtn.onclick = async () => {
    let url = urlInput.value.trim();
    if (url && !/^https?:\/\//i.test(url)) url = 'https://' + url;
    try {
      const parsed = new URL(url);
      api.nav.openApp(parsed.origin);
    } catch {
      $('#join-error').textContent = 'Enter a valid URL.';
      $('#join-error').style.display = 'block';
    }
  };

  async function detectServer() {
    $('#host-status').textContent = 'Looking for a local Haven server…';
    $('#btn-start-server').style.display = 'none';
    $('#host-error').style.display = 'none';
    try {
      const result = await api.server.detect();
      if (result.found) {
        $('#host-status').textContent = `Found engine at ${result.path}`;
        $('#btn-start-server').dataset.path = result.path;
        $('#btn-start-server').style.display = 'inline-block';
      } else {
        $('#host-status').textContent = 'No local Haven server detected. Browse for a server directory.';
      }
    } catch (err) {
      $('#host-error').textContent = err.message || 'Detection failed.';
      $('#host-error').style.display = 'block';
    }
  }

  $('#btn-start-server').onclick = async () => {
    const dir = $('#btn-start-server').dataset.path;
    const res = await api.server.start(dir);
    if (res.success) {
      api.nav.openApp(res.url || `http://localhost:${res.port}`);
    } else {
      $('#host-error').textContent = res.error || 'Failed to start server.';
      $('#host-error').style.display = 'block';
    }
  };

  $('#btn-browse-server').onclick = async () => {
    const dir = await api.server.browse();
    if (!dir) return;
    const res = await api.server.start(dir);
    if (res.success) {
      api.nav.openApp(res.url || `http://localhost:${res.port}`);
    } else {
      $('#host-error').textContent = res.error || 'Failed to start server.';
      $('#host-error').style.display = 'block';
    }
  };

  refreshSession();
})();
