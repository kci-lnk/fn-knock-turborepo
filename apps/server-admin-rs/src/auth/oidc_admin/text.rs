use crate::i18n::Translator;

pub(super) fn oidc_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.oidc.{key}"))
}

pub(super) fn oidc_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.oidc.{key}"), params)
}
