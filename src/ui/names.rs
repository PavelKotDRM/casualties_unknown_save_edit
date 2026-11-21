//! Display-name lookup combining game locale data and interface translations.

use super::InterfaceLanguage;
use super::text::{prettify_id, strip_rich_text};
use crate::locale::Locale;
use crate::schema;
use std::borrow::Cow;

/// Display-name lookup that prefers the game's language file.
#[derive(Clone, Copy)]
pub struct Names<'a> {
    /// Loaded locale, if the game installation was found.
    pub locale: Option<&'a Locale>,
    /// Language used by the editor's interface.
    pub interface_language: InterfaceLanguage<'a>,
}

impl Names<'_> {
    /// Translates an editor interface string using the selected UI language.
    pub fn text<'a>(&self, source: &'a str) -> Cow<'a, str> {
        self.interface_language.text(source)
    }

    /// Readable, localized label for a body or limb field.
    pub fn field(&self, field: &schema::FieldSpec) -> String {
        if field.label != field.key {
            self.text(field.label).into_owned()
        } else {
            self.stat(field.key)
        }
    }

    /// Game label when available, otherwise a translated readable identifier.
    pub fn stat(&self, key: &str) -> String {
        let stem = key
            .strip_suffix("Amount")
            .or_else(|| key.strip_suffix("Level"))
            .unwrap_or(key);
        if let Some(text) = self
            .locale
            .and_then(|locale| locale.other(key).or_else(|| locale.moodle_name(stem)))
        {
            return strip_rich_text(text);
        }
        self.text(&humanize(stem)).into_owned()
    }

    /// Item display name (falls back to the id).
    pub fn item(&self, id: &str) -> String {
        self.locale
            .and_then(|l| l.item_name(id))
            .map_or_else(|| prettify_id(id), str::to_owned)
    }

    /// Item description, if available.
    pub fn item_description(&self, id: &str) -> Option<String> {
        self.locale
            .and_then(|l| l.item_description(id))
            .map(strip_rich_text)
    }

    /// Liquid display name.
    pub fn liquid(&self, id: &str) -> String {
        self.item(id)
    }

    /// Limb display name by index.
    pub fn limb(&self, index: usize) -> String {
        let Some(def) = schema::LIMBS.get(index) else {
            return format!("Limb #{index}");
        };
        let name = self
            .locale
            .and_then(|l| l.other(def.object))
            .unwrap_or(def.name)
            .to_owned();
        self.text(&name).into_owned()
    }

    /// Run setting label.
    pub fn run_setting(&self, id: &str) -> String {
        self.locale
            .and_then(|l| l.other(&format!("runset{id}")))
            .map(str::to_owned)
            .or_else(|| schema::run_setting(id).map(|d| d.label.to_owned()))
            .unwrap_or_else(|| id.to_owned())
    }

    /// Run setting description.
    pub fn run_setting_description(&self, id: &str) -> Option<String> {
        self.locale
            .and_then(|l| l.other(&format!("runset{id}dsc")))
            .map(strip_rich_text)
    }

    /// Dropdown option label of a run setting.
    pub fn run_setting_option(&self, id: &str, index: usize) -> String {
        let fallback = schema::run_setting(id).and_then(|d| match d.kind {
            schema::RunSettingKind::Dropdown(opts) => opts.get(index).copied(),
            _ => None,
        });
        let key = fallback.map(|f| format!("runset{id}{}", f.to_lowercase()));
        key.and_then(|k| self.locale.and_then(|l| l.other(&k)).map(str::to_owned))
            .or_else(|| fallback.map(str::to_owned))
            .unwrap_or_else(|| format!("Option {index}"))
    }

    /// Skill display name.
    pub fn skill(&self, def: &schema::SkillDef) -> String {
        let name = self
            .locale
            .and_then(|l| l.other(def.locale_key))
            .unwrap_or(def.name)
            .to_owned();
        self.text(&name).into_owned()
    }

    /// Playable character name (`character` field), e.g. `[SAW-03] "Milky"`.
    pub fn character(&self, character: i64) -> String {
        let index = usize::try_from(character).ok();
        index
            .and_then(|i| {
                self.locale
                    .and_then(|l| l.other(&format!("char{i}name")))
                    .map(strip_rich_text)
                    .or_else(|| schema::CHARACTERS.get(i).map(|d| d.name.to_owned()))
            })
            .unwrap_or_else(|| {
                self.text("Character #{index}")
                    .replace("{index}", &character.to_string())
            })
    }

    /// Character name, or a placeholder while the game still has it locked.
    pub fn character_if_unlocked(
        &self,
        character: usize,
        progress: &crate::player_prefs::GameProgress,
    ) -> String {
        if progress.is_unlocked(character) {
            self.character(character as i64)
        } else {
            format!(
                "{} {}",
                if progress.is_known() { "🔒" } else { "?" },
                self.text("Character #{index}")
                    .replace("{index}", &character.to_string())
            )
        }
    }

    /// Layer name, or a placeholder until the player has descended to it.
    pub fn layer_if_reached(
        &self,
        biome: i64,
        progress: &crate::player_prefs::GameProgress,
    ) -> String {
        if progress.is_layer_reached(biome) {
            self.layer(biome)
        } else {
            format!(
                "{} {}",
                if progress.is_known() { "🔒" } else { "?" },
                self.text("Layer {number}")
                    .replace("{number}", &(biome + 1).to_string())
            )
        }
    }

    /// Ending name with the case status of its end screen, e.g.
    /// `Escape — MORE INFO AT MEETING`.
    pub fn ending(&self, index: usize) -> String {
        let Some(def) = schema::ENDINGS.get(index) else {
            return self.ending_placeholder(index);
        };
        let status = self
            .locale
            .and_then(|l| l.other(def.status_key))
            .map(strip_rich_text)
            .unwrap_or_else(|| def.status.to_owned());
        format!("{} — {status}", self.text(def.name))
    }

    /// Ending name, or a placeholder until the player has reached it.
    pub fn ending_if_discovered(
        &self,
        index: usize,
        progress: &crate::player_prefs::GameProgress,
    ) -> String {
        if progress.is_ending_discovered(index) {
            self.ending(index)
        } else {
            format!(
                "{} {}",
                if progress.is_known() { "🔒" } else { "?" },
                self.ending_placeholder(index)
            )
        }
    }

    fn ending_placeholder(&self, index: usize) -> String {
        self.text("Ending #{number}")
            .replace("{number}", &(index + 1).to_string())
    }

    /// Dossier of a character (`char<i>desc`) from the game's language file.
    pub fn character_description(&self, character: usize) -> Option<String> {
        self.locale
            .and_then(|l| l.other(&format!("char{character}desc")))
            .map(strip_rich_text)
    }

    /// Quirks of a character (`char<i>quirks`) from the game's language file.
    pub fn character_quirks(&self, character: usize) -> Option<String> {
        self.locale
            .and_then(|l| l.other(&format!("char{character}quirks")))
            .map(strip_rich_text)
    }

    /// Requirement shown by the game for a locked character.
    pub fn character_unlock_requirement(&self) -> String {
        self.locale
            .and_then(|l| l.other("characterunlockdsc"))
            .map(strip_rich_text)
            .unwrap_or_else(|| {
                self.text("Finish a run with the previous character to unlock")
                    .into_owned()
            })
    }

    /// Hold slot display name (`items[].slot`), e.g. right hand or mouth.
    pub fn hold_slot(&self, slot: i64) -> String {
        let Some(def) = usize::try_from(slot)
            .ok()
            .and_then(|i| schema::HOLD_SLOTS.get(i))
        else {
            return self
                .text("Unknown slot {index}")
                .replace("{index}", &slot.to_string());
        };
        self.locale
            .and_then(|l| l.other(def.locale_key))
            .map_or_else(|| self.text(def.name).into_owned(), str::to_owned)
    }

    /// Name of the layer the run resumes on (`biome` field).
    pub fn layer(&self, biome: i64) -> String {
        let from_locale = biome
            .checked_add(1)
            .and_then(|layer| {
                self.locale
                    .and_then(|locale| locale.other(&format!("layertitle{layer}")))
            })
            .map(|s| s.replace('\n', " — "));
        from_locale
            .or_else(|| {
                usize::try_from(biome)
                    .ok()
                    .and_then(|i| schema::LAYERS.get(i))
                    .map(|s| (*s).to_owned())
            })
            .unwrap_or_else(|| self.text("Unknown layer").into_owned())
    }
}

fn humanize(key: &str) -> String {
    let mut out = String::new();
    for (i, c) in key.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_labels_preserve_explicit_labels_and_prefer_clean_game_names() {
        let locale = Locale::from_json(
            r##"{
            "other": {"bloodOxygen": "<b>Game oxygen</b>"},
            "moodles": {"pain": "<color=#ff0000>Game pain</color>"}
        }"##,
        )
        .unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::Russian,
        };
        assert_eq!(names.stat("bloodOxygen"), "Game oxygen");
        assert_eq!(names.stat("painAmount"), "Game pain");
        assert_eq!(names.stat("septicShock"), "Септический шок");
        let field = schema::FieldSpec {
            key: "bloodOxygen",
            label: "Stamina",
            help: "",
            kind: schema::Kind::Number,
            min: 0.0,
            max: 100.0,
            unit: "",
            good: schema::Good::Neutral,
        };
        assert_eq!(names.field(&field), "Выносливость");
    }
}
