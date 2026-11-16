//! Bringing the main window back on Wayland.
//!
//! winit can't hide, un-minimize or focus a window on Wayland: the compositor
//! only raises a window for an XDG activation token, and the tray host (GNOME's
//! AppIndicator extension, KDE) hands one over with each click. This sends
//! `xdg_activation_v1.activate(token, surface)` for winit's own surface, on
//! winit's Wayland connection but with an event queue of its own, so it works
//! from the tray thread even while the window isn't painting.

use std::ffi::c_void;
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_surface::WlSurface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

pub struct Activator {
    conn: Connection,
    surface: WlSurface,
    activation: XdgActivationV1,
    _queue: EventQueue<State>,
}

pub struct State;

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<XdgActivationV1, ()> for State {
    fn event(_: &mut Self, _: &XdgActivationV1, _: <XdgActivationV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Activator {
    /// The main window's Wayland handles, or `None` on X11 (or when the
    /// compositor has no xdg-activation).
    pub fn new(window: &impl raw_window_handle::HasWindowHandle, display: &impl raw_window_handle::HasDisplayHandle) -> Option<Self> {
        use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
        let RawDisplayHandle::Wayland(d) = display.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Wayland(w) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: both come from the root window, which lives as long as the app.
        unsafe { Self::from_ptrs(d.display.as_ptr(), w.surface.as_ptr()) }.map_err(|e| tracing::warn!("Wayland activation unavailable: {e}")).ok()
    }

    unsafe fn from_ptrs(display: *mut c_void, surface: *mut c_void) -> Result<Self, String> {
        let backend = unsafe { Backend::from_foreign_display(display.cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
        let activation: XdgActivationV1 = globals.bind(&queue.handle(), 1..=1, ()).map_err(|e| e.to_string())?;
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) }.map_err(|e| e.to_string())?;
        let surface = WlSurface::from_id(&conn, id).map_err(|e| e.to_string())?;
        Ok(Self { conn, surface, activation, _queue: queue })
    }

    /// Ask the compositor to raise (and un-minimize) the window.
    pub fn activate(&self, token: &str) {
        self.activation.activate(token.to_owned(), &self.surface);
        if let Err(e) = self.conn.flush() {
            tracing::warn!("Wayland activation failed: {e}");
        }
    }
}
