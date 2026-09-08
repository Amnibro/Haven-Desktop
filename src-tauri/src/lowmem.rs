//! Ask WebView2 to run lean while nobody is looking at the window.
//!
//! Chromium flags (--in-process-gpu, --single-process, renderer limits) were
//! measured on this app and change nothing: WebView2 ignores them. What it
//! does honor is MemoryUsageTargetLevel, which tells the browser to drop
//! caches and shrink heaps while the app sits in the tray or minimized, and
//! to go back to normal when it is shown again.
pub fn set_low_memory(window: &tauri::WebviewWindow, low: bool) {
    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows_core::Interface;
        let _ = window.with_webview(move |w| unsafe {
            if let Ok(core) = w.controller().CoreWebView2() {
                if let Ok(v19) = core.cast::<ICoreWebView2_19>() {
                    let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
                    let _ = v19.SetMemoryUsageTargetLevel(level);
                }
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (window, low);
}
