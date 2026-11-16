// Hide the console window in release builds on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod builtin;
mod chrome;
mod demo;
mod friends_window;
mod image_cache;
mod ingame;
mod launch_popup;
mod links;
mod login;
mod offline_ui;
mod overlay;
mod shell;
mod tray;
#[cfg(target_os = "linux")]
mod tray_linux;
mod updater;
#[cfg(target_os = "linux")]
mod wayland_activate;

fn main() -> eframe::Result {
    // A `steam://` link (or a plain second launch) goes to the running copy.
    let link = std::env::args().skip(1).find(|a| a.starts_with("steam://"));
    // `--separate` runs a second copy (for previews and testing).
    let separate = std::env::args().any(|a| a == "--separate");
    if !separate && links::forward_to_running(link.as_deref().unwrap_or("show")) {
        return Ok(());
    }
    // An update downloaded last time installs now, before the app opens.
    if !separate && updater::apply_pending_on_start() {
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,eframe=warn,egui_glow=warn".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(i18n::BRAND)
            // Starts at sign-in size; the app resizes itself after sign-in.
            .with_inner_size([560.0, 400.0])
            // Our own title bar (see chrome.rs) instead of the Windows one.
            .with_decorations(false)
            .with_icon(std::sync::Arc::new(ui::brand::window_icon(256))),
        // The app keeps the main window's geometry itself (app.rs): eframe's
        // copy would reopen the small sign-in window at the main size.
        persist_window: false,
        ..Default::default()
    };
    // "SteamInRust" names the settings folder (and Wayland app id): kept so
    // existing settings carry over to Aurora.
    ui::native::run("SteamInRust", options, Box::new(move |cc| Ok(Box::new(app::SteamApp::new(cc, link)))))
}
