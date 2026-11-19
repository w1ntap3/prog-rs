//! Root view: a left menu of sections and a content area for the selection.
//!
//! The menu is driven by the section list from `crate::config`. The
//! `settings` section is special: it renders the config/data export-import
//! panel (data actions are stubs until the data store gains a real format).

use gpui::{
    App, ClickEvent, Context, FocusHandle, FontWeight, KeyBinding, PathPromptOptions, Window,
    actions, div, px, rgb, svg,
};
use gpui::prelude::*;

use crate::{assets, config, paths, theme};

/// Section label that renders the settings panel instead of a placeholder.
pub const SETTINGS_SECTION: &str = "settings";

actions!(prog_rs, [Quit]);

/// App-wide keybindings, bound by the shell in `main`.
pub fn key_bindings() -> [KeyBinding; 1] {
    [KeyBinding::new("escape", Quit, None)]
}

pub struct AppRoot {
    sections: Vec<String>,
    selected: usize,
    status: Option<String>,
    focus_handle: FocusHandle,
}

impl AppRoot {
    pub fn new(sections: Vec<String>, cx: &mut Context<Self>) -> Self {
        debug_assert!(!sections.is_empty(), "config guarantees non-empty sections");
        Self {
            sections,
            selected: 0,
            status: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        self.status = None;
        cx.notify();
    }

    /// Ask the OS where to write the config export, then export there.
    fn export_config(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_new_path(&paths::config_home(), Some("prog-rs-config.toml"));
        cx.spawn(async move |this, cx| {
            let Ok(prompt) = prompt.await else { return };
            let status = match prompt {
                Ok(Some(path)) => {
                    match config::Config::load_or_create().and_then(|c| c.export(&path)) {
                        Ok(()) => format!("exported config → {}", path.display()),
                        Err(error) => format!("config export failed: {error:#}"),
                    }
                }
                Ok(None) => return, // cancelled
                Err(error) => format!("file picker failed: {error:#}"),
            };
            this.update(cx, |this, cx| {
                this.status = Some(status);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Ask the OS for a config file, validate it, and make it the live config.
    fn import_config(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import prog-rs config".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(prompt) = prompt.await else { return };
            let (status, sections) = match prompt {
                Ok(Some(mut picked)) if !picked.is_empty() => {
                    let path = picked.remove(0);
                    match config::Config::import(&path) {
                        Ok(config) => (
                            format!("imported config ({} sections)", config.sections.len()),
                            Some(config.sections),
                        ),
                        Err(error) => (format!("config import failed: {error:#}"), None),
                    }
                }
                Ok(_) => return, // cancelled
                Err(error) => (format!("file picker failed: {error:#}"), None),
            };
            this.update(cx, |this, cx| {
                if let Some(sections) = sections {
                    this.sections = sections;
                    this.selected = 0;
                }
                this.status = Some(status);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn stub_notice(&mut self, cx: &mut Context<Self>) {
        self.status = Some("data export/import is not implemented yet".to_owned());
        cx.notify();
    }

    fn brand(&self) -> impl IntoElement {
        div()
            .h(px(theme::geometry::BRAND_HEIGHT))
            .px(px(theme::space::ITEM_X))
            .flex()
            .items_center()
            .gap(px(theme::space::TIGHT))
            .child(svg().size(px(theme::geometry::LOGO)).path(assets::LOGO))
            .child(
                div()
                    .flex()
                    .text_size(px(theme::type_scale::BRAND))
                    .font_weight(FontWeight(theme::weight::BRAND))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .child("prog-")
                    .child(
                        div()
                            .text_color(rgb(theme::color::PURPLE))
                            .child("rs"),
                    ),
            )
    }

    fn nav_item(&self, index: usize, label: &str, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.selected == index;
        div()
            .id(format!("nav-{index}"))
            .pr(px(theme::space::ITEM_X))
            // Compensate the selection border so labels stay aligned.
            .pl(px(if selected {
                theme::space::ITEM_X - theme::geometry::RULE_H
            } else {
                theme::space::ITEM_X
            }))
            .py(px(theme::space::ITEM_Y))
            .cursor_pointer()
            .text_size(px(theme::type_scale::NAV))
            .font_weight(FontWeight(if selected {
                theme::weight::NAV_ACTIVE
            } else {
                theme::weight::NAV
            }))
            .text_color(if selected {
                rgb(theme::color::ACCENT)
            } else {
                rgb(theme::color::MUTED)
            })
            .when(selected, |el| {
                el.bg(rgb(theme::color::SURFACE))
                    .border_l_2()
                    .border_color(rgb(theme::color::ACCENT))
            })
            .when(!selected, |el| {
                el.hover(|style| {
                    style
                        .bg(rgb(theme::color::SURFACE))
                        .text_color(rgb(theme::color::FOREGROUND))
                })
            })
            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
            .child(label.to_owned())
    }

    fn group_label(text: &'static str) -> impl IntoElement {
        div()
            .text_size(px(theme::type_scale::KICKER))
            .text_color(rgb(theme::color::MUTED))
            .child(text.to_uppercase())
    }

    fn button(
        label: &'static str,
        enabled: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(format!("btn-{label}"))
            .px(px(theme::space::ITEM_X))
            .py(px(theme::space::ITEM_Y - 2.))
            .bg(rgb(theme::color::SURFACE))
            .text_size(px(theme::type_scale::NAV))
            .text_color(if enabled {
                rgb(theme::color::FOREGROUND)
            } else {
                rgb(theme::color::MUTED)
            })
            .map(|el| {
                if enabled {
                    el.cursor_pointer().hover(|style| {
                        style.text_color(rgb(theme::color::ACCENT))
                    })
                } else {
                    el.cursor_pointer()
                }
            })
            .on_click(on_click)
            .child(label)
    }

    fn settings(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(theme::space::GROUP_Y))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::TIGHT))
                    .child(Self::group_label("config"))
                    .child(
                        div()
                            .flex()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::button(
                                "export config",
                                true,
                                cx.listener(|this, _, _, cx| this.export_config(cx)),
                            ))
                            .child(Self::button(
                                "import config",
                                true,
                                cx.listener(|this, _, _, cx| this.import_config(cx)),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::TIGHT))
                    .child(Self::group_label("data"))
                    .child(
                        div()
                            .flex()
                            .gap(px(theme::space::TIGHT))
                            .child(Self::button(
                                "export data",
                                false,
                                cx.listener(|this, _, _, cx| this.stub_notice(cx)),
                            ))
                            .child(Self::button(
                                "import data",
                                false,
                                cx.listener(|this, _, _, cx| this.stub_notice(cx)),
                            )),
                    ),
            )
            .when_some(self.status.clone(), |el, status| {
                el.child(
                    div()
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::ACCENT))
                        .child(status),
                )
            })
    }

    fn content(&self, cx: &Context<Self>) -> impl IntoElement {
        let label = self
            .sections
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or_default();
        let is_settings = label == SETTINGS_SECTION;
        div()
            .flex_1()
            .h_full()
            .pt(px(theme::space::CONTENT_TOP))
            .px(px(theme::space::CONTENT_X))
            .flex()
            .flex_col()
            .gap(px(theme::space::KICKER_GAP))
            .child(
                div()
                    .text_size(px(theme::type_scale::KICKER))
                    .text_color(rgb(theme::color::MUTED))
                    .child(format!("prog-rs / {label}").to_uppercase()),
            )
            .child(
                div()
                    .text_size(px(theme::type_scale::TITLE))
                    .font_weight(FontWeight(theme::weight::TITLE))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .child(label.to_owned()),
            )
            .child(
                div()
                    .mt(px(theme::space::TIGHT))
                    .w(px(theme::geometry::RULE_W))
                    .h(px(theme::geometry::RULE_H))
                    .bg(rgb(theme::color::ACCENT)),
            )
            .when(is_settings, |el| {
                el.child(div().mt(px(theme::space::TIGHT)).child(self.settings(cx)))
            })
            .when(!is_settings, |el| {
                el.child(
                    div()
                        .mt(px(4.))
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::MUTED))
                        .child("empty"),
                )
            })
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(theme::color::BACKGROUND))
            .text_color(rgb(theme::color::FOREGROUND))
            .font_family(theme::FONT)
            .flex()
            .track_focus(&self.focus_handle)
            .on_action(|_: &Quit, window, _cx| window.remove_window())
            .child(
                div()
                    .w(px(theme::geometry::SIDEBAR_WIDTH))
                    .h_full()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(rgb(theme::color::SURFACE))
                    .child(self.brand())
                    .child(div().w_full().border_b_1().border_color(rgb(theme::color::SURFACE)))
                    .child(
                        div()
                            .pt(px(theme::space::SIDEBAR_TOP))
                            .flex()
                            .flex_col()
                            .gap(px(theme::space::ITEM_GAP))
                            .children(
                                self.sections
                                    .iter()
                                    .enumerate()
                                    .map(|(index, label)| self.nav_item(index, label, cx)),
                            ),
                    ),
            )
            .child(self.content(cx))
    }
}
