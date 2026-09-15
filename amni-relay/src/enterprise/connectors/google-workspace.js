'use strict';

const CONNECTOR_ID = 'google-workspace';

function createGoogleWorkspaceConnector(config) {
  const customerId = String(config?.customerId || '').trim();
  const clientId = String(config?.clientId || '').trim();
  const configured = !!(customerId && clientId);

  return {
    id: CONNECTOR_ID,
    label: 'Google Workspace',
    status: configured ? 'configured-stub' : 'scaffold',
    capabilities: ['migrate-mail', 'open-docs', 'drive-read'],
    configured,
    openDocument(docUrl) {
      if (!docUrl || !/^https:\/\//i.test(docUrl)) {
        return { ok: false, error: 'invalid_url' };
      }
      return {
        ok: true,
        mode: 'stub',
        action: 'open-docs',
        url: docUrl,
        note: 'Opens Docs/Drive URLs where the user already has access. No Workspace write-back in this scaffold.',
      };
    },
    startMigration(opts) {
      return {
        ok: true,
        mode: 'stub',
        action: 'migrate-mail',
        customerId: customerId || null,
        items: Number(opts?.itemLimit) > 0 ? Number(opts.itemLimit) : 0,
        note: 'Migration job is a stub. Wire Google Workspace export in a follow-up.',
      };
    },
  };
}

module.exports = {
  CONNECTOR_ID,
  createGoogleWorkspaceConnector,
};
