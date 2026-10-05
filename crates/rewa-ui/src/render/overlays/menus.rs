//! Popup menus: the choice menu of a setting or filter and the clip context menu.

use crate::render::*;

impl Renderer {
    pub(crate) fn render_settings_menu(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(menu_state) = &model.settings_menu else {
            return Ok(());
        };
        if menu_state.items.is_empty() {
            return Ok(());
        }

        self.hits.push(HitRegion {
            rect: rect(0.0, 0.0, width, height),
            action: Action::DismissSettingsMenu,
        });

        let target_action = match menu_state.kind {
            SettingsMenuKind::Theme => Action::ChooseTheme,
            SettingsMenuKind::Language => Action::ChooseLanguage,
            SettingsMenuKind::HoverStyle => Action::ChooseHoverStyle,
            SettingsMenuKind::HoverStrength => Action::ChooseHoverStrength,
            SettingsMenuKind::TimeFilter => Action::ChooseTimeFilter,
            SettingsMenuKind::CollectionFilter => Action::ChooseCollectionFilter,
            SettingsMenuKind::TypeFilter => Action::ChooseTypeFilter,
            SettingsMenuKind::SizeFilter => Action::ChooseSizeFilter,
            SettingsMenuKind::ClipSort => Action::ChooseClipSort,
            SettingsMenuKind::Display => Action::ChooseDisplay,
            SettingsMenuKind::FrameRate => Action::ChooseFrameRate,
            SettingsMenuKind::Duration => Action::ChooseDuration,
            SettingsMenuKind::Codec => Action::ChooseCodec,
            SettingsMenuKind::Quality => Action::ChooseQuality,
            SettingsMenuKind::AudioMode => Action::ChooseAudioMode,
            SettingsMenuKind::DesktopDevice => Action::ChooseDesktopDevice,
            SettingsMenuKind::DesktopGain => Action::ChooseDesktopGain,
            SettingsMenuKind::Microphone => Action::ChooseMicrophone,
            SettingsMenuKind::MicrophoneGain => Action::ChooseMicrophoneGain,
            SettingsMenuKind::StorageLimit => Action::ChooseStorageLimit,
        };
        let anchor = self
            .hits
            .iter()
            .rev()
            .find(|hit| hit.action == target_action)
            .map_or(rect(width - 380.0, 150.0, width - 40.0, 190.0), |hit| {
                hit.rect
            });
        let control_width = (anchor.right - anchor.left).max(190.0);
        let columns = if menu_state.kind == SettingsMenuKind::DesktopGain {
            3
        } else {
            1
        };
        let has_details = menu_state.items.iter().any(|item| item.detail.is_some());
        let item_height = if has_details { 48.0 } else { 30.0 };
        let rows = menu_state.items.len().div_ceil(columns);
        let menu_height = 12.0 + rows as f32 * item_height;
        let menu_width = control_width.max(if has_details { 310.0 } else { 190.0 });
        let menu_left = (anchor.right - menu_width).max(18.0);
        let below = anchor.bottom + 8.0;
        let above = anchor.top - menu_height - 8.0;
        let menu_top = if below + menu_height <= height - 18.0 {
            below
        } else {
            above.max(18.0)
        };
        let menu = rect(
            menu_left,
            menu_top,
            menu_left + menu_width,
            menu_top + menu_height,
        );

        self.popover_surface(menu)?;

        let cell_width = (menu_width - 12.0) / columns as f32;
        for (index, item) in menu_state.items.iter().enumerate() {
            let column = index % columns;
            let row = index / columns;
            let item_area = rect(
                menu.left + 6.0 + column as f32 * cell_width,
                menu.top + 6.0 + row as f32 * item_height,
                menu.left + 6.0 + (column + 1) as f32 * cell_width,
                menu.top + 6.0 + (row + 1) as f32 * item_height,
            );
            let action = Action::SelectSettingsOption(index);
            let selected = menu_state.selected == Some(index);
            let highlighted = menu_state.highlighted == index || self.is_hovered(&action);
            if highlighted {
                self.tint(item_area, 0.08, RADIUS_SMALL - 2.0)?;
            }
            if selected {
                if columns > 1 {
                    self.tint(item_area, 0.12, RADIUS_SMALL - 2.0)?;
                } else {
                    // a macOS menu marks the current choice with a check, not a bar
                    let center_y = if item.detail.is_some() {
                        item_area.top + 16.5
                    } else {
                        (item_area.top + item_area.bottom) / 2.0
                    };
                    self.glyph(
                        Glyph::Check,
                        rect(
                            item_area.left + 5.0,
                            center_y - 6.0,
                            item_area.left + 17.0,
                            center_y + 6.0,
                        ),
                        self.palette.primary,
                    )?;
                }
            }
            if columns > 1 {
                self.text(
                    &item.label,
                    item_area,
                    &self.body_center.clone(),
                    self.palette.primary,
                )?;
            } else if let Some(detail) = &item.detail {
                self.text(
                    &item.label,
                    rect(
                        item_area.left + 20.0,
                        item_area.top + 4.0,
                        item_area.right - 12.0,
                        item_area.top + 26.0,
                    ),
                    &self.body.clone(),
                    self.palette.primary,
                )?;
                self.text(
                    detail,
                    rect(
                        item_area.left + 20.0,
                        item_area.top + 24.0,
                        item_area.right - 12.0,
                        item_area.bottom - 3.0,
                    ),
                    &self.small.clone(),
                    self.palette.secondary,
                )?;
            } else {
                self.text(
                    &item.label,
                    rect(
                        item_area.left + 20.0,
                        item_area.top,
                        item_area.right - 12.0,
                        item_area.bottom,
                    ),
                    &self.body.clone(),
                    self.palette.primary,
                )?;
            }
            self.hits.push(HitRegion {
                rect: item_area,
                action,
            });
        }
        Ok(())
    }

