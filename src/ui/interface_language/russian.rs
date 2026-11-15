mod application;
mod character;
mod general;
mod health;
mod inventory;

pub(super) fn translate(source: &str) -> Option<&'static str> {
    general::translate(source)
        .or_else(|| health::translate(source))
        .or_else(|| character::translate(source))
        .or_else(|| application::translate(source))
        .or_else(|| inventory::translate(source))
}
