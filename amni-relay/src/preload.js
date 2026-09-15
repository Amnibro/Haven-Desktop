'use strict';

const { ipcRenderer } = require('electron');

window.amni = {
  platform: process.platform,
  getVersion: () => ipcRenderer.invoke('app:version'),
  identity: () => ipcRenderer.invoke('identity:get'),
  window: {
    minimize: () => ipcRenderer.send('window:minimize'),
    close: () => ipcRenderer.send('window:close'),
  },
  nav: {
    openApp: (serverUrl) => ipcRenderer.send('nav:open-app', serverUrl),
  },
  server: {
    detect: () => ipcRenderer.invoke('server:detect'),
    start: (dir) => ipcRenderer.invoke('server:start', dir),
    browse: () => ipcRenderer.invoke('server:browse'),
  },
  sso: {
    status: () => ipcRenderer.invoke('sso:status'),
    start: (protocol, overrides) => ipcRenderer.invoke('sso:start', protocol, overrides),
    completeStub: (profile) => ipcRenderer.invoke('sso:complete-stub', profile),
    logout: () => ipcRenderer.invoke('sso:logout'),
  },
  enterprise: {
    status: () => ipcRenderer.invoke('enterprise:status'),
  },
  openExternal: (url) => ipcRenderer.send('open-external', url),
};