    pub(crate) fn render_context_menu(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(context) = model.context_menu else {
            return Ok(());
        };
        let Some(clip) = model.clips.get(context.clip) else {
            return Ok(());
        };
        self.hits.push(HitRegion {
            rect: rect(0.0, 0.0, width, height),
            action: Action::DismissContextMenu,
        });

        enum Entry<'a> {
            Header(&'a str),
            Row(&'a str, Action, bool),
            Rule,
        }
        let index = context.clip;
        let mut entries = vec![
            Entry::Header(clip.title.as_str()),
            Entry::Row(
                if model.is_favorite(index) {
                    self.strings.favorite_remove
                } else {
                    self.strings.favorite_add
                },
                Action::ToggleFavorite(index),
                false,
            ),
            Entry::Row(self.strings.edit_clip, Action::EditClip(index), false),
            Entry::Row(self.strings.rename, Action::RenameClip(index), false),
            Entry::Rule,
            Entry::Row(
                self.strings.open_in_explorer,
                Action::ShowClipInExplorer(index),
                false,
            ),
            Entry::Row(
                self.strings.open_in_player,
                Action::OpenClipExternally(index),
                false,
            ),
        ];
        if !model.collections.is_empty() {
            entries.push(Entry::Rule);
            entries.push(Entry::Header(self.strings.move_to_collection));
            for (collection, item) in model.collections.iter().take(6).enumerate() {
                entries.push(Entry::Row(
                    &item.name,
                    Action::MoveClipToCollection {
                        clip: index,
                        collection,
                    },
                    false,
                ));
            }
        }
        entries.extend([
            Entry::Rule,
            Entry::Row(
                self.strings.select_multiple,
                Action::ToggleSelectionMode,
                false,
            ),
            Entry::Row(self.strings.delete_clip, Action::DeleteClip(index), true),
        ]);

        const ROW: f32 = 26.0;
        const RULE: f32 = 11.0;
        let pitch = |entry: &Entry<'_>| match entry {
            Entry::Rule => RULE,
            _ => ROW,
        };
        let menu_width = 240.0;
        let menu_height = 10.0 + entries.iter().map(pitch).sum::<f32>();
        let left = context.x.min(width - menu_width - 12.0).max(12.0);
        // a menu near the foot opens upwards from the pointer, the way macOS flips it
        let top = if context.y + menu_height <= height - 12.0 {
            context.y
        } else {
            (context.y - menu_height).max(12.0)
        };
        let menu = rect(left, top, left + menu_width, top + menu_height);
        self.hits.push(HitRegion {
            rect: menu,
            action: Action::Ignore,
        });
        self.popover_surface(menu)?;
        let mut row_top = menu.top + 5.0;
        for entry in entries {
            let step = pitch(&entry);
            let row = rect(menu.left + 5.0, row_top, menu.right - 5.0, row_top + step);
            match entry {
                Entry::Header(label) => {
                    self.text(
                        &self.shorten(label, &self.caption, row.right - row.left - 24.0),
                        rect(row.left + 12.0, row.top, row.right - 12.0, row.bottom),
                        &self.caption.clone(),
                        self.palette.muted,
                    )?;
                }
                Entry::Rule => {
                    let middle = (row.top + row.bottom) / 2.0;
                    self.fill(
                        rect(row.left + 11.0, middle, row.right - 11.0, middle + 1.0),
                        self.palette.hairline,
                        0.0,
                    )?;
                }
                Entry::Row(label, action, dangerous) => {
                    self.context_menu_row(row, label, action, dangerous)?;
                }
            }
            row_top += step;
        }
        Ok(())
    }

    pub(crate) fn context_menu_row(
        &mut self,
        area: LogicalRect,
        label: &str,
        action: Action,
        dangerous: bool,
    ) -> Result<(), String> {
        if self.is_hovered(&action) {
            if dangerous {
                self.fill_alpha(area, self.palette.destructive, 0.14, 6.0)?;
            } else {
                self.tint(area, 0.08, 6.0)?;
            }
        }
        self.text(
            &self.shorten(label, &self.body, area.right - area.left - 24.0),
            rect(area.left + 12.0, area.top, area.right - 12.0, area.bottom),
            &self.body.clone(),
            if dangerous {
                self.palette.destructive
            } else {
                self.palette.primary
            },
        )?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }
}
