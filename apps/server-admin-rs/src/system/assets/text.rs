use crate::i18n::Translator;

pub(super) fn dnsmasq_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.dnsmasq.{key}"))
}

pub(super) fn dnsmasq_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.dnsmasq.{key}"), params)
}

pub(super) fn tunnel_manager_text(translator: &Translator, manager: &str, key: &str) -> String {
    translator.t(&format!("server.tunnelManagers.{manager}.{key}"))
}

pub(super) fn tunnel_manager_text_params(
    translator: &Translator,
    manager: &str,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.tunnelManagers.{manager}.{key}"), params)
}
