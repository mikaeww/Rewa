//! Dialog plates: delete confirmation and the name prompt with its text field.

use crate::render::*;

#[derive(Clone, Copy)]
pub(crate) enum TextInputTarget {
    Search,
    Prompt,
}

impl Renderer {
    pub(crate) fn render_text_input(
        &mut self,
        input: &TextInput,
        field: LogicalRect,
        placeholder: &str,
        focused: bool,
        target: TextInputTarget,
    ) -> Result<(), String> {
        let body = self.body.clone();
        if input.value.is_empty() {
            self.text(placeholder, field, &body, self.palette.secondary)?;
        } else {
            if focused {
                let (start, end) = input.selection();
                if end > start {
                    let before: String = input.value.chars().take(start).collect();
                    let selected: String =
                        input.value.chars().skip(start).take(end - start).collect();
                    let offset = self.measure(&before, &body);
                    let width = self.measure(&selected, &body);
                    self.fill(
                        rect(
                            field.left + offset,
                            field.top + 8.0,
                            (field.left + offset + width).min(field.right),
                            field.bottom - 8.0,
                        ),
                        self.palette.selection,
                        3.0,
                    )?;
                }
            }
            self.text(&input.value, field, &body, self.palette.primary)?;
        }

        if focused {
            let prefixes = (0..=input.characters())
                .map(|count| {
                    let prefix: String = input.value.chars().take(count).collect();
                    self.measure(&prefix, &body)
                })
                .collect::<Vec<_>>();
            let caret_x = (field.left + prefixes[input.caret]).min(field.right);
            self.fill(
                rect(caret_x, field.top + 8.0, caret_x + 1.5, field.bottom - 8.0),
                self.palette.primary,
                0.0,
            )?;
            for index in 0..=input.characters() {
                let left = if index == 0 {
                    field.left
                } else {
                    field.left + (prefixes[index - 1] + prefixes[index]) / 2.0
                };
                let right = if index == input.characters() {
                    field.right
                } else {
                    field.left + (prefixes[index] + prefixes[index + 1]) / 2.0
                };
                self.hits.push(HitRegion {
                    rect: rect(
                        left.min(field.right),
                        field.top,
                        right.min(field.right),
                        field.bottom,
                    ),
                    action: match target {
                        TextInputTarget::Search => Action::PlaceSearchCaret(index),
                        TextInputTarget::Prompt => Action::PlacePromptCaret(index),
                    },
                });
            }
        }
        Ok(())
    }

    pub(crate) fn render_delete_modal(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(target) = &model.pending_delete else {
            return Ok(());
        };
        let (title, detail, confirmation) = match target {
            DeleteTarget::Clip(index) => {
                let name = model
                    .clips
                    .get(*index)
                    .map_or(self.strings.this_clip, |clip| clip.title.as_str());
                (
                    self.strings.delete_clip_question,
                    self.strings.delete_clip_body(name),
                    self.strings.delete,
                )
            }
            DeleteTarget::Collection(path) => {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(self.strings.this_collection);
                (
                    self.strings.delete_collection_question,
                    self.strings.delete_collection_body(name),
                    self.strings.delete,
                )
            }
        };
        self.plate(
            width,
            height,
            380.0,
            176.0,
            Action::CancelDelete,
            |renderer, modal| {
                renderer.text(
                    title,
                    rect(
                        modal.left + 22.0,
                        modal.top + 16.0,
                        modal.right - 22.0,
                        modal.top + 44.0,
                    ),
                    &renderer.heading.clone(),
                    renderer.palette.primary,
                )?;
                renderer.text(
                    &detail,
                    rect(
                        modal.left + 22.0,
                        modal.top + 48.0,
                        modal.right - 22.0,
                        modal.bottom - 62.0,
                    ),
                    &renderer.body_wrap.clone(),
                    renderer.palette.secondary,
                )?;
                renderer.plate_foot(
                    modal,
                    [
                        (
                            renderer.strings.cancel,
                            Action::CancelDelete,
                            PlateButton::Plain,
                        ),
                        (
                            confirmation,
                            Action::ConfirmDelete,
                            PlateButton::Destructive,
                        ),
                    ],
                )
            },
        )
    }

