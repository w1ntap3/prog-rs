//! Root view: a left menu of sections and a content area for the selection.
//!
//! The menu is driven by the section list from `crate::config`. The
//! `settings` section renders one settings block per configured section
//! (finance conversion rates for now), plus config/data export-import.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, App, Bounds, ClickEvent, Context, Entity, FocusHandle, FontWeight,
    KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, PathPromptOptions,
    Pixels, Point, ScrollHandle, Window, actions, canvas, div, fill, point, px, rgb, size, svg,
};

use crate::{assets, config, currency, finances, paths, theme};

/// Section label that renders the settings panel instead of a placeholder.
pub const SETTINGS_SECTION: &str = "settings";

/// Section label that renders the finances dashboard.
pub const FINANCES_SECTION: &str = "finances";

actions!(prog_rs, [Quit]);

/// App-wide keybindings, bound by the shell in `main`.
pub fn key_bindings() -> [KeyBinding; 1] {
    [KeyBinding::new("escape", Quit, None)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsField {
    EurRate,
    AznRate,
}

impl SettingsField {
    fn currency(self) -> currency::Currency {
        match self {
            SettingsField::EurRate => currency::Currency::Eur,
            SettingsField::AznRate => currency::Currency::Azn,
        }
    }

    fn next(self) -> Self {
        match self {
            SettingsField::EurRate => SettingsField::AznRate,
            SettingsField::AznRate => SettingsField::EurRate,
        }
    }
}

pub struct AppRoot {
    config: config::Config,
    selected: usize,
    status: Option<String>,
    finances: Entity<finances::Finances>,
    focus_handle: FocusHandle,
    eur_rate: String,
    azn_rate: String,
    focus_eur: FocusHandle,
    focus_azn: FocusHandle,
    content_scroll: ScrollHandle,
    scrollbar_hover: bool,
    scrollbar_drag: Option<f32>,
}

impl AppRoot {
    pub fn new(config: config::Config, cx: &mut Context<Self>) -> Self {
        debug_assert!(!config.sections.is_empty(), "config guarantees non-empty sections");
        let rates = config.rates;
        Self {
            eur_rate: currency::rate_string(rates.eur),
            azn_rate: currency::rate_string(rates.azn),
            finances: cx.new(move |cx| finances::Finances::new(rates, cx)),
            config,
            selected: 0,
            status: None,
            focus_handle: cx.focus_handle(),
            focus_eur: cx.focus_handle(),
            focus_azn: cx.focus_handle(),
            content_scroll: ScrollHandle::new(),
            scrollbar_hover: false,
            scrollbar_drag: None,
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

    /// Push the live config's rates into the finance view.
    fn sync_rates(&mut self, cx: &mut Context<Self>) {
        let rates = self.config.rates;
        self.eur_rate = currency::rate_string(rates.eur);
        self.azn_rate = currency::rate_string(rates.azn);
        self.finances
            .update(cx, |finances, cx| finances.set_rates(rates, cx));
    }

    fn apply_rate(&mut self, field: SettingsField, cx: &mut Context<Self>) {
        let cur = field.currency();
        let raw = match field {
            SettingsField::EurRate => self.eur_rate.clone(),
            SettingsField::AznRate => self.azn_rate.clone(),
        };
        let Some(rate) = currency::parse_decimal(&raw) else {
            self.status = Some(format!(
                "enter how many CZK 1 {} is worth",
                cur.label()
            ));
            cx.notify();
            return;
        };
        if rate < 0.0 {
            self.status = Some("conversion rate must not be negative".into());
            cx.notify();
            return;
        }

        let mut rates = self.config.rates;
        rates.set(cur, rate);
        self.config.rates = rates;
        match field {
            SettingsField::EurRate => self.eur_rate = currency::rate_string(rate),
            SettingsField::AznRate => self.azn_rate = currency::rate_string(rate),
        }
        self.status = Some(match self.config.save() {
            Ok(()) => format!(
                "1 {} = {} CZK",
                cur.label(),
                currency::trim_decimal(rate)
            ),
            Err(error) => format!("save failed: {error:#}"),
        });
        self.finances
            .update(cx, |finances, cx| finances.set_rates(rates, cx));
        cx.notify();
    }

    fn settings_key(
        &mut self,
        field: SettingsField,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();

        if key == "tab" {
            self.focus_rate(field.next(), window, cx);
            return;
        }
        if key == "enter" {
            self.apply_rate(field, cx);
            return;
        }

        let value = match field {
            SettingsField::EurRate => &mut self.eur_rate,
            SettingsField::AznRate => &mut self.azn_rate,
        };
        if key == "backspace" {
            if modifiers.control || modifiers.platform {
                value.clear();
            } else {
                value.pop();
            }
            cx.notify();
            return;
        }
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        let Some(chars) = event.keystroke.key_char.as_deref() else {
            return;
        };
        if !chars.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
            return;
        }
        let adds_separator = chars.chars().any(|c| c == '.' || c == ',');
        if adds_separator && value.contains(['.', ',']) {
            return;
        }
        value.push_str(chars);
        cx.notify();
    }

    fn focus_rate(&mut self, field: SettingsField, window: &mut Window, cx: &mut Context<Self>) {
        let focus = match field {
            SettingsField::EurRate => &self.focus_eur,
            SettingsField::AznRate => &self.focus_azn,
        };
        focus.focus(window, cx);
        cx.notify();
    }

    /// Ask the OS where to write the config export, then export there.
    fn export_config(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_new_path(&paths::config_home(), Some("prog-rs-config.toml"));
        cx.spawn(async move |this, cx| {
            let Ok(prompt) = prompt.await else { return };
            let Ok(Some(path)) = prompt else { return };
            this.update(cx, |this, cx| {
                this.status = Some(match this.config.export(&path) {
                    Ok(()) => format!("exported config → {}", path.display()),
                    Err(error) => format!("config export failed: {error:#}"),
                });
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
            let (status, imported) = match prompt {
                Ok(Some(mut picked)) if !picked.is_empty() => {
                    let path = picked.remove(0);
                    match config::Config::import(&path) {
                        Ok(config) => (
                            format!("imported config ({} sections)", config.sections.len()),
                            Some(config),
                        ),
                        Err(error) => (format!("config import failed: {error:#}"), None),
                    }
                }
                Ok(_) => return, // cancelled
                Err(error) => (format!("file picker failed: {error:#}"), None),
            };
            this.update(cx, |this, cx| {
                if let Some(config) = imported {
                    this.config = config;
                    this.selected = 0;
                    this.sync_rates(cx);
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
            .text_color(rgb(theme::color::SEA_GREEN))
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
                    el.cursor_pointer()
                        .hover(|style| style.text_color(rgb(theme::color::MINT)))
                } else {
                    el.cursor_pointer()
                }
            })
            .on_click(on_click)
            .child(label)
    }

    fn rate_field(
        &self,
        field: SettingsField,
        placeholder: &str,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (focus_handle, value) = match field {
            SettingsField::EurRate => (&self.focus_eur, &self.eur_rate),
            SettingsField::AznRate => (&self.focus_azn, &self.azn_rate),
        };
        let focused = focus_handle.is_focused(window);
        let focus = focus_handle.clone();
        let empty = value.is_empty();
        let shown = if empty { placeholder.to_owned() } else { value.clone() };

        div()
            .id(format!("rate-{field:?}"))
            .w(px(theme::geometry::FIELD_TIME_W))
            .px(px(theme::space::FIELD_X))
            .py(px(theme::space::FIELD_Y))
            .border_b_1()
            .border_color(rgb(if focused {
                theme::color::ACCENT
            } else {
                theme::color::SURFACE
            }))
            .flex()
            .items_center()
            .text_size(px(theme::type_scale::FIELD))
            .text_color(rgb(if empty {
                theme::color::MUTED
            } else {
                theme::color::FOREGROUND
            }))
            .cursor_text()
            .track_focus(&focus)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.focus_rate(field, window, cx)
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.settings_key(field, event, window, cx)
            }))
            .child(div().child(shown))
            .when(focused, |el| {
                el.child(
                    div()
                        .ml(px(theme::space::DOT_GAP))
                        .text_color(rgb(theme::color::ACCENT))
                        .child("▌")
                        .with_animation(
                            format!("settings-caret-{field:?}"),
                            Animation::new(Duration::from_millis(900)).repeat(),
                            |el, t| el.opacity(if t < 0.5 { 1.0 } else { 0.0 }),
                        ),
                )
            })
    }

    /// Settings owned by the finances section.
    fn currency_settings(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme::space::TIGHT))
            .child(Self::group_label("currency"))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(theme::space::TIGHT))
                    .child(Self::rate_label("1 EUR ="))
                    .child(self.rate_field(SettingsField::EurRate, "rate", window, cx))
                    .child(Self::rate_label("CZK"))
                    .child(Self::rate_label("1 AZN ="))
                    .child(self.rate_field(SettingsField::AznRate, "rate", window, cx))
                    .child(Self::rate_label("CZK")),
            )
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child("enter to apply · every finance total is shown in CZK"),
            )
    }

    fn rate_label(text: &'static str) -> impl IntoElement {
        div()
            .pb(px(theme::space::FIELD_Y))
            .text_size(px(theme::type_scale::MINI))
            .text_color(rgb(theme::color::MUTED))
            .child(text)
    }

    /// One settings block per configured section.
    fn section_settings(
        &self,
        section: &str,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_finances = section == FINANCES_SECTION;
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme::space::TIGHT))
            .child(
                div()
                    .text_size(px(theme::type_scale::NAV))
                    .font_weight(FontWeight(theme::weight::NAV_ACTIVE))
                    .text_color(rgb(theme::color::MINT))
                    .child(section.to_owned()),
            )
            .when(is_finances, |el| el.child(self.currency_settings(window, cx)))
            .when(!is_finances, |el| {
                el.child(
                    div()
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::MUTED))
                        .child("no settings for this section yet"),
                )
            })
    }

    fn settings(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme::space::GROUP_Y))
            .children(
                self.config
                    .sections
                    .iter()
                    .filter(|section| section.as_str() != SETTINGS_SECTION)
                    .map(|section| self.section_settings(section, window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(theme::space::TIGHT))
                    .child(Self::group_label("app"))
                    .child(
                        div()
                            .flex()
                            .justify_center()
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
                            ))
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
                        .max_w(px(theme::geometry::SETTINGS_STATUS_W))
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::YELLOW))
                        .child(status),
                )
            })
    }

    fn content(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .h_full()
            .relative()
            .child(
                div()
                    .id("content-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.content_scroll)
                    .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                    .child(self.content_inner(window, cx)),
            )
            .child(self.scrollbar(cx))
    }

    fn content_inner(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let label = self
            .config
            .sections
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or_default();
        let is_settings = label == SETTINGS_SECTION;
        let is_finances = label == FINANCES_SECTION;
        div()
            .w_full()
            .pt(px(theme::space::CONTENT_TOP))
            .px(px(theme::space::CONTENT_X))
            .pb(px(theme::space::CONTENT_TOP))
            .flex()
            .flex_col()
            .gap(px(theme::space::KICKER_GAP))
            .when(is_settings, |el| el.items_center())
            .child(
                div()
                    .text_size(px(theme::type_scale::KICKER))
                    .text_color(rgb(theme::color::SEA_GREEN))
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
                el.child(
                    div()
                        .mt(px(theme::space::TIGHT))
                        .w_full()
                        .child(self.settings(window, cx)),
                )
            })
            .when(is_finances, |el| {
                el.child(
                    div()
                        .mt(px(theme::space::TIGHT))
                        .w_full()
                        .child(self.finances.clone()),
                )
            })
            .when(!is_settings && !is_finances, |el| {
                el.child(
                    div()
                        .mt(px(4.))
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::MUTED))
                        .child("empty"),
                )
            })
    }

    /// Geometry shared by painting and dragging the content scrollbar.
    /// Returns `(top, viewport_height, thumb_height, max_offset)`.
    fn scrollbar_geometry(&self) -> Option<(f32, f32, f32, f32)> {
        let bounds = self.content_scroll.bounds();
        let viewport = bounds.size.height.as_f32();
        let max = self.content_scroll.max_offset().y.as_f32();
        if viewport <= 0.0 || max <= 0.5 {
            return None;
        }
        let thumb = ((viewport / (viewport + max)) * viewport)
            .max(theme::geometry::SCROLLBAR_MIN_H)
            .min(viewport);
        Some((bounds.origin.y.as_f32(), viewport, thumb, max))
    }

    fn scrollbar_down(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((top, viewport, thumb, max)) = self.scrollbar_geometry() else {
            return;
        };
        let progress = (-self.content_scroll.offset().y.as_f32() / max).clamp(0.0, 1.0);
        let thumb_top = top + (viewport - thumb) * progress;
        let y = position.y.as_f32();
        let grab = if y >= thumb_top && y <= thumb_top + thumb {
            y - thumb_top
        } else {
            let target = (y - top - thumb / 2.0).clamp(0.0, viewport - thumb);
            let p = if viewport > thumb {
                target / (viewport - thumb)
            } else {
                0.0
            };
            self.content_scroll
                .set_offset(point(px(0.), px(-max * p)));
            thumb / 2.0
        };
        self.scrollbar_drag = Some(grab);
        cx.notify();
    }

    fn scrollbar_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(grab) = self.scrollbar_drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.scrollbar_drag = None;
            cx.notify();
            return;
        }
        let Some((top, viewport, thumb, max)) = self.scrollbar_geometry() else {
            return;
        };
        let target = (event.position.y.as_f32() - top - grab).clamp(0.0, viewport - thumb);
        let p = if viewport > thumb {
            target / (viewport - thumb)
        } else {
            0.0
        };
        self.content_scroll
            .set_offset(point(px(0.), px(-max * p)));
        cx.notify();
    }

    fn scrollbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let handle = self.content_scroll.clone();
        let hovered = self.scrollbar_hover;
        let dragging = self.scrollbar_drag.is_some();
        div()
            .id("content-scrollbar")
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .w(px(theme::geometry::SCROLLBAR_HIT_W))
            .block_mouse_except_scroll()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if this.scrollbar_hover != *hovered {
                    this.scrollbar_hover = *hovered;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.scrollbar_down(event.position, cx)
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.scrollbar_move(event, cx)
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.scrollbar_drag = None;
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |bounds: Bounds<Pixels>, _, window, _| {
                        paint_scrollbar(bounds, &handle, hovered, dragging, window);
                    },
                )
                .size_full(),
            )
    }

}

