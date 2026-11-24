//! Controller and keyboard input for Big Picture, turned into the same few
//! presses: a direction, A (select), B (back), X, Y, LB/RB (tabs), Start
//! (menu) and Guide.

use egui::{Key, Modifiers};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::focus::Dir;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Press {
    Move(Dir),
    A,
    B,
    X,
    Y,
    Lb,
    Rb,
    Lt,
    Rt,
    /// Right-stick vertical scroll tick: positive scrolls down, negative scrolls up.
    Scroll(i8),
    Start,
    Guide,
    QuickAccess,
}

/// What the last input came from: hints show buttons or keys to match.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Device {
    #[default]
    Keyboard,
    Pad,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum ButtonHintMode {
    #[default]
    Auto,
    Controller,
    Keyboard,
}

impl ButtonHintMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Controller, Self::Keyboard];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => i18n::t!("Auto"),
            Self::Controller => i18n::t!("Controller"),
            Self::Keyboard => i18n::t!("Keyboard"),
        }
    }
}

pub enum PadEvent {
    Press(Press),
    /// How many controllers are connected now.
    Connected(usize),
}

/// Start reading controllers on a background thread. Each press wakes the UI.
pub fn spawn(ctx: egui::Context) -> mpsc::Receiver<PadEvent> {
    let (tx, rx) = mpsc::channel();
    // The standard Windows.Gaming.Input gamepad reading omits Guide.
    // Use the SDL/XInput reader as the single source of Guide presses on Windows.
    #[cfg(windows)]
    {
        let tx = tx.clone();
        let ctx = ctx.clone();
        client::listen_for_controller_guide(move || {
            if tx.send(PadEvent::Press(Press::Guide)).is_err() {
                return false;
            }
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            true
        });
    }
    let spawned = std::thread::Builder::new().name("gamepads".into()).spawn(move || read_pads(tx, ctx));
    if let Err(e) = spawned {
        tracing::warn!("controllers: {e}");
    }
    rx
}

/// The controller settings, read by the thread that reads the pads.
static SWAP_AB: AtomicBool = AtomicBool::new(false);
static REPEAT_LEVEL: AtomicU8 = AtomicU8::new(1);
static STICK_LEVEL: AtomicU8 = AtomicU8::new(1);

/// Apply the controller settings.
pub fn tune(swap_ab: bool, repeat: u8, stick: u8) {
    SWAP_AB.store(swap_ab, Ordering::Relaxed);
    REPEAT_LEVEL.store(repeat, Ordering::Relaxed);
    STICK_LEVEL.store(stick, Ordering::Relaxed);
}

/// A held direction repeats like a held key: once, then after a pause,
/// steadily. (First repeat, then every…) for slow, normal and fast.
fn repeat_times(level: u8) -> (Duration, Duration) {
    let (first, every) = match level {
        0 => (500, 200),
        2 => (280, 70),
        _ => (380, 120),
    };
    (Duration::from_millis(first), Duration::from_millis(every))
}

/// How far the stick goes to start a move, and to end it (with a gap so a
/// stick near the edge doesn't flicker), for low, normal and high sensitivity.
fn stick_edges(level: u8) -> (f32, f32) {
    match level {
        0 => (0.72, 0.5),
        2 => (0.4, 0.25),
        _ => (0.55, 0.35),
    }
}

/// What a face or menu button means. `swap`: for pads with A and B the other
/// way round, the right button confirms and the bottom one goes back.
fn button_press(b: gilrs::Button, swap: bool) -> Option<Press> {
    use gilrs::Button;
    Some(match b {
        Button::South if swap => Press::B,
        Button::East if swap => Press::A,
        Button::South => Press::A,
        Button::East => Press::B,
        Button::West => Press::X,
        Button::North => Press::Y,
        Button::LeftTrigger => Press::Lb,
        Button::RightTrigger => Press::Rb,
        Button::LeftTrigger2 => Press::Lt,
        Button::RightTrigger2 => Press::Rt,
        Button::Start => Press::Start,
        #[cfg(not(windows))]
        Button::Mode => Press::Guide,
        _ => return None,
    })
}

