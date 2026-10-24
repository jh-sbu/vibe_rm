//! Whether actors can be killed: essential and protected actors (the base's
//! `ACBS` flags, or filling a quest alias with the flag while its quest runs)
//! bleed out instead of dying, protected ones but to the player. Choices:
//! `known_ambiguities/protection.md`.

#[cfg(test)]
#[path = "protection_tests.rs"]
mod tests;
