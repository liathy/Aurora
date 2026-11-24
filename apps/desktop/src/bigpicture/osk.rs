//! Shared on-screen keyboard for Big Picture & Steam Deck Game Mode:
//! English + Cyrillic alphabets, Shift (uppercase), Symbols layer, Clear,
//! and controller shortcuts (X = Delete, Y = Space, LT = Shift, RT = Go).

use super::focus::Place;
use super::{BigPicture, Frame, paint};
use egui::{Align2, Id, Rect, Ui, pos2, vec2};
use i18n::t;
use ui::widgets as w;

const KEYS_EN: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl'", "zxcvbnm-:."];
const KEYS_EN_SHIFT: [&str; 4] = ["!@#$%^&*()", "QWERTYUIOP", "ASDFGHJKL\"", "ZXCVBNM_?!"];
const KEYS_RU: [&str; 4] = ["1234567890-", "йцукенгшщзх", "фывапролджэ", "ячсмитьбюъё"];
const KEYS_RU_SHIFT: [&str; 4] = ["!\"№;%:?*()_", "ЙЦУКЕНГШЩЗХ", "ФЫВАПРОЛДЖЭ", "ЯЧСМИТЬБЮЪЁ"];
const KEYS_SYM: [&str; 4] = ["!@#$%^&*()", "-_=+[]{};:", "/?,.<>'\"~`", "|\\€£¥°•·+*"];

/// Keyboard mode state held on `BigPicture`.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OskState {
    pub cyrillic: bool,
    pub shift: bool,
    pub symbols: bool,
}

impl OskState {
    pub fn rows(self) -> [&'static str; 4] {
        if self.symbols {
            KEYS_SYM
        } else if self.cyrillic {
            if self.shift { KEYS_RU_SHIFT } else { KEYS_RU }
        } else if self.shift {
            KEYS_EN_SHIFT
        } else {
            KEYS_EN
        }
    }

    pub fn max_cols(self) -> usize {
        self.rows().iter().map(|r| r.chars().count()).max().unwrap_or(10)
    }
}

/// A key pressed on the on-screen keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Del,
    Clear,
    Go,
}

/// Apply an OSK key to `target`, optionally capping character length and uppercasing.
/// Returns `true` when `Key::Go` was pressed.
pub fn apply_key(target: &mut String, key: Key, max_chars: Option<usize>, uppercase: bool) -> bool {
    match key {
        Key::Char(ch) => {
            if uppercase {
                for up in ch.to_uppercase() {
                    if max_chars.is_none_or(|m| target.chars().count() < m) {
                        target.push(up);
                    }
                }
            } else if max_chars.is_none_or(|m| target.chars().count() < m) {
                target.push(ch);
            }
            false
        }
        Key::Del => {
            target.pop();
            false
        }
        Key::Clear => {
            target.clear();
            false
        }
        Key::Go => true,
    }
}

