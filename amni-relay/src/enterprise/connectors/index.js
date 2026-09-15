'use strict';

const { createMicrosoftOfficeConnector } = require('./microsoft-office');
const { createGoogleWorkspaceConnector } = require('./google-workspace');

function createConnectors(store) {
  function getConfig() {
    return store.get('connectors') || { 'microsoft-office': {}, 'google-workspace': {} };
  }

  function setConfig(id, patch) {
    const all = getConfig();
    all[id] = { ...(all[id] || {}), ...(patch || {}) };
    store.set('connectors', all);
    return all[id];
  }

  function list() {
    const cfg = getConfig();
    return [
      createMicrosoftOfficeConnector(cfg['microsoft-office']),
      createGoogleWorkspaceConnector(cfg['google-workspace']),
    ].map((c) => ({
      id: c.id,
      label: c.label,
      status: c.status,
      capabilities: c.capabilities,
      configured: c.configured,
    }));
  }

  function get(id) {
    const cfg = getConfig();
    if (id === 'microsoft-office') return createMicrosoftOfficeConnector(cfg[id]);
    if (id === 'google-workspace') return createGoogleWorkspaceConnector(cfg[id]);
    throw new Error(`Unknown connector: ${id}`);
  }

  return { getConfig, setConfig, list, get };
}

module.exports = {
  createConnectors,
};
