'use strict';

const crypto = require('crypto');

function newState() {
  return crypto.randomBytes(16).toString('hex');
}

function createSession({ protocol, subject, email, issuer }) {
  if (!protocol || !subject) {
    throw new Error('session requires protocol and subject');
  }
  return {
    id: newState(),
    protocol,
    subject,
    email: email || null,
    issuer: issuer || null,
    createdAt: Date.now(),
    stub: true,
  };
}

function isSessionValid(session, maxAgeMs = 12 * 60 * 60 * 1000) {
  if (!session || !session.id || !session.subject || !session.protocol) return false;
  if (typeof session.createdAt !== 'number') return false;
  return Date.now() - session.createdAt < maxAgeMs;
}

module.exports = {
  newState,
  createSession,
  isSessionValid,
};