/// Draw a compact, touch-friendly desktop on-screen keyboard inside `ui`.
/// Uses the exact same layout layers as the Big Picture keyboard.
pub fn draw_desktop(ui: &mut Ui, state: &mut OskState, t: &ui::Theme, go_label: &str) -> Option<Key> {
    let mut out = None;
    let row_w = ui.available_width();
    let gap = 6.0;
    let top_pad = 8.0;
    let key_h = ((ui.available_height() - top_pad - 4.0 * gap) / 5.0).clamp(36.0, 42.0);
    let total_h = top_pad + 5.0 * key_h + 4.0 * gap;
    let (panel_rect, _) = ui.allocate_exact_size(vec2(row_w, total_h), egui::Sense::hover());
    ui.painter().line_segment(
        [panel_rect.left_top(), panel_rect.right_top()],
        egui::Stroke::new(1.0, t.border.gamma_multiply(0.45)),
    );
    let start_x = panel_rect.left();
    let mut y = panel_rect.top() + top_pad;
    for (ri, row) in state.rows().iter().enumerate() {
        let cols = row.chars().count();
        let key_w = (panel_rect.width() - (cols - 1) as f32 * gap) / cols as f32;
        for (ci, ch) in row.chars().enumerate() {
            let x = start_x + ci as f32 * (key_w + gap);
            let right = if ci + 1 == cols { panel_rect.right() } else { x + key_w };
            let r = Rect::from_min_max(pos2(x, y), pos2(right, y + key_h));
            let label = ch.to_string();
            if desktop_key_cap(ui, t, Id::unique(("desk-osk-key", ri, ci)), r, &label, false, false) {
                out = Some(Key::Char(ch));
            }
        }
        y += key_h + gap;
    }

    let shift_label = if state.shift { "SHIFT" } else { "Shift" };
    let sym_label = if state.symbols { "ABC" } else { "!@#" };
    let lang_label = if state.cyrillic { "ABC" } else { "АБВ" };
    let specials: [(f32, &str, &str, bool, bool); 7] = [
        (1.4, "shift", shift_label, false, state.shift),
        (1.3, "sym", sym_label, false, state.symbols),
        (1.3, "lang", lang_label, false, state.cyrillic && !state.symbols),
        (2.4, " ", t!("Space"), false, false),
        (1.4, "del", t!("Delete"), false, false),
        (1.2, "clear", t!("Clear"), false, false),
        (2.0, "go", go_label, true, false),
    ];
    let weight_sum: f32 = specials.iter().map(|x| x.0).sum();
    let avail = row_w - gap * (specials.len() - 1) as f32;
    let mut x = start_x;
    for (i, (weight, what, label, accent, active)) in specials.iter().enumerate() {
        let right = if i + 1 == specials.len() { panel_rect.right() } else { x + avail * weight / weight_sum };
        let r = Rect::from_min_max(pos2(x, y), pos2(right, y + key_h));
        if desktop_key_cap(ui, t, Id::unique(("desk-osk-special", i)), r, label, *accent, *active) {
            match *what {
                " " => out = Some(Key::Char(' ')),
                "del" => out = Some(Key::Del),
                "clear" => out = Some(Key::Clear),
                "shift" => {
                    state.symbols = false;
                    state.shift = !state.shift;
                }
                "sym" => {
                    state.symbols = !state.symbols;
                }
                "lang" => {
                    state.symbols = false;
                    state.cyrillic = !state.cyrillic;
                }
                _ => out = Some(Key::Go),
            }
        }
        x = right + gap;
    }
    out
}

fn desktop_key_cap(
    ui: &Ui,
    t: &ui::Theme,
    id: Id,
    r: Rect,
    label: &str,
    accent: bool,
    active: bool,
) -> bool {
    let resp = ui.interact(r, id, egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(r) {
        let hover = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered(), 0.1);
        let pressed = resp.is_pointer_button_down_on();
        let base = if accent {
            t.accent
        } else if active {
            w::mix(t.raised, t.accent, 0.28)
        } else {
            w::mix(t.raised, t.surface, 0.25)
        };
        let fill = if accent {
            w::mix(base, egui::Color32::WHITE, 0.12 * hover)
        } else {
            w::mix(base, t.surface_hover, hover)
        };
        let fill = if pressed { w::mix(fill, egui::Color32::BLACK, 0.14) } else { fill };
        let border = if active && !accent {
            t.accent
        } else if accent {
            egui::Color32::TRANSPARENT
        } else {
            w::mix(t.border.gamma_multiply(0.55), t.accent, hover * 0.55)
        };
        ui.painter().rect_filled(r, t.corner_small(), fill);
        if border != egui::Color32::TRANSPARENT {
            ui.painter().rect_stroke(r, t.corner_small(), egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
        }
        let is_single = label.chars().count() == 1;
        let mut font_size = if is_single { 15.5 } else { 12.0 };
        let text_color = if accent { t.on_accent } else { t.text };
        if !is_single {
            let max_w = (r.width() - 6.0).max(8.0);
            let measured = ui.painter().layout_no_wrap(label.to_string(), w::font_semibold(font_size), text_color).size().x;
            if measured > max_w {
                font_size = (font_size * (max_w / measured)).clamp(9.5, font_size);
            }
        }
        let font = if is_single { w::font_semibold(font_size) } else { w::font_medium(font_size) };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font, text_color);
    }
    resp.clicked()
}

