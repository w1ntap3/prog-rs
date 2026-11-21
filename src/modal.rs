//! App-wide confirmation dialog.
//!
//! A modal is deferred to the top of the window and dims everything behind it,
//! so it works regardless of which tab requested it. Future popups should use
//! this helper so the "dim the rest of the app" behavior stays consistent.

use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, Window, anchored, deferred, div, point, px, rgb,
};

use crate::theme;

pub fn confirm_dialog(
    window: &Window,
    title: &str,
    body: &str,
    confirm_label: &str,
    danger: u32,
    on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let viewport = window.viewport_size();

    deferred(
        anchored()
            .position(point(px(0.), px(0.)))
            .child(
                div()
                    .w(px(viewport.width.as_f32()))
                    .h(px(viewport.height.as_f32()))
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(rgb(theme::color::BACKGROUND).alpha(0.82))
                            .occlude(),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(theme::geometry::MODAL_W))
                                    .p(px(theme::space::GROUP_Y))
                                    .bg(rgb(theme::color::BACKGROUND))
                                    .border_1()
                                    .border_color(rgb(theme::color::SURFACE))
                                    .occlude()
                                    .flex()
                                    .flex_col()
                                    .gap(px(theme::space::TIGHT))
                                    .child(
                                        div()
                                            .text_size(px(theme::type_scale::TITLE))
                                            .font_weight(gpui::FontWeight(
                                                theme::weight::TITLE,
                                            ))
                                            .text_color(rgb(theme::color::FOREGROUND))
                                            .child(title.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(theme::type_scale::HINT))
                                            .text_color(rgb(theme::color::MUTED))
                                            .child(body.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .mt(px(theme::space::TIGHT))
                                            .flex()
                                            .justify_end()
                                            .gap(px(theme::space::TIGHT))
                                            .child(
                                                div()
                                                    .id("modal-cancel")
                                                    .px(px(theme::space::ITEM_X))
                                                    .py(px(theme::space::ITEM_Y - 2.))
                                                    .text_size(px(theme::type_scale::NAV))
                                                    .text_color(rgb(theme::color::MUTED))
                                                    .cursor_pointer()
                                                    .hover(|style| {
                                                        style.text_color(rgb(
                                                            theme::color::FOREGROUND,
                                                        ))
                                                    })
                                                    .on_click(on_cancel)
                                                    .child("cancel"),
                                            )
                                            .child(
                                                div()
                                                    .id("modal-confirm")
                                                    .px(px(theme::space::ITEM_X))
                                                    .py(px(theme::space::ITEM_Y - 2.))
                                                    .bg(rgb(theme::color::SURFACE))
                                                    .text_size(px(theme::type_scale::NAV))
                                                    .text_color(rgb(danger))
                                                    .cursor_pointer()
                                                    .hover(move |style| {
                                                        style.text_color(rgb(danger))
                                                    })
                                                    .on_click(on_confirm)
                                                    .child(confirm_label.to_owned()),
                                            ),
                                    ),
                            ),
                    ),
            ),
    )
    .with_priority(10)
    .into_any_element()
}
