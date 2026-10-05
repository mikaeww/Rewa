//! The pages inside the stage card.

mod clips;
mod collections;
mod editor;
mod library;
mod player;
mod setting_lines;
mod settings;

pub(crate) use clips::{clips_overflow, library_layout};
pub(crate) use setting_lines::LineControl;