/// Draw the on-screen keyboard from `y` down; advances `y` past its bottom.
/// Fits cleanly within `max_width` on both 1080p TVs and 1280×800 handhelds.
pub fn draw(
    bp: &mut BigPicture,
    ui: &mut Ui,
    f: &Frame,
    left: f32,
    max_width: f32,
    y: &mut f32,
    go: &str,
) -> Option<Key> {
    let s = f.s;
    if f.lt {
        bp.osk.shift = !bp.osk.shift;
    }
    let mut out = if f.rt { Some(Key::Go) } else { None };

    let rows = bp.osk.rows();
    let cols = bp.osk.max_cols() as f32;
    let gap = 8.0 * s;
    let key = ((max_width - (cols - 1.0) * gap) / cols).clamp(48.0 * s, 72.0 * s);
    let unit = key + gap;
    let row_w = cols * unit - gap;
    let start_x = left + ((max_width - row_w) * 0.5).max(0.0);

    for (ri, row) in rows.iter().enumerate() {
        let count = row.chars().count() as f32;
        let rx = start_x + ((cols - count) * 0.5 * unit).max(0.0);
        for (ci, ch) in row.chars().enumerate() {
            let r = Rect::from_min_size(pos2(rx + ci as f32 * unit, *y), vec2(key, key));
            let label = ch.to_string();
            if key_cap(bp, ui, f, Id::unique(("bp-key", ri, ci)), r, &label, false, false) {
                out = Some(Key::Char(ch));
            }
        }
        *y += unit;
    }

    // Bottom action row scaled proportionally to fit `row_w`.
    let shift_label = if bp.osk.shift { "SHIFT" } else { "Shift" };
    let sym_label = if bp.osk.symbols { "ABC" } else { "!@#" };
    let lang_label = if bp.osk.cyrillic { "ABC" } else { "АБВ" };
    let specials: [(f32, &str, &str, bool, bool); 7] = [
        (1.4, "shift", shift_label, false, bp.osk.shift),
        (1.3, "sym", sym_label, false, bp.osk.symbols),
        (1.3, "lang", lang_label, false, bp.osk.cyrillic && !bp.osk.symbols),
        (2.4, " ", t!("Space"), false, false),
        (1.4, "del", t!("Delete"), false, false),
        (1.2, "clear", t!("Clear"), false, false),
        (2.0, "go", go, true, false),
    ];
    let weight_sum: f32 = specials.iter().map(|x| x.0).sum();
    let avail = row_w - gap * (specials.len() - 1) as f32;
    let mut x = start_x;
    for (i, (weight, what, label, accent, active)) in specials.iter().enumerate() {
        let wdt = (avail * (*weight / weight_sum)).max(key);
        let r = Rect::from_min_size(pos2(x, *y), vec2(wdt, key));
        if key_cap(bp, ui, f, Id::unique(("bp-key-special", i)), r, label, *accent, *active) {
            match *what {
                " " => out = Some(Key::Char(' ')),
                "del" => out = Some(Key::Del),
                "clear" => out = Some(Key::Clear),
                "shift" => {
                    bp.osk.symbols = false;
                    bp.osk.shift = !bp.osk.shift;
                }
                "sym" => {
                    bp.osk.symbols = !bp.osk.symbols;
                }
                "lang" => {
                    bp.osk.symbols = false;
                    bp.osk.cyrillic = !bp.osk.cyrillic;
                }
                _ => out = Some(Key::Go),
            }
        }
        x += wdt + gap;
    }
    *y += unit;
    out
}

/// Height the keyboard occupies at scale `s` and `max_width` (for chat slide-up animation).
pub fn panel_height(s: f32, max_width: f32, state: OskState) -> f32 {
    let cols = state.max_cols() as f32;
    let gap = 8.0 * s;
    let key = ((max_width - (cols - 1.0) * gap) / cols).clamp(48.0 * s, 72.0 * s);
    5.0 * (key + gap) + 34.0 * s
}

