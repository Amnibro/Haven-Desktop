'use strict';

/**
 * Amni Relay — Electron entry.
 * Distinct product packaging. Vendors Haven Desktop core; does not theme-flag Haven.
 */

const {
  app, BrowserWindow, BrowserView, ipcMain, Tray, Menu, dialog, shell, session, nativeImage, screen,
} = require('electron');
const path = require('path');
const { identity, getWindowTitle, getAboutText } = require('./identity');
const vendor = require('./vendor-haven');
const { createFileStore } = require('./lib/store');
const { createSsoService } = require('./sso');
const { createEnterprise } = require('./enterprise');
const { tokens } = require('./theme/tokens');

app.setName(identity.productName);
app.setPath('userData', path.join(app.getPath('appData'), identity.dataDirName));

const store = createFileStore(path.join(app.getPath('userData'), 'relay.json'), {
  windowBounds: { width: 1200, height: 800 },
  lastServerUrl: null,
});
const sso = createSsoService(store);
const enterprise = createEnterprise(store);

const ICON_PATH = path.join(__dirname, '..', 'assets', 'icon.svg');
const IS_DEV = process.argv.includes('--dev');

let ServerManager = null;
let AudioCaptureManager = null;
if (vendor.havenExists()) {
  try { ({ ServerManager } = vendor.requireHaven('main/server-manager')); } catch (e) {
    console.warn('[Amni Relay] ServerManager unavailable:', e.message);
  }
  try { ({ AudioCaptureManager } = vendor.requireHaven('main/audio-capture')); } catch (e) {
    console.warn('[Amni Relay] AudioCaptureManager unavailable:', e.message);
  }
}

let welcomeWindow = null;
let mainWindow = null;
let tray = null;
let serverManager = null;
let audioCapture = null;
let activeView = null;

function iconImage() {
  try { return nativeImage.createFromPath(ICON_PATH); } catch { return undefined; }
}

function createWelcomeWindow() {
  if (welcomeWindow) {
    welcomeWindow.show();
    welcomeWindow.focus();
    return;
  }
  welcomeWindow = new BrowserWindow({
    width: 760,
    height: 620,
    backgroundColor: tokens.bg,
    title: getWindowTitle('Welcome'),
    icon: iconImage(),
    frame: false,
    resizable: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  welcomeWindow.loadFile(path.join(__dirname, 'renderer', 'welcome.html'));
  welcomeWindow.on('closed', () => { welcomeWindow = null; });
  if (IS_DEV) welcomeWindow.webContents.openDevTools({ mode: 'detach' });
}

function createAppWindow(serverUrl) {
  if (!mainWindow) {
    const bounds = store.get('windowBounds') || { width: 1200, height: 800 };
    mainWindow = new BrowserWindow({
      ...bounds,
      minWidth: 800,
      minHeight: 600,
      backgroundColor: tokens.bg,
      title: identity.productName,
      icon: iconImage(),
      autoHideMenuBar: false,
      webPreferences: {
        backgroundThrottling: false,
      },
    });
    mainWindow.loadFile(path.join(__dirname, 'renderer', 'splash.html'));
    mainWindow.on('close', () => {
      if (!mainWindow) return;
      if (!mainWindow.isMaximized()) store.set('windowBounds', mainWindow.getBounds());
    });
    mainWindow.on('closed', () => {
      mainWindow = null;
      activeView = null;
    });
  }

  attachServerView(serverUrl);
  store.set('lastServerUrl', serverUrl);
  if (welcomeWindow) welcomeWindow.close();
  mainWindow.show();
}

function attachServerView(serverUrl) {
  if (!mainWindow) return;
  if (activeView) {
    try { mainWindow.removeBrowserView(activeView); } catch {}
    try { activeView.webContents.destroy(); } catch {}
    activeView = null;
  }

  const view = new BrowserView({
    webPreferences: {
      preload: path.join(__dirname, 'overlay-preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false,
    },
  });
  mainWindow.addBrowserView(view);
  const size = mainWindow.getContentSize();
  view.setBounds({ x: 0, y: 0, width: size[0], height: size[1] });
  view.setAutoResize({ width: true, height: true });
  view.webContents.on('page-title-updated', (_e, title) => {
    const cleaned = (title || '').replace(/^Haven\b/i, identity.productName);
    mainWindow.setTitle(cleaned && !/^Loading /i.test(cleaned) ? cleaned : identity.productName);
  });
  view.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:\/\//i.test(url)) shell.openExternal(url);
    return { action: 'deny' };
  });
  view.webContents.loadURL(serverUrl);
  activeView = view;
}

function openAbout() {
  const about = new BrowserWindow({
    width: 480,
    height: 360,
    parent: mainWindow || welcomeWindow || undefined,
    modal: true,
    backgroundColor: tokens.bg,
    title: getWindowTitle('About'),
    icon: iconImage(),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  about.setMenu(null);
  about.loadFile(path.join(__dirname, 'renderer', 'about.html'));
}

function buildMenu() {
  return Menu.buildFromTemplate([
    {
      label: identity.productName,
      submenu: [
        { label: `About ${identity.productName}`, click: () => openAbout() },
        { type: 'separator' },
        { role: 'quit', label: `Quit ${identity.productName}` },
      ],
    },
    {
      label: 'Edit',
      submenu: [
        { role: 'undo' }, { role: 'redo' }, { type: 'separator' },
        { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' },
      ],
    },
    {
      label: 'View',
      submenu: [
        { role: 'reload' },
        { role: 'forceReload' },
        { role: 'toggleDevTools' },
        { type: 'separator' },
        { role: 'togglefullscreen' },
        {
          label: 'Back to welcome',
          click() {
            if (mainWindow) {
              mainWindow.close();
            }
            createWelcomeWindow();
          },
        },
      ],
    },
    {
      label: 'Help',
      submenu: [
        {
          label: 'Product notes',
          click: () => dialog.showMessageBox({
            type: 'info',
            title: identity.productName,
            message: identity.productName,
            detail: getAboutText(app.getVersion()),
          }),
        },
      ],
    },
  ]);
}

function createTray() {
  const img = iconImage();
  if (!img || img.isEmpty()) return;
  const sf = screen.getPrimaryDisplay().scaleFactor || 1;
  const size = process.platform === 'win32' ? Math.round(16 * sf) : 22;
  tray = new Tray(img.resize({ width: size, height: size }));
  tray.setToolTip(identity.productName);
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: `${identity.productName} v${app.getVersion()}`, enabled: false },
    { label: `Show ${identity.productName}`, click: () => (mainWindow || welcomeWindow)?.show() },
    { label: `About ${identity.productName}`, click: () => openAbout() },
    { type: 'separator' },
    { label: `Quit ${identity.productName}`, click: () => { app.isQuitting = true; app.quit(); } },
  ]));
}

