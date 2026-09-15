'use strict';

/**
 * Amni Relay product identity.
 * This is a distinct product in the Amni Workspace suite — not a Haven theme flag.
 */

const identity = {
  suiteName: 'Amni Workspace',
  productName: 'Amni Relay',
  shortName: 'Relay',
  appId: 'com.amni.relay',
  dataDirName: 'amni-relay',
  tagline: 'Enterprise communications for Amni Workspace',
  description:
    'Amni Relay is the Amni Workspace communications client. It vendors Haven Desktop core (channels, group messaging, DMs, files, roles, WebRTC) and adds Amni identity plus enterprise controls.',

  engine: {
    name: 'Haven',
    desktopPackage: 'haven-desktop',
    role: 'unpaid communications engine',
    unpaid: true,
    entitled: false,
  },

  intendedStandaloneRepo: 'Amnibro/Amni-Relay',
  inRepoPath: 'amni-relay/',

  billing: {
    stripe: false,
    entitlements: false,
    licensingTelemetry: false,
    havenUnpaid: true,
    note:
      'Haven upstream stays unpaid and unentitled. This product must not add Stripe, entitlements, or licensing telemetry to Haven.',
  },

  about: {
    copyright: 'Copyright © Amni Workspace',
    poweredBy: 'Powered by the Haven engine (unpaid, no entitlements).',
    notAClone: 'Not a Microsoft 365 clone. Amni Workspace is its own suite; Relay is the Teams-line communications face.',
  },
};

function getWindowTitle(page) {
  if (!page) return identity.productName;
  return `${identity.productName} — ${page}`;
}

function getAboutText(version) {
  const ver = version ? ` v${version}` : '';
  return [
    `${identity.productName}${ver}`,
    identity.suiteName,
    '',
    identity.tagline,
    identity.about.poweredBy,
    identity.about.notAClone,
  ].join('\n');
}

module.exports = {
  identity,
  getWindowTitle,
  getAboutText,
};
