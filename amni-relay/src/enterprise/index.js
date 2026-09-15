'use strict';

const { DEFAULT_POLICY, normalizePolicy, evaluateMessageAction } = require('./messaging');
const { createConnectors } = require('./connectors');

function createEnterprise(store) {
  const connectors = createConnectors(store);

  function getMessagingPolicy() {
    return normalizePolicy(store.get('messagingPolicy'));
  }

  function setMessagingPolicy(patch) {
    const next = normalizePolicy({ ...getMessagingPolicy(), ...(patch || {}) });
    store.set('messagingPolicy', next);
    return next;
  }

  function status() {
    return {
      messaging: getMessagingPolicy(),
      connectors: connectors.list(),
      billing: {
        stripe: false,
        entitlements: false,
        licensingTelemetry: false,
      },
    };
  }

  return {
    getMessagingPolicy,
    setMessagingPolicy,
    evaluateMessageAction: (action) => evaluateMessageAction(getMessagingPolicy(), action),
    connectors,
    status,
  };
}

module.exports = {
  DEFAULT_POLICY,
  normalizePolicy,
  evaluateMessageAction,
  createEnterprise,
};
