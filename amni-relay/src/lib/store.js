'use strict';

const fs = require('fs');
const path = require('path');

/**
 * Isolated JSON store for Amni Relay. No telemetry, no cloud sync.
 */
function createFileStore(filePath, defaults) {
  const data = { ...defaults };

  function load() {
    try {
      if (fs.existsSync(filePath)) {
        const parsed = JSON.parse(fs.readFileSync(filePath, 'utf8'));
        if (parsed && typeof parsed === 'object') Object.assign(data, parsed);
      }
    } catch {
      // Corrupt store: keep defaults.
    }
  }

  function save() {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
  }

  load();

  return {
    get(key) {
      if (!key) return { ...data };
      return data[key];
    },
    set(key, value) {
      data[key] = value;
      save();
      return value;
    },
    path: filePath,
  };
}

function createMemoryStore(defaults) {
  const data = { ...defaults };
  return {
    get(key) {
      if (!key) return { ...data };
      return data[key];
    },
    set(key, value) {
      data[key] = value;
      return value;
    },
    path: ':memory:',
  };
}

module.exports = {
  createFileStore,
  createMemoryStore,
};
