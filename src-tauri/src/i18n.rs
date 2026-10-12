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
    ("dialog.saveImage", "Save image"),
    ("download.saved", "Download saved"),
    ("download.failed", "Download failed"),
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
    ("update.upToDate", "You're on the latest version, Haven Desktop v{version}."),
    ("update.notPackaged", "Update checks only work in the installed app."),
    ("update.later", "Later"),
    ("update.installFailed", "The update could not be installed."),
    ("menu.checkForUpdates", "Check for Updates"),
    ("serverList.gate.title", "Your server list"),
    ("serverList.gate.add", "Add \"{name}\" at {url} to your server list?"),
    ("serverList.gate.addBack", "Add \"{name}\" at {url} back to your server list? You removed it earlier."),
    ("serverList.gate.remove", "Remove \"{name}\" ({url}) from your server list?"),
    ("serverList.gate.rename", "Rename \"{oldName}\" ({url}) to \"{name}\"?"),
    ("serverList.gate.changeIcon", "Change the icon of \"{name}\" ({url})?"),
    ("serverList.gate.iconSet", "Its icon will load from {icon} on every server you open."),
    ("serverList.gate.iconRemoved", "Its custom icon will be removed."),
    ("serverList.gate.open", "Open {url} and add it to your server list?"),
    ("serverList.gate.askedBy", "Asked by the page of {name} ({url})."),
    ("serverList.gate.explain", "Every server you open in Haven shows this list, so allow it only if you made this change yourself."),
    ("serverList.gate.block", "Ignore {name} until restart"),
    ("serverList.gate.confirmAdd", "Add"),
    ("serverList.gate.confirmRemove", "Remove"),
    ("serverList.gate.confirmChange", "Change"),
    ("serverList.gate.confirmOpen", "Open"),
    ("cert.title", "Certificate check"),
    ("cert.unknownMessage", "{host} is using a certificate your computer can't verify."),
    ("cert.unknownDetail", "That's normal for a Haven server that made its own certificate. Trust it only if this is a server you know, because on a shared network someone could be pretending to be it. Haven remembers your answer, and asks again if the certificate ever changes."),
    ("cert.changedMessage", "The certificate for {host} has changed."),
    ("cert.changedDetail", "This happens when a server gets a new certificate, but it can also mean someone is intercepting your connection. Trust the new one only if whoever runs the server says it changed."),
    ("cert.fingerprint", "Fingerprint: {value}"),
    ("cert.previousFingerprint", "Previous fingerprint: {value}"),
    ("cert.issuer", "Issued by: {value}"),
    ("cert.expires", "Valid until: {value}"),
    ("cert.trust", "Trust this certificate"),
    ("cert.trustNew", "Trust the new certificate"),
    ("cert.vpnTitle", "Your VPN is presenting its own certificate"),
    ("cert.networkTitle", "This network is presenting its own certificate"),
    ("cert.certFor", "Haven got a certificate for {host} issued by {issuer} instead of the usual one. Trust it to keep using Haven on this connection."),
    ("cert.vpnTip", "If your VPN has threat protection or a web filter, allow {host} there, or exclude Haven from the VPN."),
    ("cert.keyChanged", "This is different from the certificate you trusted before."),
    ("cert.details", "Details"),
    ("cert.issuedTo", "Issued to: {value}"),
    ("cert.valid", "Valid: {from} to {to}"),
    ("net.redirectTitle", "Haven was redirected"),
    ("net.redirectBody", "Your network or VPN redirected Haven to {host}. It may need you to sign in or allow this site."),
    ("net.openSignIn", "Open sign-in page"),
    ("net.captiveTitle", "Sign in to this Wi-Fi to continue"),
    ("net.captiveBody", "This Wi-Fi needs you to sign in before Haven can connect. Haven reconnects by itself once you're signed in."),
    ("net.autoRetry", "Haven tries again by itself when the connection comes back."),
    ("net.untrustedBody", "Haven didn't connect because the certificate wasn't trusted. Try again to see the question again."),
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
    ("dialog.cancel", "Cancelar"),
    ("update.upToDate", "Você já está na versão mais recente, Haven Desktop v{version}."),
    ("update.notPackaged", "A verificação de atualizações só funciona no aplicativo instalado."),
    ("menu.checkForUpdates", "Verificar atualizações"),
    ("cert.title", "Verificação de certificado"),
    ("cert.unknownMessage", "{host} usa um certificado que seu computador não consegue verificar."),
    ("cert.unknownDetail", "Isso é normal em um servidor Haven que criou o próprio certificado. Confie nele só se for um servidor que você conhece, porque em uma rede compartilhada alguém pode estar se passando por ele. O Haven lembra sua resposta e pergunta de novo se o certificado mudar."),
    ("cert.changedMessage", "O certificado de {host} mudou."),
    ("cert.changedDetail", "Isso acontece quando um servidor recebe um certificado novo, mas também pode significar que alguém está interceptando sua conexão. Confie no novo só se quem administra o servidor disser que ele mudou."),
    ("cert.fingerprint", "Impressão digital: {value}"),
    ("cert.previousFingerprint", "Impressão digital anterior: {value}"),
    ("cert.issuer", "Emitido por: {value}"),
    ("cert.expires", "Válido até: {value}"),
    ("cert.trust", "Confiar neste certificado"),
    ("cert.trustNew", "Confiar no novo certificado"),
    ("cert.vpnTitle", "Sua VPN está apresentando o próprio certificado"),
    ("cert.networkTitle", "Esta rede está apresentando o próprio certificado"),
    ("cert.certFor", "O Haven recebeu para {host} um certificado emitido por {issuer} em vez do habitual. Confie nele para continuar usando o Haven nesta conexão."),
    ("cert.vpnTip", "Se a sua VPN tiver proteção contra ameaças ou filtro web, permita {host} nela ou exclua o Haven da VPN."),
    ("cert.keyChanged", "Ele é diferente do certificado em que você confiou antes."),
    ("cert.details", "Detalhes"),
    ("cert.issuedTo", "Emitido para: {value}"),
    ("cert.valid", "Válido de: {from} a {to}"),
    ("net.redirectTitle", "O Haven foi redirecionado"),
    ("net.redirectBody", "Sua rede ou VPN redirecionou o Haven para {host}. Talvez você precise fazer login ou permitir este site."),
    ("net.openSignIn", "Abrir página de login"),
    ("net.captiveTitle", "Faça login neste Wi-Fi para continuar"),
    ("net.captiveBody", "Este Wi-Fi pede login antes que o Haven possa se conectar. O Haven se reconecta sozinho quando você fizer login."),
    ("net.autoRetry", "O Haven tenta de novo sozinho quando a conexão voltar."),
    ("net.untrustedBody", "O Haven não se conectou porque o certificado não foi aceito. Tente de novo para ver a pergunta outra vez."),
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_and_certificate_strings_are_translated() {
        for (k, _) in EN.iter().filter(|(k, _)| k.starts_with("cert.") || k.starts_with("net.")) { assert!(PT_BR.iter().any(|(p, _)| p == k), "{k}") }
        let v: HashMap<String, String> = [("host".to_string(), "h.example".to_string()), ("issuer".to_string(), "Acme".to_string())].into();
        assert_eq!(translate("pt-BR", "net.redirectBody", &v), "Sua rede ou VPN redirecionou o Haven para h.example. Talvez você precise fazer login ou permitir este site.");
        assert!(translate("en", "cert.certFor", &v).contains("h.example issued by Acme"));
    }
}
