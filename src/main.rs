mod appeal;
mod app;
mod assets;
mod config;
mod currency;
mod data;
mod datetime_picker;
mod finances;
mod modal;
mod paths;
mod startup;
mod study;
mod theme;

use gpui::{
    App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui::prelude::*;

fn main() {
    startup::prepare();

    // A broken config must not brick the app: report and fall back to defaults.
    let config = config::Config::load_or_create().unwrap_or_else(|error| {
        eprintln!("prog-rs: {error:#} (using default config)");
        config::Config::default()
    });

    gpui_platform::application()
        .with_assets(assets::EmbeddedAssets)
        .run(move |cx: &mut App| {
            cx.bind_keys(app::key_bindings());
            let restore_size = size(
                px(theme::geometry::RESTORE_WIDTH),
                px(theme::geometry::RESTORE_HEIGHT),
            );
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Maximized(Bounds::centered(
                        None,
                        restore_size,
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("prog-rs".into()),
                        ..Default::default()
                    }),
                    // Must match the desktop file basename so KWin associates
                    // the window with prog-rs.desktop (icon in task switcher).
                    app_id: Some("prog-rs".into()),
                    ..Default::default()
                },
                |window, cx| {
                    let root = cx.new(|cx| app::AppRoot::new(config.clone(), cx));
                    root.update(cx, |root, cx| root.focus(window, cx));
                    root
                },
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