    pub(crate) fn render_prompt_modal(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(prompt) = &model.prompt else {
            return Ok(());
        };
        self.plate(
            width,
            height,
            420.0,
            204.0,
            Action::CancelPrompt,
            |renderer, modal| {
                renderer.text(
                    prompt.title(renderer.strings),
                    rect(
                        modal.left + 22.0,
                        modal.top + 16.0,
                        modal.right - 22.0,
                        modal.top + 44.0,
                    ),
                    &renderer.heading.clone(),
                    renderer.palette.primary,
                )?;
                renderer.text(
                    prompt.label(renderer.strings),
                    rect(
                        modal.left + 22.0,
                        modal.top + 50.0,
                        modal.right - 22.0,
                        modal.top + 68.0,
                    ),
                    &renderer.small.clone(),
                    renderer.palette.secondary,
                )?;
                let field = rect(
                    modal.left + 22.0,
                    modal.top + 72.0,
                    modal.right - 22.0,
                    modal.top + 104.0,
                );
                renderer.tint(field, 0.07, 9.0)?;
                renderer.stroke(
                    field,
                    mix(renderer.palette.surface, renderer.palette.primary, 0.3),
                    9.0,
                    1.0,
                )?;
                renderer.render_text_input(
                    &prompt.input,
                    rect(
                        field.left + 10.0,
                        field.top,
                        field.right - 10.0,
                        field.bottom,
                    ),
                    "",
                    true,
                    TextInputTarget::Prompt,
                )?;
                renderer.text(
                    renderer.strings.prompt_hint,
                    rect(
                        modal.left + 22.0,
                        field.bottom + 6.0,
                        modal.right - 22.0,
                        field.bottom + 24.0,
                    ),
                    &renderer.small.clone(),
                    renderer.palette.muted,
                )?;
                renderer.plate_foot(
                    modal,
                    [
                        (
                            renderer.strings.cancel,
                            Action::CancelPrompt,
                            PlateButton::Plain,
                        ),
                        (
                            prompt.confirm(renderer.strings),
                            Action::ConfirmPrompt,
                            PlateButton::Primary,
                        ),
                    ],
                )
            },
        )
    }

    /// Leech's panel: a dimmed stage and the plate on it; a click outside dismisses it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn plate(
        &mut self,
        width: f32,
        height: f32,
        plate_width: f32,
        plate_height: f32,
        dismiss: Action,
        body: impl FnOnce(&mut Self, LogicalRect) -> Result<(), String>,
    ) -> Result<(), String> {
        let overlay = rect(0.0, 0.0, width, height);
        self.fill_alpha(overlay, 0x000000, 0.38, 0.0)?;
        self.hits.push(HitRegion {
            rect: overlay,
            action: dismiss,
        });
        let plate_width = plate_width.min(width - 40.0);
        let left = (width - plate_width) / 2.0;
        let top = (height - plate_height) / 2.0;
        let modal = rect(left, top, left + plate_width, top + plate_height);
        self.hits.push(HitRegion {
            rect: modal,
            action: Action::Ignore,
        });
        self.plate_surface(modal)?;
        body(self, modal)
    }

    /// The plate's foot: a hairline and the actions on the right, the default last.
    pub(crate) fn plate_foot<const N: usize>(
        &mut self,
        modal: LogicalRect,
        buttons: [(&str, Action, PlateButton); N],
    ) -> Result<(), String> {
        let foot_top = modal.bottom - 52.0;
        self.fill(
            rect(modal.left, foot_top, modal.right, foot_top + 1.0),
            self.palette.hairline,
            0.0,
        )?;
        let center = (foot_top + modal.bottom) / 2.0;
        let mut right = modal.right - 18.0;
        for (label, action, kind) in buttons.into_iter().rev() {
            let width = (self.measure(label, &self.button) + 28.0).max(84.0);
            let area = rect(right - width, center - 14.0, right, center + 14.0);
            let (background, foreground) = match kind {
                PlateButton::Plain => (self.palette.surface, self.palette.primary),
                PlateButton::Primary => (self.palette.accent, self.palette.accent_text),
                PlateButton::Destructive => (self.palette.destructive, 0xffffff),
            };
            self.pill(area, background, label, foreground, Some(action))?;
            right = area.left - 8.0;
        }
        Ok(())
    }
}
