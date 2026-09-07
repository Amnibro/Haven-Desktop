use crate::i18n;
use crate::state::{self, AppState};
use serde_json::json;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn i18n_get_state(app: AppHandle) -> Result<i18n::I18nState, String> {
    state::i18n_state(&app)
}

#[tauri::command]
pub fn i18n_set_language(
    app: AppHandle,
    state: State<AppState>,
    preference: String,
) -> Result<i18n::I18nState, String> {
    let mut pref = preference;
    if pref == "system" {
        pref = "auto".into();
    }
    let supported = pref == "auto"
        || i18n::supported_locales()
            .iter()
            .any(|l| l.code == pref);
    if !supported {
        return state::i18n_state(&app);
    }
    state::set_value(&app, "language", json!(pref))?;
    state::set_value(&app, "languagePreferenceSet", json!(true))?;
    let locale = state::refresh_locale(&app)?;
    *state.current_locale.lock() = locale;
    let i18n_state = state::i18n_state(&app)?;
    let _ = app.emit("i18n:changed", &i18n_state);
    Ok(i18n_state)
}

#[tauri::command]
pub fn i18n_refresh_automatic(app: AppHandle) -> Result<i18n::I18nState, String> {
    let pref = state::get_value(&app, "language")?
        .as_str()
        .unwrap_or("auto")
        .to_string();
    if pref == "auto" {
        state::refresh_locale(&app)?;
        let i18n_state = state::i18n_state(&app)?;
        let _ = app.emit("i18n:changed", &i18n_state);
        return Ok(i18n_state);
    }
    state::i18n_state(&app)
}