fn paint_scrollbar(
    bounds: Bounds<Pixels>,
    handle: &ScrollHandle,
    hovered: bool,
    dragging: bool,
    window: &mut Window,
) {
    let viewport = bounds.size.height.as_f32();
    let max = handle.max_offset().y.as_f32();
    if viewport <= 0.0 || max <= 0.5 {
        return;
    }

    let thumb_height = ((viewport / (viewport + max)) * viewport)
        .max(theme::geometry::SCROLLBAR_MIN_H)
        .min(viewport);
    let progress = (-handle.offset().y.as_f32() / max).clamp(0.0, 1.0);
    let top = bounds.origin.y.as_f32() + (viewport - thumb_height) * progress;
    let width = if hovered || dragging {
        theme::geometry::SCROLLBAR_HOVER_W
    } else {
        theme::geometry::SCROLLBAR_IDLE_W
    };
    let x = bounds.origin.x.as_f32() + bounds.size.width.as_f32()
        - width
        - theme::geometry::SCROLLBAR_MARGIN;
    let thumb = Bounds::new(
        point(px(x), px(top)),
        size(px(width), px(thumb_height)),
    );
    let color = if dragging {
        theme::color::ACCENT
    } else if hovered {
        theme::color::MUTED
    } else {
        theme::color::SURFACE
    };
    window.paint_quad(fill(thumb, rgb(color)));
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .id("sidebar-nav")
                            .pt(px(theme::space::SIDEBAR_TOP))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(theme::space::ITEM_GAP))
                            .children(
                                self.config
                                    .sections
                                    .iter()
                                    .enumerate()
                                    .map(|(index, label)| self.nav_item(index, label, cx)),
                            ),
                    ),
            )
            .child(self.content(window, cx))
    }
}
