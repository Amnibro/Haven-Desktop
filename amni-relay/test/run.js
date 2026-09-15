'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const REPO = path.resolve(ROOT, '..');

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    passed += 1;
    console.log(`  ok  ${name}`);
  } catch (err) {
    failed += 1;
    console.error(`  FAIL ${name}`);
    console.error(`       ${err.message}`);
  }
}

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), 'utf8');
}

function walkFiles(dir, acc) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'node_modules' || entry.name === 'dist') continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkFiles(full, acc);
    else acc.push(full);
  }
  return acc;
}

console.log('identity');
const { identity, getWindowTitle, getAboutText } = require('../src/identity');
test('product is Amni Relay', () => {
  assert.strictEqual(identity.productName, 'Amni Relay');
  assert.strictEqual(identity.suiteName, 'Amni Workspace');
  assert.strictEqual(identity.appId, 'com.amni.relay');
});
test('engine is unpaid Haven', () => {
  assert.strictEqual(identity.engine.name, 'Haven');
  assert.strictEqual(identity.engine.unpaid, true);
  assert.strictEqual(identity.engine.entitled, false);
});
test('no billing flags', () => {
  assert.strictEqual(identity.billing.stripe, false);
  assert.strictEqual(identity.billing.entitlements, false);
  assert.strictEqual(identity.billing.licensingTelemetry, false);
  assert.strictEqual(identity.billing.havenUnpaid, true);
});
test('window title and about copy', () => {
  assert.strictEqual(getWindowTitle('Welcome'), 'Amni Relay — Welcome');
  const about = getAboutText('0.1.0');
  assert.match(about, /Amni Relay v0\.1\.0/);
  assert.match(about, /Amni Workspace/);
  assert.match(about, /Haven engine/);
  assert.match(about, /Not a Microsoft 365 clone/);
});

console.log('vendor-haven');
const vendor = require('../src/vendor-haven');
test('resolves sibling Haven Desktop', () => {
  assert.strictEqual(vendor.havenRoot(), REPO);
  assert.ok(vendor.havenExists(), 'expected ../src Haven modules');
  const mods = vendor.listCoreModules();
  assert.ok(fs.existsSync(mods.serverManager));
  assert.ok(fs.existsSync(mods.audioCapture));
  assert.ok(fs.existsSync(mods.appPreload));
});
test('requireHaven loads ServerManager', () => {
  const { ServerManager } = vendor.requireHaven('main/server-manager');
  assert.strictEqual(typeof ServerManager, 'function');
});

