//! Windows: controllers XInput can't see (PlayStation, Switch, Steam
//! Controller, generic DirectInput pads), read with SDL3 (`SDL3.dll` next to
//! the app) while a game with our steamclient runs, and published to the game
//! through shared memory. The steamclient shows them to the game as Xbox
//! pads, through Steam Input and through XInput, like Steam does.
//!
//! Xbox pads are left alone: games read them through XInput themselves.

#[path = "../../steamclient/src/pads_shm.rs"]
#[allow(dead_code)]
mod shm;

pub use shm::ENV_PADS;

/// A game with our steamclient is starting: read pads until [`stop`].
/// Returns the shared memory's name for the game's environment.
#[cfg(not(windows))]
pub fn start() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn stop() {}

#[cfg(windows)]
pub use imp::{start, stop};

#[cfg(windows)]
mod imp {
    use super::shm::{self, Shared, State};
    use std::ffi::{CStr, c_char, c_void};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn CreateFileMappingW(file: isize, attrs: *const c_void, protect: u32, size_hi: u32, size_lo: u32, name: *const u16) -> *mut c_void;
        fn MapViewOfFile(map: *mut c_void, access: u32, off_hi: u32, off_lo: u32, bytes: usize) -> *mut c_void;
    }
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
    const PAGE_READWRITE: u32 = 0x4;
    const FILE_MAP_ALL_ACCESS: u32 = 0xF001F;
    const INIT_GAMEPAD: u32 = 0x2000;

    type Gp = *mut c_void;

    /// The SDL3 functions we use (SDLCALL is the C convention).
    struct Sdl {
        set_hint: unsafe extern "C" fn(*const c_char, *const c_char) -> bool,
        init: unsafe extern "C" fn(u32) -> bool,
        quit_subsystem: unsafe extern "C" fn(u32),
        get_error: unsafe extern "C" fn() -> *const c_char,
        free: unsafe extern "C" fn(*mut c_void),
        set_joystick_events: unsafe extern "C" fn(bool),
        set_gamepad_events: unsafe extern "C" fn(bool),
        get_gamepads: unsafe extern "C" fn(*mut i32) -> *mut u32,
        type_for_id: unsafe extern "C" fn(u32) -> i32,
        vendor_for_id: unsafe extern "C" fn(u32) -> u16,
        product_for_id: unsafe extern "C" fn(u32) -> u16,
        name_for_id: unsafe extern "C" fn(u32) -> *const c_char,
        open: unsafe extern "C" fn(u32) -> Gp,
        close: unsafe extern "C" fn(Gp),
        connected: unsafe extern "C" fn(Gp) -> bool,
        update: unsafe extern "C" fn(),
        button: unsafe extern "C" fn(Gp, i32) -> bool,
        axis: unsafe extern "C" fn(Gp, i32) -> i16,
        rumble: unsafe extern "C" fn(Gp, u16, u16, u32) -> bool,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn load_sdl() -> Option<Sdl> {
        let path = std::env::current_exe().ok()?.parent()?.join("SDL3.dll");
        if !path.is_file() {
            tracing::info!("controllers: no SDL3.dll next to the app; only Xbox pads reach games");
            return None;
        }
        let m = unsafe { LoadLibraryExW(wide(&path.to_string_lossy()).as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if m.is_null() {
            tracing::warn!("controllers: couldn't load {}", path.display());
            return None;
        }
        macro_rules! f {
            ($name:literal) => {{
                let p = unsafe { GetProcAddress(m, concat!($name, "\0").as_ptr().cast()) };
                if p.is_null() {
                    tracing::warn!("controllers: SDL3.dll has no {}", $name);
                    return None;
                }
                unsafe { std::mem::transmute::<*mut c_void, _>(p) }
            }};
        }
        Some(Sdl {
            set_hint: f!("SDL_SetHint"),
            init: f!("SDL_Init"),
            quit_subsystem: f!("SDL_QuitSubSystem"),
            get_error: f!("SDL_GetError"),
            free: f!("SDL_free"),
            set_joystick_events: f!("SDL_SetJoystickEventsEnabled"),
            set_gamepad_events: f!("SDL_SetGamepadEventsEnabled"),
            get_gamepads: f!("SDL_GetGamepads"),
            type_for_id: f!("SDL_GetGamepadTypeForID"),
            vendor_for_id: f!("SDL_GetGamepadVendorForID"),
            product_for_id: f!("SDL_GetGamepadProductForID"),
            name_for_id: f!("SDL_GetGamepadNameForID"),
            open: f!("SDL_OpenGamepad"),
            close: f!("SDL_CloseGamepad"),
            connected: f!("SDL_GamepadConnected"),
            update: f!("SDL_UpdateGamepads"),
            button: f!("SDL_GetGamepadButton"),
            axis: f!("SDL_GetGamepadAxis"),
            rumble: f!("SDL_RumbleGamepad"),
        })
    }

    fn sdl() -> Option<&'static Sdl> {
        static SDL: OnceLock<Option<Sdl>> = OnceLock::new();
        SDL.get_or_init(load_sdl).as_ref()
    }

    /// The shared memory, created once for the app's lifetime.
    fn mapping() -> Option<(&'static Shared, &'static str)> {
        static MAP: OnceLock<Option<(usize, String)>> = OnceLock::new();
        let (addr, name) = MAP
            .get_or_init(|| {
                let name = format!("Local\\sir-pads-{}", std::process::id());
                let size = std::mem::size_of::<Shared>();
                let h = unsafe { CreateFileMappingW(-1, std::ptr::null(), PAGE_READWRITE, 0, size as u32, wide(&name).as_ptr()) };
                if h.is_null() {
                    tracing::warn!("controllers: couldn't create the shared memory");
                    return None;
                }
                let view = unsafe { MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, size) };
                if view.is_null() {
                    return None;
                }
                // Zeroed by the system; the handle stays open for good.
                let shared = unsafe { &*(view as *const Shared) };
                shared.magic.store(shm::MAGIC, Ordering::Release);
                Some((view as usize, name))
            })
            .as_ref()?;
        Some((unsafe { &*(*addr as *const Shared) }, name.as_str()))
    }

    /// Games using the pads right now.
    static USERS: AtomicUsize = AtomicUsize::new(0);
    /// The reader thread is running.
    static RUNNING: Mutex<bool> = Mutex::new(false);

    pub fn start() -> Option<String> {
        let sdl = sdl()?;
        let (shared, name) = mapping()?;
        let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        USERS.fetch_add(1, Ordering::AcqRel);
        if !*running {
            if std::thread::Builder::new().name("pads".into()).spawn(move || read_pads(sdl, shared)).is_err() {
                USERS.fetch_sub(1, Ordering::AcqRel);
                return None;
            }
            *running = true;
        }
        Some(name.to_string())
    }

    pub fn stop() {
        let mut n = USERS.load(Ordering::Acquire);
        while n > 0 {
            match USERS.compare_exchange_weak(n, n - 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(now) => n = now,
            }
        }
    }

    struct Open {
        id: u32,
        gp: Gp,
        last: State,
        rumble: (u16, u16),
        rumbled: Instant,
    }

    /// SDL_GamepadButton → XInput button bit, by position (SDL's south
    /// button is Xbox A, like Steam maps it).
    const BUTTONS: [(i32, u16); 15] = [
        (0, 0x1000),     // south → A
        (1, 0x2000),     // east → B
        (2, 0x4000),     // west → X
        (3, 0x8000),     // north → Y
        (4, 0x0020),     // back
        (5, shm::GUIDE), // guide
        (6, 0x0010),     // start
        (7, 0x0040),     // left stick
        (8, 0x0080),     // right stick
        (9, 0x0100),     // left shoulder
        (10, 0x0200),    // right shoulder
        (11, 0x0001),    // d-pad up
        (12, 0x0002),    // d-pad down
        (13, 0x0004),    // d-pad left
        (14, 0x0008),    // d-pad right
    ];

    /// SDL's Y axes point down, XInput's up.
    fn flip(v: i16) -> i16 {
        (-(v as i32)).clamp(-32768, 32767) as i16
    }

    fn trigger(v: i16) -> u8 {
        (v.max(0) as u32 * 255 / 32767) as u8
    }

    fn state(sdl: &Sdl, gp: Gp) -> State {
        let b = |i| unsafe { (sdl.button)(gp, i) };
        let a = |i| unsafe { (sdl.axis)(gp, i) };
        State {
            packet: 0,
            buttons: BUTTONS.iter().filter(|(i, _)| b(*i)).fold(0, |acc, (_, bit)| acc | bit),
            lx: a(0),
            ly: flip(a(1)),
            rx: a(2),
            ry: flip(a(3)),
            lt: trigger(a(4)),
            rt: trigger(a(5)),
        }
    }

    /// Xbox pads are XInput's; the game reads those itself.
    fn is_xbox(sdl: &Sdl, id: u32) -> bool {
        let kind = unsafe { (sdl.type_for_id)(id) };
        kind == 2 || kind == 3 || unsafe { (sdl.vendor_for_id)(id) } == 0x045e
    }

    fn name(sdl: &Sdl, id: u32) -> String {
        let p = unsafe { (sdl.name_for_id)(id) };
        if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
    }

    fn read_pads(sdl: &'static Sdl, shared: &'static Shared) {
        unsafe {
            for (k, v) in [(c"SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", c"1"), (c"SDL_JOYSTICK_THREAD", c"1")] {
                (sdl.set_hint)(k.as_ptr(), v.as_ptr());
            }
        }
        if !unsafe { (sdl.init)(INIT_GAMEPAD) } {
            let e = unsafe { CStr::from_ptr((sdl.get_error)()) }.to_string_lossy().into_owned();
            tracing::warn!("controllers: SDL couldn't start: {e}");
            *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = false;
            return;
        }
        // Nobody reads SDL's event queue: keep it from filling up.
        unsafe {
            (sdl.set_joystick_events)(false);
            (sdl.set_gamepad_events)(false);
        }
        tracing::info!("controllers: reading PlayStation/Switch/other pads for games");
        let mut pads: [Option<Open>; shm::SLOTS] = Default::default();
        let mut scanned: Option<Instant> = None;
        // Every pad SDL reports, logged once (what it is, and if games get it).
        let mut seen: std::collections::HashSet<u32> = Default::default();
        let mut last_count = None;
        loop {
            if USERS.load(Ordering::Acquire) == 0 {
                let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
                if USERS.load(Ordering::Acquire) == 0 {
                    for (slot, p) in pads.iter_mut().enumerate() {
                        if let Some(o) = p.take() {
                            unsafe { (sdl.close)(o.gp) };
                        }
                        shared.slots[slot].write(None);
                    }
                    unsafe { (sdl.quit_subsystem)(INIT_GAMEPAD) };
                    *running = false;
                    tracing::info!("controllers: stopped reading pads");
                    return;
                }
            }
            unsafe { (sdl.update)() };
            if scanned.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
                scanned = Some(Instant::now());
                let mut n = 0;
                let ids = unsafe { (sdl.get_gamepads)(&mut n) };
                if !ids.is_null() {
                    let list: Vec<u32> = unsafe { std::slice::from_raw_parts(ids, n.max(0) as usize) }.to_vec();
                    unsafe { (sdl.free)(ids.cast()) };
                    if last_count != Some(list.len()) {
                        tracing::info!("controllers: SDL sees {} pad(s)", list.len());
                        last_count = Some(list.len());
                    }
                    for &id in &list {
                        if seen.insert(id) {
                            let (kind, vendor, product) = unsafe { ((sdl.type_for_id)(id), (sdl.vendor_for_id)(id), (sdl.product_for_id)(id)) };
                            let what = if is_xbox(sdl, id) { "Xbox, left to the game's XInput" } else { "shared with games" };
                            tracing::info!("controllers: {} (SDL type {kind}, USB {vendor:04x}:{product:04x}): {what}", name(sdl, id));
                        }
                    }
                    for id in list {
                        if pads.iter().flatten().any(|o| o.id == id) || is_xbox(sdl, id) {
                            continue;
                        }
                        let Some(slot) = pads.iter().position(Option::is_none) else { break };
                        let gp = unsafe { (sdl.open)(id) };
                        if gp.is_null() {
                            continue;
                        }
                        tracing::info!("controllers: {} → game pad {}", name(sdl, id), slot + 1);
                        shared.slots[slot].set_rumble(0, 0);
                        pads[slot] = Some(Open { id, gp, last: State::default(), rumble: (0, 0), rumbled: Instant::now() });
                    }
                }
            }
            for (slot, p) in pads.iter_mut().enumerate() {
                let Some(o) = p else { continue };
                if !unsafe { (sdl.connected)(o.gp) } {
                    tracing::info!("controllers: game pad {} disconnected", slot + 1);
                    unsafe { (sdl.close)(o.gp) };
                    *p = None;
                    shared.slots[slot].write(None);
                    continue;
                }
                let mut s = state(sdl, o.gp);
                s.packet = o.last.packet;
                if s != o.last {
                    s.packet = s.packet.wrapping_add(1);
                }
                o.last = s;
                shared.slots[slot].write(Some(s));
                // XInput rumble lasts until changed; SDL's needs a duration.
                let want = shared.slots[slot].rumble();
                if want != o.rumble || (want != (0, 0) && o.rumbled.elapsed() > Duration::from_secs(20)) {
                    let ms = if want == (0, 0) { 0 } else { 30_000 };
                    unsafe { (sdl.rumble)(o.gp, want.0, want.1, ms) };
                    o.rumble = want;
                    o.rumbled = Instant::now();
                }
            }
            shared.heartbeat.store(shm::now_ms(), Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Needs SDL3.dll next to the test binary (skipped without it).
        #[test]
        fn reads_pads_until_stopped() {
            let dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
            if !dir.join("SDL3.dll").is_file() {
                eprintln!("no SDL3.dll next to the test: skipped");
                return;
            }
            let wait = |what: &str, f: &dyn Fn() -> bool| {
                let t = Instant::now();
                while !f() {
                    assert!(t.elapsed() < Duration::from_secs(10), "{what}");
                    std::thread::sleep(Duration::from_millis(20));
                }
            };
            let name = start().expect("pads start");
            assert!(name.starts_with("Local\\sir-pads-"));
            let (shared, _) = mapping().unwrap();
            wait("the reader keeps the heartbeat", &|| shared.live());
            let pads: Vec<_> = shared.slots.iter().map(|s| s.read()).collect();
            eprintln!("pads seen: {pads:?}");
            stop();
            wait("the reader stops", &|| !*RUNNING.lock().unwrap());
            // And starts again for the next game.
            start().expect("pads start again");
            wait("the reader restarts", &|| *RUNNING.lock().unwrap());
            stop();
            wait("the reader stops again", &|| !*RUNNING.lock().unwrap());
        }
    }
}