function registerIpc() {
  ipcMain.handle('app:version', () => app.getVersion());
  ipcMain.handle('identity:get', () => identity);
  ipcMain.handle('sso:status', () => sso.status());
  ipcMain.handle('sso:start', (_e, protocol, overrides) => sso.start(protocol, overrides));
  ipcMain.handle('sso:complete-stub', (_e, profile) => sso.completeStub(profile));
  ipcMain.handle('sso:logout', () => sso.logout());
  ipcMain.handle('enterprise:status', () => enterprise.status());

  ipcMain.handle('server:detect', () => {
    if (!serverManager) return { found: false };
    return serverManager.detectServer();
  });
  ipcMain.handle('server:start', (_e, dir) => {
    if (!serverManager) return { success: false, error: 'Haven ServerManager not vendored' };
    return serverManager.startServer(dir);
  });
  ipcMain.handle('server:browse', async () => {
    const res = await dialog.showOpenDialog(welcomeWindow || mainWindow, {
      title: 'Select Haven engine directory',
      properties: ['openDirectory'],
    });
    return res.canceled ? null : res.filePaths[0];
  });

  ipcMain.on('nav:open-app', (_e, serverUrl) => {
    if (typeof serverUrl === 'string' && /^https?:\/\//i.test(serverUrl)) {
      createAppWindow(serverUrl);
    }
  });
  ipcMain.on('window:minimize', () => welcomeWindow?.minimize());
  ipcMain.on('window:close', () => welcomeWindow?.close());
  ipcMain.on('open-external', (_e, url) => {
    if (typeof url === 'string' && /^https?:\/\//i.test(url)) shell.openExternal(url);
  });

  if (audioCapture) {
    ipcMain.handle('audio:is-supported', () => {
      try { return audioCapture.isSupported(); } catch { return false; }
    });
  }
}

app.whenReady().then(() => {
  if (ServerManager) serverManager = new ServerManager(store, { showConsole: IS_DEV });
  if (AudioCaptureManager) audioCapture = new AudioCaptureManager();

  session.defaultSession.setPermissionRequestHandler((_wc, permission, callback) => {
    callback(['media', 'display-capture', 'notifications', 'fullscreen', 'clipboard-sanitized-write'].includes(permission));
  });

  registerIpc();
  Menu.setApplicationMenu(buildMenu());
  createWelcomeWindow();
  createTray();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('activate', () => {
  if (!mainWindow && !welcomeWindow) createWelcomeWindow();
});

process.on('uncaughtException', (err) => {
  console.error('[Amni Relay]', err);
});
