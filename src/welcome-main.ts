import { installWelcomeBridge } from "./welcome-bridge";

async function boot() {
  await installWelcomeBridge();
  await import("./welcome.js");
}

boot().catch((err) => {
  console.error("[Haven] welcome boot failed", err);
  document.body.insertAdjacentHTML(
    "beforeend",
    `<pre style="color:#f88;padding:16px">Failed to start Haven Desktop UI:\n${String(err)}</pre>`,
  );
});
