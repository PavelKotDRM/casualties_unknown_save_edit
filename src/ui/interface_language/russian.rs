mod about;
mod application;
mod character;
mod endings;
mod general;
mod health;
mod inventory;
mod wiki;

pub(super) fn translate(source: &str) -> Option<&'static str> {
    general::translate(source)
        .or_else(|| health::translate(source))
        .or_else(|| character::translate(source))
        .or_else(|| application::translate(source))
        .or_else(|| inventory::translate(source))
        .or_else(|| wiki::translate(source))
        .or_else(|| endings::translate(source))
        .or_else(|| about::translate(source))
}
