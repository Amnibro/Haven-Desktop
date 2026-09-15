'use strict';

const REQUIRED = ['issuer', 'clientId', 'redirectUri'];

function normalizeOidcConfig(input) {
  const cfg = {
    issuer: String(input?.issuer || '').trim(),
    clientId: String(input?.clientId || '').trim(),
    clientSecret: input?.clientSecret ? String(input.clientSecret) : '',
    redirectUri: String(input?.redirectUri || 'amni-relay://oidc/callback').trim(),
    scopes: Array.isArray(input?.scopes) && input.scopes.length
      ? input.scopes.map(String)
      : ['openid', 'profile', 'email'],
  };
  const missing = REQUIRED.filter((k) => !cfg[k]);
  return { cfg, missing };
}

function buildAuthorizationUrl(cfg, state) {
  const issuer = cfg.issuer.replace(/\/+$/, '');
  const url = new URL(`${issuer}/authorize`);
  url.searchParams.set('response_type', 'code');
  url.searchParams.set('client_id', cfg.clientId);
  url.searchParams.set('redirect_uri', cfg.redirectUri);
  url.searchParams.set('scope', cfg.scopes.join(' '));
  url.searchParams.set('state', state);
  return url.toString();
}

function createOidcClient(input) {
  const { cfg, missing } = normalizeOidcConfig(input);
  if (missing.length) {
    throw new Error(`OIDC config missing: ${missing.join(', ')}`);
  }

  return {
    protocol: 'oidc',
    config: { ...cfg, clientSecret: cfg.clientSecret ? '[redacted]' : '' },
    startLogin(state) {
      return {
        protocol: 'oidc',
        mode: 'stub',
        authorizationUrl: buildAuthorizationUrl(cfg, state),
        state,
      };
    },
    handleCallback(query) {
      if (!query || query.state !== query.expectedState) {
        return { ok: false, error: 'state_mismatch' };
      }
      if (!query.code) {
        return { ok: false, error: 'missing_code' };
      }
      return {
        ok: true,
        mode: 'stub',
        protocol: 'oidc',
        subject: `oidc:${cfg.clientId}:${query.code.slice(0, 8)}`,
        email: query.email || null,
        issuer: cfg.issuer,
      };
    },
  };
}

module.exports = {
  normalizeOidcConfig,
  buildAuthorizationUrl,
  createOidcClient,
};
