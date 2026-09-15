'use strict';

const fs = require('fs');
const path = require('path');

try {
  const vendor = require('./vendor-haven');
  if (vendor.havenExists()) {
    require(vendor.havenPath('main/app-preload.js'));
  }
} catch (err) {
  console.warn('[Amni Relay] Haven app-preload not composed:', err.message);
}

function applyAmniOverlay() {
  try {
    document.documentElement.setAttribute('data-amni-relay', '1');
    document.documentElement.setAttribute('data-amni-workspace', '1');
  } catch {}

  const cssPath = path.join(__dirname, 'theme', 'overlay.css');
  let css = '';
  try { css = fs.readFileSync(cssPath, 'utf8'); } catch { return; }

  const style = document.createElement('style');
  style.id = 'amni-relay-overlay';
  style.textContent = css;
  (document.head || document.documentElement).appendChild(style);

  if (!document.querySelector('.amni-relay-wordmark')) {
    const mark = document.createElement('div');
    mark.className = 'amni-relay-wordmark';
    mark.textContent = 'Amni Relay';
    (document.body || document.documentElement).appendChild(mark);
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', applyAmniOverlay, { once: true });
} else {
  applyAmniOverlay();
}
