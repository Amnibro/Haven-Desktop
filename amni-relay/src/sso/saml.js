'use strict';

const REQUIRED = ['entryPoint', 'issuer'];

function normalizeSamlConfig(input) {
  const cfg = {
    entryPoint: String(input?.entryPoint || '').trim(),
    issuer: String(input?.issuer || '').trim(),
    callbackUrl: String(input?.callbackUrl || 'amni-relay://saml/acs').trim(),
    cert: String(input?.cert || '').trim(),
  };
  const missing = REQUIRED.filter((k) => !cfg[k]);
  return { cfg, missing };
}

function createAuthnRequest(cfg, requestId) {
  const dest = escapeXml(cfg.entryPoint);
  const issuer = escapeXml(cfg.issuer);
  const acs = escapeXml(cfg.callbackUrl);
  const id = escapeXml(requestId);
  return [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"`,
    `  ID="${id}" Version="2.0" AssertionConsumerServiceURL="${acs}"`,
    `  Destination="${dest}">`,
    `  <saml:Issuer xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion">${issuer}</saml:Issuer>`,
    `</samlp:AuthnRequest>`,
  ].join('\n');
}

function escapeXml(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function createSamlClient(input) {
  const { cfg, missing } = normalizeSamlConfig(input);
  if (missing.length) {
    throw new Error(`SAML config missing: ${missing.join(', ')}`);
  }

  return {
    protocol: 'saml',
    config: { ...cfg, cert: cfg.cert ? '[configured]' : '' },
    startLogin(requestId) {
      return {
        protocol: 'saml',
        mode: 'stub',
        entryPoint: cfg.entryPoint,
        authnRequest: createAuthnRequest(cfg, requestId),
        requestId,
      };
    },
    consumeResponse(payload) {
      if (!payload || payload.requestId !== payload.expectedRequestId) {
        return { ok: false, error: 'request_mismatch' };
      }
      return {
        ok: true,
        mode: 'stub',
        protocol: 'saml',
        subject: payload.nameId || `saml:${cfg.issuer}:stub`,
        email: payload.email || null,
        issuer: cfg.issuer,
      };
    },
  };
}

module.exports = {
  normalizeSamlConfig,
  createAuthnRequest,
  createSamlClient,
};
