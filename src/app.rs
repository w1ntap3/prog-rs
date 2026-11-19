//! Root view: a left menu of sections and a content area for the selection.
//!
//! The menu is driven by the section list from `crate::config`.

use gpui::{Context, FocusHandle, FontWeight, KeyBinding, Window, actions, div, px, rgb, svg};
use gpui::prelude::*;

use crate::{assets, theme};

actions!(prog_rs, [Quit]);

/// App-wide keybindings, bound by the shell in `main`.
pub fn key_bindings() -> [KeyBinding; 1] {
    [KeyBinding::new("escape", Quit, None)]
}

pub struct AppRoot {
    sections: Vec<String>,
    selected: usize,
    focus_handle: FocusHandle,
}

impl AppRoot {
    pub fn new(sections: Vec<String>, cx: &mut Context<Self>) -> Self {
        debug_assert!(!sections.is_empty(), "config guarantees non-empty sections");
        Self {
            sections,
            selected: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
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
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = index;
                cx.notify();
            }))
            .child(label.to_owned())
    }

    fn content(&self) -> impl IntoElement {
        let label = self
            .sections
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or_default();
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
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(theme::type_scale::HINT))
                    .text_color(rgb(theme::color::MUTED))
                    .child("empty"),
            )
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
            .child(self.content())
    }
}