/// On Windows the gamepad library reads every pad 125 times a second, even
/// with none plugged in, which was nearly all of the app's CPU use while
/// idle. So it only runs while a controller is connected: Windows says when
/// one is plugged in.
fn read_pads(tx: mpsc::Sender<PadEvent>, ctx: egui::Context) {
    #[cfg(windows)]
    let (wake_tx, wake_rx) = mpsc::channel::<()>();
    #[cfg(windows)]
    let _watch = plugged::on_added(move || {
        let _ = wake_tx.send(());
    });
    #[cfg(windows)]
    let mut announced = None;
    loop {
        #[cfg(windows)]
        if plugged::count() == 0 {
            if announced != Some(0) {
                announced = Some(0);
                if tx.send(PadEvent::Connected(0)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
            // Sleep until one is plugged in (and look again now and then).
            let _ = wake_rx.recv_timeout(Duration::from_secs(10));
            while wake_rx.try_recv().is_ok() {}
            continue;
        }
        #[cfg(windows)]
        {
            announced = None;
        }
        let gilrs = match gilrs::Gilrs::new() {
            Ok(g) => g,
            Err(gilrs::Error::NotImplemented(g)) => g,
            Err(e) => {
                tracing::warn!("controllers unavailable: {e}");
                return;
            }
        };
        if !read_connected(gilrs, &tx, &ctx) {
            return;
        }
    }
}

/// Windows' own list of game controllers, for the cheap "is one plugged in?".
#[cfg(windows)]
mod plugged {
    use windows::Foundation::EventHandler;
    use windows::Gaming::Input::RawGameController;

    pub fn count() -> u32 {
        RawGameController::RawGameControllers().and_then(|list| list.Size()).unwrap_or(0)
    }

    /// Calls `wake` whenever a controller is plugged in, for as long as the
    /// returned value is kept.
    pub fn on_added(wake: impl Fn() + Send + Sync + 'static) -> Option<Watch> {
        let handler = EventHandler::<RawGameController>::new(move |_, _| {
            wake();
            Ok(())
        });
        RawGameController::RawGameControllerAdded(&handler).ok().map(Watch)
    }

    pub struct Watch(i64);

    impl Drop for Watch {
        fn drop(&mut self) {
            let _ = RawGameController::RemoveRawGameControllerAdded(self.0);
        }
    }
}

#[derive(Default)]
struct HeldDirections {
    dpad: [bool; 4],
    stick: [bool; 4],
    next: [Option<Instant>; 4],
    /// Right stick Y scroll direction (-1 up, 0 neutral, 1 down) and repeat timer.
    scroll_dir: i8,
    next_scroll: Option<Instant>,
    /// Analog trigger latching (for pads that report triggers via axis rather than buttons).
    lt_down: bool,
    rt_down: bool,
    /// Guide button held: Guide + A emits QuickAccess without also emitting Guide or A.
    guide_down: bool,
    guide_chorded: bool,
}

const SCROLL_REPEAT: Duration = Duration::from_millis(65);

impl HeldDirections {
    fn update(&mut self, event: gilrs::EventType) -> Option<Press> {
        use gilrs::EventType;
        match event {
            EventType::ButtonPressed(button, _) => self.update_button(button, true),
            EventType::ButtonReleased(button, _) => self.update_button(button, false),
            EventType::AxisChanged(axis, value, _) => self.update_axis(axis, value),
            _ => None,
        }
    }

    fn update_button(&mut self, button: gilrs::Button, pressed: bool) -> Option<Press> {
        use gilrs::Button;
        let swap = SWAP_AB.load(Ordering::Relaxed);
        if button == Button::Mode {
            if pressed {
                self.guide_down = true;
                self.guide_chorded = false;
                return None;
            }
            let emit_guide = self.guide_down && !self.guide_chorded;
            self.guide_down = false;
            self.guide_chorded = false;
            return emit_guide.then_some(Press::Guide);
        }
        if self.guide_down {
            let is_confirm = button_press(button, swap) == Some(Press::A);
            if is_confirm {
                if pressed {
                    self.guide_chorded = true;
                    return Some(Press::QuickAccess);
                }
                return None;
            }
        }
        let index = match button {
            Button::DPadUp => 0,
            Button::DPadDown => 1,
            Button::DPadLeft => 2,
            Button::DPadRight => 3,
            Button::LeftTrigger2 => {
                let fire = pressed && !self.lt_down;
                self.lt_down = pressed;
                return fire.then_some(Press::Lt);
            }
            Button::RightTrigger2 => {
                let fire = pressed && !self.rt_down;
                self.rt_down = pressed;
                return fire.then_some(Press::Rt);
            }
            other => return pressed.then(|| button_press(other, swap)).flatten(),
        };
        self.dpad[index] = pressed;
        None
    }

    fn update_axis(&mut self, axis: gilrs::Axis, value: f32) -> Option<Press> {
        use gilrs::Axis;
        match axis {
            Axis::LeftStickY | Axis::LeftStickX => {
                let (positive, negative) = if axis == Axis::LeftStickY { (0, 1) } else { (3, 2) };
                let (on, off) = stick_edges(STICK_LEVEL.load(Ordering::Relaxed));
                self.stick[positive] = value > if self.stick[positive] { off } else { on };
                self.stick[negative] = -value > if self.stick[negative] { off } else { on };
            }
            Axis::RightStickY => {
                let prev = self.scroll_dir;
                let (on, off) = (0.45, 0.25);
                self.scroll_dir = if value < -if prev == 1 { off } else { on } {
                    1
                } else if value > if prev == -1 { off } else { on } {
                    -1
                } else {
                    0
                };
            }
            Axis::LeftZ => {
                let was = self.lt_down;
                self.lt_down = if was { value > 0.35 } else { value > 0.65 };
                if self.lt_down && !was {
                    return Some(Press::Lt);
                }
            }
            Axis::RightZ => {
                let was = self.rt_down;
                self.rt_down = if was { value > 0.35 } else { value > 0.65 };
                if self.rt_down && !was {
                    return Some(Press::Rt);
                }
            }
            _ => {}
        }
        None
    }

    fn due(&mut self, now: Instant, first: Duration, repeat: Duration) -> impl Iterator<Item = Dir> + '_ {
        [Dir::Up, Dir::Down, Dir::Left, Dir::Right].into_iter().enumerate().filter_map(move |(i, dir)| {
            if !self.dpad[i] && !self.stick[i] {
                self.next[i] = None;
                return None;
            }
            match self.next[i] {
                None => self.next[i] = Some(now + first),
                Some(at) if now >= at => self.next[i] = Some(now + repeat),
                Some(_) => return None,
            }
            Some(dir)
        })
    }

    fn due_scroll(&mut self, now: Instant) -> Option<i8> {
        if self.scroll_dir == 0 {
            self.next_scroll = None;
            return None;
        }
        match self.next_scroll {
            None => {
                self.next_scroll = Some(now + SCROLL_REPEAT);
                Some(self.scroll_dir)
            }
            Some(at) if now >= at => {
                self.next_scroll = Some(now + SCROLL_REPEAT);
                Some(self.scroll_dir)
            }
            Some(_) => None,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.next.into_iter().flatten().chain(self.next_scroll).min()
    }

    fn has_active_hold(&self) -> bool {
        self.dpad.iter().any(|b| *b) || self.stick.iter().any(|b| *b) || self.scroll_dir != 0
    }
}

fn next_poll_timeout(held: &std::collections::HashMap<gilrs::GamepadId, HeldDirections>, now: Instant) -> Duration {
    let earliest = held.values().filter_map(HeldDirections::next_deadline).min();
    if let Some(deadline) = earliest {
        return deadline.saturating_duration_since(now).clamp(Duration::from_millis(5), Duration::from_millis(50));
    }
    if held.values().any(HeldDirections::has_active_hold) {
        return Duration::from_millis(16);
    }
    // Idle: wake immediately on any gilrs event, otherwise check infrequently to save battery.
    Duration::from_millis(250)
}

/// Read pads until the app is gone or, on Windows, the last pad is unplugged.
fn read_connected(mut gilrs: gilrs::Gilrs, tx: &mpsc::Sender<PadEvent>, ctx: &egui::Context) -> bool {
    use gilrs::EventType;
    let send = |e: PadEvent| {
        let ok = tx.send(e).is_ok();
        if ok {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
        ok
    };
    let mut held = std::collections::HashMap::<gilrs::GamepadId, HeldDirections>::new();
    let mut connected = gilrs.gamepads().count();
    if !send(PadEvent::Connected(connected)) {
        return false;
    }
    loop {
        let wait = next_poll_timeout(&held, Instant::now());
        if let Some(ev) = gilrs.next_event_blocking(Some(wait)) {
            match ev.event {
                EventType::Connected | EventType::Disconnected => {
                    if matches!(ev.event, EventType::Disconnected) {
                        held.remove(&ev.id);
                    }
                    let count = gilrs.gamepads().count();
                    if count != connected {
                        connected = count;
                        if !send(PadEvent::Connected(count)) {
                            return false;
                        }
                    }
                    if cfg!(windows) && count == 0 {
                        return true;
                    }
                }
                event => {
                    let state = held.entry(ev.id).or_default();
                    if let Some(press) = state.update(event) {
                        if !send(PadEvent::Press(press)) {
                            return false;
                        }
                    }
                }
            }
        }
        let (first, repeat) = repeat_times(REPEAT_LEVEL.load(Ordering::Relaxed));
        let now = Instant::now();
        for state in held.values_mut() {
            for dir in state.due(now, first, repeat) {
                if !send(PadEvent::Press(Press::Move(dir))) {
                    return false;
                }
            }
            if let Some(scroll) = state.due_scroll(now) {
                if !send(PadEvent::Press(Press::Scroll(scroll))) {
                    return false;
                }
            }
        }
    }
}

/// Which keys stand for which presses. `typing`: letters go to a text box,
/// so only keys that aren't text count. Back is Esc only, like Steam's Big
/// Picture: Backspace deletes in a text box (see [`backspace`]) and does
/// nothing anywhere else, so deleting can never leave a page.
pub fn key_map(typing: bool) -> Vec<(Key, Press)> {
    let mut map = vec![
        (Key::ArrowUp, Press::Move(Dir::Up)),
        (Key::ArrowDown, Press::Move(Dir::Down)),
        (Key::ArrowLeft, Press::Move(Dir::Left)),
        (Key::ArrowRight, Press::Move(Dir::Right)),
        (Key::Enter, Press::A),
        (Key::Escape, Press::B),
        (Key::PageUp, Press::Lb),
        (Key::PageDown, Press::Rb),
        (Key::F1, Press::Start),
        (Key::F2, Press::QuickAccess),
        (Key::Home, Press::Guide),
    ];
    if !typing {
        map.extend([
            (Key::Space, Press::A),
            (Key::Q, Press::Lb),
            (Key::E, Press::Rb),
            (Key::X, Press::X),
            (Key::Y, Press::Y),
            (Key::M, Press::Start),
            (Key::OpenBracket, Press::Lt),
            (Key::CloseBracket, Press::Rt),
            // WASD moves too.
            (Key::W, Press::Move(Dir::Up)),
            (Key::S, Press::Move(Dir::Down)),
            (Key::A, Press::Move(Dir::Left)),
            (Key::D, Press::Move(Dir::Right)),
        ]);
    }
    map
}

/// Backspace in a text box: delete the last character (with Ctrl, the last
/// word). An empty box stays empty; it never means "back".
pub fn backspace(field: &mut String, word: bool) {
    if word {
        let kept = field.trim_end().len();
        field.truncate(kept);
        let cut = field.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
        field.truncate(cut);
    } else {
        field.pop();
    }
}

/// This frame's keyboard presses (taken, so egui doesn't also act on them).
pub fn keyboard(ctx: &egui::Context, typing: bool) -> Vec<Press> {
    let mut out = Vec::new();
    ctx.input_mut(|i| {
        for (key, press) in key_map(typing) {
            if i.consume_key(Modifiers::NONE, key) {
                out.push(press);
            }
        }
        if i.consume_key(Modifiers::SHIFT, Key::Tab) {
            out.push(Press::Lb);
        }
        if i.consume_key(Modifiers::NONE, Key::Tab) {
            out.push(Press::Rb);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swapping_a_and_b() {
        use gilrs::Button;
        assert_eq!(button_press(Button::South, false), Some(Press::A));
        assert_eq!(button_press(Button::East, false), Some(Press::B));
        assert_eq!(button_press(Button::South, true), Some(Press::B));
        assert_eq!(button_press(Button::East, true), Some(Press::A));
        // Only A and B swap.
        assert_eq!(button_press(Button::West, true), Some(Press::X));
        #[cfg(not(windows))]
        assert_eq!(button_press(Button::Mode, true), Some(Press::Guide));
        #[cfg(windows)]
        assert_eq!(button_press(Button::Mode, true), None);
        assert_eq!(button_press(Button::DPadUp, false), None);
    }

    #[test]
    fn controller_levels_keep_their_order() {
        let (slow, normal, fast) = (repeat_times(0), repeat_times(1), repeat_times(2));
        assert!(slow.1 > normal.1 && normal.1 > fast.1);
        assert!(slow.0 > normal.0 && normal.0 > fast.0);
        for level in 0..3 {
            let (on, off) = stick_edges(level);
            assert!(on > off && off > 0.0 && on < 1.0, "the stick needs a gap between on and off");
        }
        assert!(stick_edges(0).0 > stick_edges(1).0 && stick_edges(1).0 > stick_edges(2).0);
    }

    #[test]
    fn short_tap_fires_and_release_cancels_repeat() {
        let mut held = HeldDirections::default();
        let now = Instant::now();
        let (first, repeat) = repeat_times(1);
        held.dpad[2] = true;
        assert_eq!(held.due(now, first, repeat).collect::<Vec<_>>(), [Dir::Left]);
        held.dpad[2] = false;
        assert!(held.due(now + first, first, repeat).next().is_none());
        held.dpad[2] = true;
        assert_eq!(held.due(now + first, first, repeat).collect::<Vec<_>>(), [Dir::Left]);
        assert_eq!(held.due(now + first + first, first, repeat).collect::<Vec<_>>(), [Dir::Left]);
    }

    #[test]
    fn backspace_never_goes_back() {
        for typing in [false, true] {
            let map = key_map(typing);
            assert!(!map.iter().any(|(k, _)| *k == Key::Backspace), "Backspace mapped (typing: {typing})");
            assert!(map.iter().any(|(k, p)| *k == Key::Escape && *p == Press::B), "Esc must be back (typing: {typing})");
        }
    }

    #[test]
    fn typing_keeps_letters_for_the_text_box() {
        let map = key_map(true);
        for key in [Key::Space, Key::Q, Key::E, Key::X, Key::Y, Key::M, Key::W, Key::A, Key::S, Key::D] {
            assert!(!map.iter().any(|(k, _)| *k == key), "{key:?} must type, not act");
        }
    }

    #[test]
    fn backspace_deletes_and_stops_at_empty() {
        let mut s = String::from("hi");
        backspace(&mut s, false);
        assert_eq!(s, "h");
        backspace(&mut s, false);
        backspace(&mut s, false);
        backspace(&mut s, false);
        assert_eq!(s, "");
        let mut s = String::from("see you tonight  ");
        backspace(&mut s, true);
        assert_eq!(s, "see you ");
        backspace(&mut s, true);
        assert_eq!(s, "see ");
        backspace(&mut s, true);
        assert_eq!(s, "");
        backspace(&mut s, true);
        assert_eq!(s, "");
    }

    #[test]
    fn guide_chord_emits_quick_access_without_guide_or_a() {
        use gilrs::Button;
        SWAP_AB.store(false, Ordering::Relaxed);
        let mut held = HeldDirections::default();
        // Standalone Guide emits on release.
        assert_eq!(held.update_button(Button::Mode, true), None);
        assert_eq!(held.update_button(Button::Mode, false), Some(Press::Guide));

        // Guide + A emits QuickAccess immediately and suppresses Guide on release.
        assert_eq!(held.update_button(Button::Mode, true), None);
        assert_eq!(held.update_button(Button::South, true), Some(Press::QuickAccess));
        assert_eq!(held.update_button(Button::South, false), None);
        assert_eq!(held.update_button(Button::Mode, false), None);
    }

    #[test]
    fn right_stick_scrolls_and_triggers_fire_once() {
        use gilrs::Axis;
        let mut held = HeldDirections::default();
        let now = Instant::now();
        held.update_axis(Axis::RightStickY, -0.8);
        assert_eq!(held.due_scroll(now), Some(1));
        assert_eq!(held.due_scroll(now + Duration::from_millis(20)), None);
        assert_eq!(held.due_scroll(now + SCROLL_REPEAT), Some(1));
        held.update_axis(Axis::RightStickY, 0.0);
        assert_eq!(held.due_scroll(now + SCROLL_REPEAT * 2), None);

        assert_eq!(held.update_axis(Axis::LeftZ, 0.8), Some(Press::Lt));
        assert_eq!(held.update_axis(Axis::LeftZ, 0.9), None);
        assert_eq!(held.update_axis(Axis::LeftZ, 0.1), None);
        assert_eq!(held.update_axis(Axis::LeftZ, 0.8), Some(Press::Lt));
    }

    #[test]
    fn idle_poll_timeout_relaxes_when_nothing_held() {
        let held = std::collections::HashMap::new();
        let now = Instant::now();
        assert_eq!(next_poll_timeout(&held, now), Duration::from_millis(250));
    }
}

#[cfg(test)]
#[test]
fn button_hint_preference_round_trips_and_auto_is_default() {
    assert_eq!(ButtonHintMode::default(), ButtonHintMode::Auto);
    for mode in ButtonHintMode::ALL {
        let encoded = serde_json::to_string(&mode).unwrap();
        assert_eq!(serde_json::from_str::<ButtonHintMode>(&encoded).unwrap(), mode);
    }
}