fn key_cap(
    bp: &mut BigPicture,
    ui: &Ui,
    f: &Frame,
    id: Id,
    r: Rect,
    label: &str,
    accent: bool,
    active: bool,
) -> bool {
    let s = f.s;
    let hit = bp.focus.item(ui, id, r, ui.clip_rect(), Place::Page);
    if hit.activated {
        bp.key_pop = Some((id, f.now));
    }
    let pop = bp
        .key_pop
        .filter(|(k, _)| *k == id)
        .map(|(_, at)| ((f.now - at) / 0.16) as f32)
        .filter(|e| *e < 1.0);
    let bump = pop.map(|e| (e * std::f32::consts::PI).sin() * 0.1).unwrap_or(0.0);
    if pop.is_some() {
        ui.ctx().request_repaint();
    }
    let shown = paint::grow(r, 1.0 + 0.05 * hit.glow - bump);
    let base = if accent {
        f.t.accent
    } else if active {
        f.t.accent.gamma_multiply(0.45)
    } else {
        f.t.raised
    };
    let fill = w::mix(base, f.t.accent, hit.glow * 0.75);
    ui.painter().rect_filled(shown, paint::radius(10.0 * s), fill);
    let size = if label.chars().count() == 1 { 28.0 } else { 18.0 };
    let text_color = if accent { f.t.on_accent } else { f.t.text };
    ui.painter().text(shown.center(), Align2::CENTER_CENTER, label, w::font_semibold(size * s), text_color);
    if hit.focused {
        bp.focus.ring_around(shown, 10.0 * s);
    }
    hit.activated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osk_layers_have_four_non_empty_rows() {
        for state in [
            OskState { cyrillic: false, shift: false, symbols: false },
            OskState { cyrillic: false, shift: true, symbols: false },
            OskState { cyrillic: true, shift: false, symbols: false },
            OskState { cyrillic: true, shift: true, symbols: false },
            OskState { cyrillic: false, shift: false, symbols: true },
        ] {
            let rows = state.rows();
            assert_eq!(rows.len(), 4);
            assert!(rows.iter().all(|r| r.chars().count() >= 10));
            assert!(state.max_cols() >= 10);
        }
        assert!(OskState { cyrillic: false, shift: true, symbols: false }.rows()[1].starts_with('Q'));
        assert!(OskState { cyrillic: false, shift: false, symbols: true }.rows()[0].starts_with('!'));
    }

    #[test]
    fn apply_key_handles_chars_symbols_del_clear_and_go() {
        let mut s = String::new();
        assert!(!apply_key(&mut s, Key::Char('a'), None, false));
        assert!(!apply_key(&mut s, Key::Char('@'), None, false));
        assert!(!apply_key(&mut s, Key::Char('!'), None, false));
        assert!(!apply_key(&mut s, Key::Char(' '), None, false));
        assert!(!apply_key(&mut s, Key::Char('Z'), None, false));
        assert_eq!(s, "a@! Z");
        assert!(!apply_key(&mut s, Key::Del, None, false));
        assert_eq!(s, "a@! ");
        assert!(apply_key(&mut s, Key::Go, None, false));
        assert_eq!(s, "a@! ");
        assert!(!apply_key(&mut s, Key::Clear, None, false));
        assert!(s.is_empty());
        assert!(!apply_key(&mut s, Key::Del, None, false));
        assert!(s.is_empty());
    }

    #[test]
    fn apply_key_enforces_guard_code_uppercase_and_limit() {
        let mut code = String::new();
        for ch in ['a', 'b', '3', 'c', 'd', 'e'] {
            assert!(!apply_key(&mut code, Key::Char(ch), Some(5), true));
        }
        assert_eq!(code, "AB3CD");
        assert!(!apply_key(&mut code, Key::Del, Some(5), true));
        assert_eq!(code, "AB3C");
        assert!(!apply_key(&mut code, Key::Char('z'), Some(5), true));
        assert_eq!(code, "AB3CZ");
        assert!(apply_key(&mut code, Key::Go, Some(5), true));
    }

    #[test]
    fn desktop_key_rows_align_with_sign_in_content() {
        let ctx = egui::Context::default();
        let theme = ui::Theme::default();
        for window_width in [600.0, 720.0] {
            let screen = Rect::from_min_max(pos2(44.0, 0.0), pos2(window_width - 44.0, 260.0));
            for mut state in [OskState::default(), OskState { cyrillic: true, ..Default::default() }, OskState { symbols: true, ..Default::default() }] {
                let mut frame = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                    let bounds = ui.available_rect_before_wrap().x_range();
                    assert!(draw_desktop(ui, &mut state, &theme, "Continue").is_none());
                    let rect = |id| ctx.read_response(id).unwrap().rect;
                    for (ri, row) in state.rows().iter().enumerate() {
                        assert_eq!(rect(Id::unique(("desk-osk-key", ri, 0usize))).left(), bounds.min);
                        assert_eq!(rect(Id::unique(("desk-osk-key", ri, row.chars().count() - 1))).right(), bounds.max);
                    }
                    assert_eq!(rect(Id::unique(("desk-osk-special", 0usize))).left(), bounds.min);
                    assert_eq!(rect(Id::unique(("desk-osk-special", 6usize))).right(), bounds.max);
                });
                frame.textures_delta.clear();
            }
        }
    }
}
