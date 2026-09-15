'use strict';

const { createOidcClient } = require('./oidc');
const { createSamlClient } = require('./saml');
const { newState, createSession, isSessionValid } = require('./session');

function createSsoService(store) {
  let pending = null;

  function getConfig() {
    return store.get('sso') || { protocol: null, oidc: {}, saml: {} };
  }

  function setConfig(next) {
    const current = getConfig();
    const merged = {
      protocol: next.protocol || current.protocol || null,
      oidc: { ...current.oidc, ...(next.oidc || {}) },
      saml: { ...current.saml, ...(next.saml || {}) },
    };
    store.set('sso', merged);
    return merged;
  }

  function status() {
    const session = store.get('ssoSession');
    return {
      configured: !!(getConfig().protocol),
      protocol: getConfig().protocol,
      signedIn: isSessionValid(session),
      session: isSessionValid(session) ? publicSession(session) : null,
      pending: pending ? { protocol: pending.protocol, state: pending.state } : null,
    };
  }

  function start(protocol, overrides) {
    const cfg = setConfig({ protocol, ...(overrides || {}) });
    const state = newState();
    if (protocol === 'oidc') {
      const client = createOidcClient({ ...cfg.oidc, ...(overrides?.oidc || {}) });
      const start = client.startLogin(state);
      pending = { protocol: 'oidc', state, client };
      return start;
    }
    if (protocol === 'saml') {
      const client = createSamlClient({ ...cfg.saml, ...(overrides?.saml || {}) });
      const start = client.startLogin(state);
      pending = { protocol: 'saml', state, client };
      return start;
    }
    throw new Error('Unsupported SSO protocol (use oidc or saml)');
  }

  function completeStub(profile) {
    if (!pending) {
      throw new Error('No SSO login in progress');
    }
    let result;
    if (pending.protocol === 'oidc') {
      result = pending.client.handleCallback({
        code: profile?.code || `stub-${pending.state.slice(0, 8)}`,
        state: pending.state,
        expectedState: pending.state,
        email: profile?.email,
      });
    } else {
      result = pending.client.consumeResponse({
        requestId: pending.state,
        expectedRequestId: pending.state,
        nameId: profile?.nameId,
        email: profile?.email,
      });
    }
    pending = null;
    if (!result.ok) {
      return result;
    }
    const session = createSession(result);
    store.set('ssoSession', session);
    return { ok: true, session: publicSession(session) };
  }

  function logout() {
    store.set('ssoSession', null);
    pending = null;
    return { ok: true };
  }

  return {
    getConfig,
    setConfig,
    status,
    start,
    completeStub,
    logout,
  };
}

function publicSession(session) {
  return {
    id: session.id,
    protocol: session.protocol,
    subject: session.subject,
    email: session.email,
    issuer: session.issuer,
    createdAt: session.createdAt,
    stub: !!session.stub,
  };
}

module.exports = {
  createSsoService,
};
