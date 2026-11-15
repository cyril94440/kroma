use std::sync::OnceLock;

use kroma_module_sdk::i18n::{self, I18n};

const CATALOGS: &[(&str, &str)] = kroma_module_sdk::embedded_locales!();

pub fn text(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    static ENGINE: OnceLock<I18n> = OnceLock::new();
    ENGINE
        .get_or_init(|| i18n::engine(CATALOGS).expect("this module's catalogs"))
        .translator(locale)
        .t(key, vars)
}
