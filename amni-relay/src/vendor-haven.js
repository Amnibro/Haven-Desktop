'use strict';

const fs = require('fs');
const path = require('path');

/**
 * Resolve Haven Desktop sources from the parent repo.
 *
 * Amni Relay vendors Haven Desktop core in-place (sibling `src/`) rather
 * than forking or theming Haven itself. A future standalone repo can
 * replace this with a git submodule or published package.
 */
function havenRoot() {
  if (process.env.AMNI_HAVEN_ROOT) {
    return path.resolve(process.env.AMNI_HAVEN_ROOT);
  }
  return path.resolve(__dirname, '..', '..');
}

function havenSrc() {
  return path.join(havenRoot(), 'src');
}

function havenPath(rel) {
  return path.join(havenSrc(), rel);
}

function havenExists() {
  return fs.existsSync(havenPath('main/main.js'))
    && fs.existsSync(havenPath('main/server-manager.js'))
    && fs.existsSync(havenPath('main/audio-capture.js'))
    && fs.existsSync(havenPath('main/app-preload.js'));
}

function listCoreModules() {
  return {
    main: havenPath('main/main.js'),
    serverManager: havenPath('main/server-manager.js'),
    audioCapture: havenPath('main/audio-capture.js'),
    appPreload: havenPath('main/app-preload.js'),
    welcomePreload: havenPath('main/preload.js'),
  };
}

function resolveHavenFile(rel) {
  const candidates = [havenPath(rel)];
  if (!rel.endsWith('.js')) candidates.push(havenPath(`${rel}.js`));
  return candidates.find((p) => fs.existsSync(p)) || null;
}

function requireHaven(rel) {
  const resolved = resolveHavenFile(rel);
  if (!resolved) {
    throw new Error(`Haven module not found: ${rel} (looked in ${havenPath(rel)})`);
  }
  return require(resolved);
}

module.exports = {
  havenRoot,
  havenSrc,
  havenPath,
  havenExists,
  listCoreModules,
  resolveHavenFile,
  requireHaven,
};
