'use strict';

const CONNECTOR_ID = 'microsoft-office';

function createMicrosoftOfficeConnector(config) {
  const tenantId = String(config?.tenantId || '').trim();
  const clientId = String(config?.clientId || '').trim();
  const configured = !!(tenantId && clientId);

  return {
    id: CONNECTOR_ID,
    label: 'Microsoft Office / Microsoft 365',
    status: configured ? 'configured-stub' : 'scaffold',
    capabilities: ['migrate-mail', 'open-docs', 'calendar-read'],
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
        note: 'Opens Office/Graph documents where the user already has access. No tenant write-back in this scaffold.',
      };
    },
    startMigration(opts) {
      return {
        ok: true,
        mode: 'stub',
        action: 'migrate-mail',
        tenantId: tenantId || null,
        items: Number(opts?.itemLimit) > 0 ? Number(opts.itemLimit) : 0,
        note: 'Migration job is a stub. Wire Microsoft Graph export in a follow-up.',
      };
    },
  };
}

module.exports = {
  CONNECTOR_ID,
  createMicrosoftOfficeConnector,
};
