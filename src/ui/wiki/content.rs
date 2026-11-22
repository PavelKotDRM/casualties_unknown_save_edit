//! Entries and section-specific content for the game wiki.

use super::*;
mod body;
mod characters;
mod conditions;
mod crafting;
mod items;
mod layers;
mod run;
#[cfg(test)]
pub(super) use run::run_settings;

pub(super) fn intro(section: Section) -> Option<&'static str> {
    Some(match section {
        Section::Characters => {
            "Playable characters. Each one is unlocked by finishing a run with the previous character."
        }
        Section::Endings => {
            "Ways a run can end successfully. Undiscovered endings and their total stay hidden unless you explicitly reveal them. Hints reveal one available detail at a time."
        }
        Section::Skills => {
            "Skills grow with experience. Each character has its own starting levels and experience multipliers."
        }
        Section::Body => "Whole-body stats stored in the save's body object.",
        Section::Limbs => "Every limb stores the same set of stats.",
        Section::Conditions => {
            "Status effects (moodles) shown by the game, with its own descriptions."
        }
        Section::RunSettings => {
            "World settings chosen when a run starts. Defaults are the values of the standard preset."
        }
        Section::Presets => "Built-in world presets and the settings they change.",
        Section::Layers => "World layers and the random modifiers a layer can get.",
        Section::Items => {
            "Items known to the game with their stats from the installation. Drop probabilities combine explicit rolls, unconditional drops, and calculated category-pool selections. Each pool draw chooses a listed category and then an item uniformly, with replacement. Effects are read from the game's code: amounts are per use unless marked otherwise."
        }
        Section::Crafting => "Recipes, crafting trees, requirements and special crafting rules.",
        Section::Liquids => {
            "Liquids that containers can hold, with the effects of drinking or injecting 100 ml, read from the game's code."
        }
    })
}

pub(super) fn entries(section: Section, sources: &Sources) -> Vec<Entry> {
    match section {
        Section::Characters => characters::characters(sources),
        Section::Endings => characters::endings(sources),
        Section::Skills => characters::skills(sources.names),
        Section::Body => schema::BODY_GROUPS
            .iter()
            .flat_map(|group| {
                group
                    .fields
                    .iter()
                    .map(move |field| body::field_entry(sources.names, field, group.title))
            })
            .collect(),
        Section::Limbs => body::limbs(sources.names),
        Section::Conditions => conditions::conditions(sources.names),
        Section::RunSettings => run::run_settings(sources.names),
        Section::Presets => run::presets(sources.names),
        Section::Layers => layers::layers(sources),
        Section::Items => items::items(sources),
        Section::Crafting => crafting::recipes(sources),
        Section::Liquids => items::liquids(sources),
    }
}
