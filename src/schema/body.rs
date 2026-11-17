use super::{FieldGroup, FieldSpec};
use crate::game_data::runtime::{RuntimeSlice, metadata};

/// Serialized scalar body fields discovered from `[JsonProperty]` metadata.
/// Unknown bounds are deliberately unbounded, not guessed from an old build.
pub const BODY_GROUPS: RuntimeSlice<FieldGroup> = RuntimeSlice(|| &metadata().body_groups);

pub fn body_field(key: &str) -> Option<&'static FieldSpec> {
    BODY_GROUPS
        .into_iter()
        .flat_map(|group| group.fields)
        .find(|field| field.key == key)
}
