const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const read = p => fs.readFileSync(path.join(__dirname, '..', 'src-tauri', 'src', p), 'utf8');
test('notification attention is gated by flashTaskbar, before considering focus', () => {
  const s = read('commands/notify.rs');
  assert.match(s, /let flash = state::get_value\(&app, "flashTaskbar"\).*!= Some\(false\)/);
  assert.match(s, /filter\(\|m\| flash && !m\.is_focused\(\).*\.map\(\|m\| m\.request_user_attention\(Some/);
});
test('turning the setting off cancels attention: None is cancellation, not a request', () => {
  const s = read('commands/desktop.rs').split('pub fn desktop_set_flash_taskbar')[1].split('#[tauri::command]')[0];
  assert.match(s, /set_value\(&app, "flashTaskbar", json!\(enabled\)\)/);
  assert.match(s, /if !enabled[\s\S]*request_user_attention\(None\)/);
  assert.doesNotMatch(s, /request_user_attention\(Some/);
});
test('all positive attention requests in the Tauri source are in the guarded notify command', () => {
  const walk = d => fs.readdirSync(d, { withFileTypes: true }).flatMap(e => e.isDirectory() ? walk(path.join(d,e.name)) : [path.join(d,e.name)]);
  const hits = walk(path.join(__dirname, '..', 'src-tauri', 'src')).filter(p => p.endsWith('.rs') && /request_user_attention\(Some/.test(fs.readFileSync(p, 'utf8')));
  assert.deepEqual(hits.map(p => path.relative(path.join(__dirname, '..', 'src-tauri', 'src'), p)), ['commands/notify.rs']);
});
