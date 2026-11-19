mod gradient;
mod startup;
mod theme;

use gpui::{
    Animation, AnimationExt as _, App, Bounds, ColorSpace, Context, Render, Window, WindowBounds,
    WindowOptions, div, linear_color_stop, linear_gradient, prelude::*, px, rgb, size,
};

/// Root view: a rounded box with a slowly morphing, rotating gradient on a
/// dark background. All tokens come from [`theme`].
struct AppRoot;

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(theme::color::BACKGROUND))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(theme::size::BOX))
                    .rounded(px(theme::size::RADIUS))
                    .with_animation(
                        "gradient-flow",
                        Animation::new(theme::motion::GRADIENT_CYCLE).repeat(),
                        |el, delta| {
                            el.bg(
                                linear_gradient(
                                    delta * 360.0,
                                    linear_color_stop(rgb(gradient::sample(delta)), 0.0),
                                    linear_color_stop(rgb(gradient::sample(delta + 0.5)), 1.0),
                                )
                                .color_space(ColorSpace::Oklab),
                            )
                        },
                    ),
            )
    }
}

fn main() {
    startup::prepare();
    gpui_platform::application().run(|cx: &mut App| {
        let window_size = size(px(theme::size::WINDOW), px(theme::size::WINDOW));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    window_size,
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| AppRoot),
        )
        .unwrap();

        // Quit once the last window is closed.
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        cx.activate(true);
    });
}