console.log('theme');
const { tokens, cssVariables, cssVariableBlock } = require('../src/theme/tokens');
test('dark-engineer tokens', () => {
  assert.strictEqual(tokens.bg, '#0b0d10');
  assert.strictEqual(tokens.accent, '#3ecfaf');
  assert.strictEqual(tokens.brass, '#e8b86d');
  const vars = cssVariables();
  assert.strictEqual(vars['--amni-accent'], '#3ecfaf');
  assert.match(cssVariableBlock(), /--amni-bg: #0b0d10/);
});
test('chrome and overlay use the same tokens', () => {
  const css = read('src/renderer/welcome.css') + read('src/theme/overlay.css') + read('src/renderer/splash.html');
  assert.match(css, /#0b0d10/);
  assert.match(css, /#3ecfaf/);
  assert.match(css, /#e8b86d/);
  assert.match(read('src/theme/overlay.css'), /data-amni-relay/);
});

console.log('sso');
const { createMemoryStore } = require('../src/lib/store');
const { createOidcClient, normalizeOidcConfig } = require('../src/sso/oidc');
const { createSamlClient } = require('../src/sso/saml');
const { createSsoService } = require('../src/sso');
const { isSessionValid } = require('../src/sso/session');

test('OIDC requires issuer and client', () => {
  const { missing } = normalizeOidcConfig({});
  assert.ok(missing.includes('issuer'));
  const client = createOidcClient({
    issuer: 'https://idp.example/realms/amni',
    clientId: 'amni-relay',
  });
  const start = client.startLogin('abc123');
  assert.match(start.authorizationUrl, /response_type=code/);
  assert.match(start.authorizationUrl, /client_id=amni-relay/);
  const fail = client.handleCallback({ code: 'x', state: 'nope', expectedState: 'abc123' });
  assert.strictEqual(fail.ok, false);
  const ok = client.handleCallback({ code: 'authcode99', state: 'abc123', expectedState: 'abc123' });
  assert.strictEqual(ok.ok, true);
  assert.strictEqual(ok.protocol, 'oidc');
});

test('SAML stub builds AuthnRequest', () => {
  const client = createSamlClient({
    entryPoint: 'https://idp.example/sso',
    issuer: 'https://relay.amni.example/saml',
  });
  const start = client.startLogin('req-1');
  assert.match(start.authnRequest, /AuthnRequest/);
  assert.match(start.authnRequest, /relay\.amni\.example/);
  const ok = client.consumeResponse({ requestId: 'req-1', expectedRequestId: 'req-1', email: 'a@b.c' });
  assert.strictEqual(ok.ok, true);
  assert.strictEqual(ok.email, 'a@b.c');
});

test('SSO service creates a stub session', () => {
  const sso = createSsoService(createMemoryStore({}));
  sso.start('oidc', { oidc: { issuer: 'https://idp.example', clientId: 'relay' } });
  const result = sso.completeStub({ email: 'ada@amni.example' });
  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.session.email, 'ada@amni.example');
  assert.ok(isSessionValid(result.session));
  assert.ok(sso.status().signedIn);
  sso.logout();
  assert.strictEqual(sso.status().signedIn, false);
});

console.log('enterprise');
const { createEnterprise, evaluateMessageAction, DEFAULT_POLICY } = require('../src/enterprise');
test('default DM/group policy is hardened', () => {
  assert.strictEqual(DEFAULT_POLICY.requireAuthForDm, true);
  assert.strictEqual(DEFAULT_POLICY.allowUnsolicitedDm, false);
  assert.strictEqual(DEFAULT_POLICY.groupInvite, 'admins');
  assert.strictEqual(DEFAULT_POLICY.externalSharing, 'off');
  const blocked = evaluateMessageAction(DEFAULT_POLICY, { type: 'dm', authenticated: false });
  assert.strictEqual(blocked.allowed, false);
  const invite = evaluateMessageAction(DEFAULT_POLICY, { type: 'group-invite', role: 'member' });
  assert.strictEqual(invite.allowed, false);
  const admin = evaluateMessageAction(DEFAULT_POLICY, { type: 'group-invite', role: 'admin' });
  assert.strictEqual(admin.allowed, true);
});

test('connectors are stubs without billing', () => {
  const ent = createEnterprise(createMemoryStore({}));
  const list = ent.connectors.list();
  assert.deepStrictEqual(list.map((c) => c.id).sort(), ['google-workspace', 'microsoft-office']);
  const ms = ent.connectors.get('microsoft-office');
  const opened = ms.openDocument('https://contoso.sharepoint.com/docs/spec.docx');
  assert.strictEqual(opened.ok, true);
  assert.strictEqual(opened.mode, 'stub');
  const bad = ms.openDocument('file:///tmp/secret');
  assert.strictEqual(bad.ok, false);
  const st = ent.status();
  assert.strictEqual(st.billing.stripe, false);
  assert.strictEqual(st.billing.entitlements, false);
  assert.strictEqual(st.billing.licensingTelemetry, false);
});

console.log('product surface');
test('chrome says Amni Relay, not a Haven flag', () => {
  const welcome = read('src/renderer/welcome.html');
  const splash = read('src/renderer/splash.html');
  const about = read('src/renderer/about.html');
  const main = read('src/main.js');
  assert.match(welcome, /Amni Relay/);
  assert.match(welcome, /Amni Workspace/);
  assert.match(welcome, /Sign in with SSO/);
  assert.match(splash, /Amni Relay/);
  assert.match(about, /not a Microsoft 365 clone/i);
  assert.match(main, /app\.setName\(identity\.productName\)/);
  assert.doesNotMatch(main, /AMNI_RELAY_BRANDING/);
  assert.doesNotMatch(read('src/identity.js'), /AMNI_RELAY_BRANDING/);
});

test('PRODUCT.md explains suite vs product vs engine', () => {
  const doc = read('PRODUCT.md');
  assert.match(doc, /Amni Workspace/);
  assert.match(doc, /Amni Relay/);
  assert.match(doc, /Haven/);
  assert.match(doc, /unpaid/i);
  assert.match(doc, /not a theme toggle/i);
  assert.match(doc, /Microsoft 365/);
  assert.match(doc, /Stripe/);
  assert.match(doc, /OIDC/);
  assert.match(doc, /SAML/);
});

test('packaging identity is Amni Relay', () => {
  const pkg = JSON.parse(read('package.json'));
  const yml = read('electron-builder.yml');
  assert.strictEqual(pkg.name, 'amni-relay');
  assert.match(yml, /appId: com\.amni\.relay/);
  assert.match(yml, /productName: Amni Relay/);
});

console.log('no billing');
test('Relay tree has no Stripe / entitlements / license telemetry', () => {
  const files = walkFiles(ROOT, []);
  const banned = /stripe|entitlement|license[-_ ]?key|licensing telemetry|paddle\.com|lemonsqueezy/i;
  for (const file of files) {
    if (file.endsWith('.svg') || file.endsWith('.png')) continue;
    const text = fs.readFileSync(file, 'utf8');
    if (!banned.test(text)) continue;
    const allowed = /no Stripe|not add Stripe|stripe: false|Stripe, entitlements|without Stripe/i;
    if (!allowed.test(text)) {
      throw new Error(`billing-adjacent term in ${path.relative(ROOT, file)}`);
    }
  }
});

test('Haven package and main stay unpaid', () => {
  const havenPkg = JSON.parse(fs.readFileSync(path.join(REPO, 'package.json'), 'utf8'));
  const deps = JSON.stringify({
    ...havenPkg.dependencies,
    ...havenPkg.devDependencies,
    ...havenPkg.optionalDependencies,
  });
  assert.doesNotMatch(deps, /stripe/i);
  assert.strictEqual(havenPkg.name, 'haven-desktop');
  const havenMain = fs.readFileSync(path.join(REPO, 'src', 'main', 'main.js'), 'utf8');
  assert.doesNotMatch(havenMain, /AMNI_RELAY_BRANDING/);
  assert.doesNotMatch(havenMain, /stripe/i);
});

console.log('');
console.log(`${passed} passed, ${failed} failed`);
if (failed) process.exit(1);
