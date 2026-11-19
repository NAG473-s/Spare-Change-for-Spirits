//! Translations. Every player-facing text lives in `assets/i18n/<code>.ron`;
//! `en.ron` is the source and defines the keys. Placeholders look like
//! `{name}` and are filled by [`Lang::f`].
//!
//! Content keys follow the data ids: `product.<id>.name`,
//! `product.<id>.hint.N`, `ghost.<id>.name`, `ghost.<id>.the`,
//! `decor.<id>.name`, `story.N.intro`, `tip.<step>`, `tell.genuine.N` …

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use crate::data::assets_dir;

pub const SOURCE: &str = "en";

#[derive(Deserialize, Clone, Debug)]
pub struct LanguageDef {
    pub code: String,
    /// Name in the language itself, as shown in the language menu.
    pub name: String,
    /// Extra font (in assets/fonts) for scripts the main font lacks.
    #[serde(default)]
    pub font: Option<String>,
}

pub type Catalog = BTreeMap<String, String>;

fn read_catalog(code: &str) -> Option<Catalog> {
    let path = assets_dir().join("i18n").join(format!("{code}.ron"));
    let text = std::fs::read_to_string(&path).ok()?;
    match ron::from_str(&text) {
        Ok(c) => Some(c),
        Err(e) => {
            // A broken translation must not stop the game: fall back to English.
            warn!("error in {}: {e}", path.display());
            None
        }
    }
}

pub fn languages() -> Vec<LanguageDef> {
    let path = assets_dir().join("i18n/languages.ron");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("error in {}: {e}", path.display()))
}

/// The active language with English as fallback for missing keys.
#[derive(Resource)]
pub struct Lang {
    pub code: String,
    pub languages: Vec<LanguageDef>,
    strings: Catalog,
    source: Catalog,
}

impl Lang {
    pub fn load(code: &str) -> Self {
        let languages = languages();
        let code = if languages.iter().any(|l| l.code == code) {
            code
        } else {
            SOURCE
        };
        let source = read_catalog(SOURCE).expect("assets/i18n/en.ron is required");
        let strings = if code == SOURCE {
            source.clone()
        } else {
            read_catalog(code).unwrap_or_default()
        };
        Self {
            code: code.to_string(),
            languages,
            strings,
            source,
        }
    }

    pub fn def(&self) -> Option<&LanguageDef> {
        self.languages.iter().find(|l| l.code == self.code)
    }

    /// The text for a key: translation, else English, else the key itself.
    pub fn t(&self, key: &str) -> String {
        self.strings
            .get(key)
            .or_else(|| self.source.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    /// Like [`Lang::t`] with `{placeholder}`s filled in.
    pub fn f(&self, key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
        fill(&self.t(key), args)
    }

    /// How many numbered keys `<prefix>1`, `<prefix>2`, … the source has.
    pub fn count(&self, prefix: &str) -> usize {
        (1..)
            .take_while(|n| self.source.contains_key(&format!("{prefix}{n}")))
            .count()
    }

    /// Joins items with the language's list separator.
    pub fn join(&self, items: &[String]) -> String {
        items.join(&self.t("list.separator"))
    }

    pub fn product(&self, id: &str) -> String {
        self.t(&format!("product.{id}.name"))
    }

    pub fn ghost(&self, id: &str) -> String {
        self.t(&format!("ghost.{id}.name"))
    }

    /// A ghost as the subject at the start of a sentence ("The Shy Ghost").
    pub fn ghost_the(&self, id: &str) -> String {
        self.t(&format!("ghost.{id}.the"))
    }

    pub fn decor(&self, id: &str) -> String {
        self.t(&format!("decor.{id}.name"))
    }
}

pub fn fill(template: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

/// Language from the environment (LANGUAGE, LC_ALL, LC_MESSAGES, LANG), e.g.
/// "de_DE.UTF-8" → "de", "pt_BR" → "pt-BR", "zh_TW" → "zh-Hant".
pub fn detect(languages: &[LanguageDef]) -> Option<String> {
    let raw = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")?;
    map_locale(&raw, languages)
}

pub fn map_locale(raw: &str, languages: &[LanguageDef]) -> Option<String> {
    let locale = raw.split([':', '.', '@']).next()?.replace('_', "-");
    let mut parts = locale.split('-');
    let lang = parts.next()?.to_lowercase();
    let region = parts.next().unwrap_or("").to_uppercase();
    let candidate = match (lang.as_str(), region.as_str()) {
        ("zh", "TW" | "HK" | "MO" | "HANT") => "zh-Hant".to_string(),
        ("zh", _) => "zh-Hans".to_string(),
        ("pt", "BR") => "pt-BR".to_string(),
        ("pt", _) => "pt-PT".to_string(),
        ("es", "ES" | "") => "es".to_string(),
        ("es", _) => "es-419".to_string(),
        ("nb" | "nn", _) => "no".to_string(),
        (l, _) => l.to_string(),
    };
    languages
        .iter()
        .any(|l| l.code == candidate)
        .then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> Vec<String> {
        let mut out: Vec<String> = s
            .split('{')
            .skip(1)
            .filter_map(|p| p.split_once('}').map(|(name, _)| name.to_string()))
            .collect();
        out.sort();
        out
    }

    /// Every language has every key of en.ron, no extra keys, and the same
    /// placeholders; every language file parses.
    #[test]
    fn translations_are_complete() {
        let source = read_catalog(SOURCE).unwrap();
        let mut problems = Vec::new();
        for lang in languages() {
            if lang.code == SOURCE {
                continue;
            }
            let Some(cat) = read_catalog(&lang.code) else {
                problems.push(format!("{}: missing or unreadable file", lang.code));
                continue;
            };
            for (key, text) in &source {
                match cat.get(key) {
                    None => problems.push(format!("{}: missing {key}", lang.code)),
                    Some(t) if placeholders(t) != placeholders(text) => {
                        problems.push(format!("{}: placeholders differ in {key}", lang.code))
                    }
                    Some(t) if t.trim().is_empty() => {
                        problems.push(format!("{}: empty {key}", lang.code))
                    }
                    _ => {}
                }
            }
            for key in cat.keys().filter(|k| !source.contains_key(*k)) {
                problems.push(format!("{}: unknown key {key}", lang.code));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn fills_placeholders() {
        assert_eq!(fill("{a} and {b}", &[("a", &1), ("b", &"x")]), "1 and x");
    }

    #[test]
    fn maps_locales() {
        let langs = languages();
        assert_eq!(map_locale("de_DE.UTF-8", &langs).as_deref(), Some("de"));
        assert_eq!(map_locale("pt_BR", &langs).as_deref(), Some("pt-BR"));
        assert_eq!(
            map_locale("zh_TW.UTF-8", &langs).as_deref(),
            Some("zh-Hant")
        );
        assert_eq!(map_locale("es_MX", &langs).as_deref(), Some("es-419"));
        assert_eq!(map_locale("nb_NO", &langs).as_deref(), Some("no"));
        assert_eq!(map_locale("xx_YY", &langs), None);
    }
}
