use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleInfo {
    pub code: String,
    pub name: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I18nState {
    pub preference: String,
    pub locale: String,
    pub system_locale: String,
    pub is_preference_stored: bool,
    pub direction: String,
    pub supported_locales: Vec<LocaleInfo>,
}

const EN: &[(&str, &str)] = &[
    ("language.label", "Language"),
    ("language.automatic", "Automatic"),
    ("window.minimize", "Minimize"),
    ("window.close", "Close"),
    ("welcome.windowTitle", "Haven - Welcome"),
    ("welcome.heading", "Welcome to"),
    ("welcome.tagline", "Your private, self-hosted communication hub - now on your desktop."),
    ("welcome.host.title", "Host My Server"),
    ("welcome.host.description", "Run your own Haven server and connect to it."),
    ("welcome.join.title", "Join a Server"),
    ("welcome.join.description", "Connect to an existing Haven server."),
    ("welcome.rememberChoice", "Remember my choice"),
    ("welcome.back", "Back"),
    ("welcome.serverSetup", "Server Setup"),
    ("welcome.searchingServer", "Searching for a local Haven server..."),
    ("welcome.serverFound", "Found Haven server at:"),
    ("welcome.startServer", "Start Server"),
    ("welcome.serverNotDetected", "No Haven server detected automatically."),
    ("welcome.browseServer", "Browse for server directory..."),
    ("welcome.freshSetup", "Start Fresh Server Setup"),
    ("welcome.startingServer", "Starting server..."),
    ("welcome.tryAgain", "Try Again"),
    ("welcome.joinServer", "Join a Server"),
    ("welcome.serverAddress", "Server Address"),
    ("welcome.serverAddressHint", "Enter the URL or IP address of the Haven server you'd like to join."),
    ("welcome.connect", "Connect"),
    ("welcome.connecting", "Connecting..."),
    ("welcome.error.detectionFailed", "Server detection failed."),
    ("welcome.error.startFailed", "Failed to start the Haven server."),
    ("welcome.error.unexpected", "An unexpected error occurred."),
    ("welcome.error.invalidUrl", "Please enter a valid server URL."),
    ("welcome.error.serverUnreachable", "Could not reach that server."),
    ("welcome.error.connectionFailed", "Connection failed."),
    ("server.error.fileNotFound", "server.js was not found in that directory."),
    ("server.error.noPort", "No free port available to start the server."),
    ("server.log.crashLoop", "Server is crash-looping; not restarting again."),
    ("server.log.restarting", "Server exited with code {code}; restarting..."),
    ("dialog.selectServerDirectory", "Select Haven server directory"),
    ("dialog.ok", "OK"),
    ("dialog.cancel", "Cancel"),
    ("dialog.defaultValue", "Default: {value}"),
    ("tray.show", "Show Haven"),
    ("tray.quit", "Quit"),
    ("tray.tooltip", "Haven Desktop"),
    ("splash.windowTitle", "Loading Haven…"),
    ("update.available", "Update {version} is available"),
    ("update.now", "Update now"),
    ("update.downloading", "Downloading update…"),
    ("update.downloadingProgress", "Downloading… {percent}%"),
    ("update.downloaded", "Update ready — restart to install"),
    ("update.restartNow", "Restart now"),
    ("update.failed", "Update failed: {error}"),
    ("update.error", "Update error: {error}"),
    ("update.unavailable", "Auto-update is unavailable in this build"),
    ("update.close", "Dismiss"),
    ("audio.unknownApplication", "Unknown"),
];

const PT_BR: &[(&str, &str)] = &[
    ("language.label", "Idioma"),
    ("language.automatic", "Automático"),
    ("window.minimize", "Minimizar"),
    ("window.close", "Fechar"),
    ("welcome.heading", "Bem-vindo ao"),
    ("welcome.tagline", "Seu hub de comunicação privado e auto-hospedado — agora no desktop."),
    ("welcome.host.title", "Hospedar meu servidor"),
    ("welcome.host.description", "Execute seu próprio servidor Haven e conecte-se a ele."),
    ("welcome.join.title", "Entrar em um servidor"),
    ("welcome.join.description", "Conecte-se a um servidor Haven existente."),
    ("welcome.rememberChoice", "Lembrar minha escolha"),
    ("welcome.back", "Voltar"),
    ("welcome.serverSetup", "Configuração do servidor"),
    ("welcome.searchingServer", "Procurando um servidor Haven local..."),
    ("welcome.serverFound", "Servidor Haven encontrado em:"),
    ("welcome.startServer", "Iniciar servidor"),
    ("welcome.serverNotDetected", "Nenhum servidor Haven detectado automaticamente."),
    ("welcome.browseServer", "Procurar diretório do servidor..."),
    ("welcome.freshSetup", "Configurar servidor novo"),
    ("welcome.startingServer", "Iniciando servidor..."),
    ("welcome.tryAgain", "Tentar novamente"),
    ("welcome.joinServer", "Entrar em um servidor"),
    ("welcome.serverAddress", "Endereço do servidor"),
    ("welcome.serverAddressHint", "Digite a URL ou o IP do servidor Haven."),
    ("welcome.connect", "Conectar"),
    ("welcome.connecting", "Conectando..."),
    ("tray.show", "Mostrar Haven"),
    ("tray.quit", "Sair"),
    ("tray.tooltip", "Haven Desktop"),
];

fn lookup(locale: &str, key: &str) -> Option<&'static str> {
    let table = if locale.eq_ignore_ascii_case("pt-BR") || locale.eq_ignore_ascii_case("pt") {
        PT_BR
    } else {
        EN
    };
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

pub fn supported_locales() -> Vec<LocaleInfo> {
    vec![
        LocaleInfo {
            code: "en".into(),
            name: "English".into(),
            direction: "ltr".into(),
        },
        LocaleInfo {
            code: "pt-BR".into(),
            name: "Português (Brasil)".into(),
            direction: "ltr".into(),
        },
    ]
}

pub fn normalize_locale(value: &str) -> Option<String> {
    let candidate = value.trim().replace('_', "-");
    if candidate.is_empty() {
        return None;
    }
    for loc in supported_locales() {
        if loc.code.eq_ignore_ascii_case(&candidate) {
            return Some(loc.code);
        }
    }
    let base = candidate.split('-').next()?.to_lowercase();
    for loc in supported_locales() {
        if loc.code.split('-').next().unwrap_or("").eq_ignore_ascii_case(&base) {
            return Some(loc.code);
        }
    }
    None
}

pub fn resolve_locale(preference: &str, system_languages: &[String]) -> String {
    if preference != "auto" && preference != "system" {
        return normalize_locale(preference).unwrap_or_else(|| "en".into());
    }
    for language in system_languages {
        if let Some(locale) = normalize_locale(language) {
            return locale;
        }
    }
    "en".into()
}

pub fn system_languages() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(lang) = std::env::var("LANG") {
        out.push(lang.split('.').next().unwrap_or(&lang).replace('_', "-"));
    }
    if let Ok(lang) = std::env::var("LC_ALL") {
        out.push(lang.split('.').next().unwrap_or(&lang).replace('_', "-"));
    }
    out.push("en".into());
    out
}

pub fn translate(locale: &str, key: &str, values: &HashMap<String, String>) -> String {
    let mut message = lookup(locale, key)
        .or_else(|| lookup("en", key))
        .unwrap_or(key)
        .to_string();
    for (k, v) in values {
        message = message.replace(&format!("{{{k}}}"), v);
    }
    message
}

pub fn build_state(
    preference: &str,
    locale: &str,
    system_locale: &str,
    is_preference_stored: bool,
) -> I18nState {
    let direction = supported_locales()
        .into_iter()
        .find(|l| l.code == locale)
        .map(|l| l.direction)
        .unwrap_or_else(|| "ltr".into());
    I18nState {
        preference: preference.to_string(),
        locale: locale.to_string(),
        system_locale: system_locale.to_string(),
        is_preference_stored,
        direction,
        supported_locales: supported_locales(),
    }
}
