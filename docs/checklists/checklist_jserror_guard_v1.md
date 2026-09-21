# Checklist — Haven Desktop “Java”/main process error (v1)

- [x] Confirm “java error” = Electron main-process JS dialog
- [x] Syntax-check main files; launch installed + source
- [x] Found corrupt 1.4.28 partial updater download (sha512 mismatch)
- [x] Backup main.js + audio-capture.js
- [x] uncaughtException/unhandledRejection → main-crash.log + dialog
- [x] Purge tiny pending updater exes; soft autoUpdate errors
- [x] startCapture no-throw when native addon missing
- [x] Patch installed app.asar
- [x] Update CHANGELOG
- [ ] User relaunches Haven and confirms Welcome / Login loads
