'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const { decideHostedServerRestart } = require('../src/main/server-manager');

test('stopServer exit does not restart', () => {
  const d = decideHostedServerRestart({ intentional: true, now: 10_000, lastRestart: 0 });
  assert.equal(d.restart, false);
  assert.equal(d.reason, 'intentional');
});

test('unrequested exit 0 restarts', () => {
  const d = decideHostedServerRestart({ intentional: false, now: 10_000, lastRestart: 0 });
  assert.equal(d.restart, true);
});

test('unrequested crash restarts', () => {
  const d = decideHostedServerRestart({ intentional: false, now: 10_000, lastRestart: 0 });
  assert.equal(d.restart, true);
});

test('5s loop guard holds', () => {
  const d = decideHostedServerRestart({
    intentional: false,
    now: 12_000,
    lastRestart: 10_000,
    cooldownMs: 5000,
  });
  assert.equal(d.restart, false);
  assert.equal(d.reason, 'cooldown');
});

test('after cooldown a later unrequested exit restarts', () => {
  const d = decideHostedServerRestart({
    intentional: false,
    now: 16_000,
    lastRestart: 10_000,
    cooldownMs: 5000,
  });
  assert.equal(d.restart, true);
});
