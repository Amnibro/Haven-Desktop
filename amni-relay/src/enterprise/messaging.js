'use strict';

/**
 * Group + DM hardening policies for Amni Relay.
 * Scaffold only — enforcement is intended at the engine/server boundary.
 */
const DEFAULT_POLICY = {
  requireAuthForDm: true,
  allowUnsolicitedDm: false,
  groupInvite: 'admins',
  auditMembershipChanges: true,
  e2ePreferred: true,
  retentionDays: null,
  externalSharing: 'off',
  fileScan: 'on',
};

function normalizePolicy(input) {
  const next = { ...DEFAULT_POLICY, ...(input || {}) };
  if (!['admins', 'members', 'off'].includes(next.groupInvite)) {
    next.groupInvite = DEFAULT_POLICY.groupInvite;
  }
  if (!['off', 'members', 'anyone'].includes(next.externalSharing)) {
    next.externalSharing = DEFAULT_POLICY.externalSharing;
  }
  if (!['off', 'on'].includes(next.fileScan)) {
    next.fileScan = DEFAULT_POLICY.fileScan;
  }
  if (next.retentionDays != null) {
    const n = Number(next.retentionDays);
    next.retentionDays = Number.isFinite(n) && n > 0 ? Math.floor(n) : null;
  }
  next.requireAuthForDm = !!next.requireAuthForDm;
  next.allowUnsolicitedDm = !!next.allowUnsolicitedDm;
  next.auditMembershipChanges = !!next.auditMembershipChanges;
  next.e2ePreferred = !!next.e2ePreferred;
  return next;
}

function evaluateMessageAction(policy, action) {
  const p = normalizePolicy(policy);
  if (action?.type === 'dm' && p.requireAuthForDm && !action.authenticated) {
    return { allowed: false, reason: 'auth_required' };
  }
  if (action?.type === 'dm' && !p.allowUnsolicitedDm && action.unsolicited) {
    return { allowed: false, reason: 'unsolicited_dm_blocked' };
  }
  if (action?.type === 'group-invite' && p.groupInvite === 'off') {
    return { allowed: false, reason: 'invites_disabled' };
  }
  if (action?.type === 'group-invite' && p.groupInvite === 'admins' && action.role !== 'admin') {
    return { allowed: false, reason: 'admin_invite_only' };
  }
  if (action?.type === 'external-share' && p.externalSharing === 'off') {
    return { allowed: false, reason: 'external_sharing_off' };
  }
  return { allowed: true, reason: 'ok' };
}

module.exports = {
  DEFAULT_POLICY,
  normalizePolicy,
  evaluateMessageAction,
};
