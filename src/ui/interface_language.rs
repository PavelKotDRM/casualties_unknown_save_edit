//! Built-in interface translations.

use std::borrow::Cow;

use crate::interface_packs::InterfacePack;

mod russian;

/// Language of the editor's own labels and menus.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceLanguage<'a> {
    /// English UI.
    #[default]
    English,
    /// Russian UI.
    Russian,
    /// A user-provided interface language.
    External(&'a InterfacePack),
}

impl<'p> InterfaceLanguage<'p> {
    /// Configuration value.
    pub fn code(self) -> &'p str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
            Self::External(pack) => &pack.code,
        }
    }

    /// Parses a configuration value, falling back to English.
    pub fn from_code(code: Option<&str>, pack: Option<&'p InterfacePack>) -> Self {
        match code {
            Some("ru") => Self::Russian,
            Some("en") | None => Self::English,
            Some(code) => pack
                .filter(|p| p.code == code)
                .map_or(Self::English, Self::External),
        }
    }

    /// Translates a built-in English interface string.
    pub fn text<'a>(self, source: &'a str) -> Cow<'a, str> {
        let translated = match self {
            Self::English => None,
            Self::External(pack) => {
                return pack
                    .translations
                    .get(source)
                    .filter(|text| !text.trim().is_empty())
                    .map_or(Cow::Borrowed(source), |text| Cow::Owned(text.clone()));
            }
            Self::Russian => russian::translate(source),
        };
        translated.map_or(Cow::Borrowed(source), Cow::Borrowed)
    }
}
