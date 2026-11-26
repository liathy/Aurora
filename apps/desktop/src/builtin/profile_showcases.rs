//! A profile's showcases as Steam shows them: favorite game and group,
//! custom text, artwork, screenshots, workshop items and guides, items,
//! badges, achievements, games and reviews, in the order their owner chose.

use super::{Profile, panel};
use client::Command;
use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Ui, pos2, vec2};
use extensions::Ctx;
use i18n::t;
use model::{AppId, Showcase, ShowcaseAchievement, ShowcaseGame, SteamId, art, showcase as kind};
use ui::{Theme, icons, widgets as w};

/// Open a link from Steam text: Steam pages in Aurora, the rest in the browser.
pub(super) fn follow(ctx: &mut Ctx, url: Option<String>) {
    if let Some(a) = url.as_deref().and_then(crate::links::web_action) {
        ctx.act(a);
    }
}

fn title_of(s: &Showcase) -> String {
    match s.kind {
        kind::RARE_ACHIEVEMENTS => t!("Rarest Achievement Showcase").to_string(),
        kind::GAME_COLLECTOR => t!("Game Collector").to_string(),
        kind::ITEMS => t!("Item Showcase").to_string(),
        kind::TRADE => t!("Items Up For Trade").to_string(),
        kind::BADGES => t!("Badge Collector").to_string(),
        kind::FAVORITE_GAME => t!("Favorite Game").to_string(),
        kind::SCREENSHOTS => t!("Screenshot Showcase").to_string(),
        kind::CUSTOM_TEXT => s.slots.first().map(|x| x.title.trim().to_string()).filter(|x| !x.is_empty()).unwrap_or_else(|| t!("Custom Info Box").to_string()),
        kind::FAVORITE_GROUP => t!("Favorite Group").to_string(),
        kind::REVIEW => t!("Review Showcase").to_string(),
        // Steam titles both "Workshop Showcase".
        kind::WORKSHOP | kind::MY_WORKSHOP => t!("Workshop Showcase").to_string(),
        kind::ARTWORK | kind::SINGLE_ARTWORK => t!("Artwork Showcase").to_string(),
        kind::VIDEO => t!("Video Showcase").to_string(),
        kind::GUIDES => t!("Guide Showcase").to_string(),
        kind::MY_GUIDES => t!("My Guides Showcase").to_string(),
        kind::ACHIEVEMENTS => t!("Achievement Showcase").to_string(),
        kind::GREENLIGHT | kind::MY_GREENLIGHT => t!("Greenlight Showcase").to_string(),
        kind::SALIEN => t!("Saliens Showcase").to_string(),
        kind::REACTIONS => t!("Points Shop Reactions").to_string(),
        kind::COMPLETIONIST => t!("Completionist Showcase").to_string(),
        kind::REPLAY => t!("Steam Replay").to_string(),
        _ => t!("Showcase").to_string(),
    }
}

impl Profile {
    /// Every showcase on the profile, each in its own panel.
    pub(super) fn showcases(&mut self, ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId) {
        if !ctx.state.showcases.contains_key(&id) && self.showcase_requested.insert(id) {
            ctx.send(Command::LoadShowcases { steam_id: id, language: i18n::language().steam_name().into() });
        }
        let list = match ctx.state.showcases.get(&id) {
            Some(Ok(list)) => list.clone(),
            Some(Err(_)) => return,
            None => {
                panel(ui, t, |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 120.0), Sense::hover());
                    w::skeleton(ui, t, r, t.corner_small());
                });
                ui.add_space(4.0);
                return;
            }
        };
        // Completionist and Workshop showcases as Steam's page draws them
        // (game headers, workshop pictures and stats only it has), matched
        // to the showcases by their order on the page.
        let page = match ctx.state.profile_pages.get(&id) {
            Some(Ok(p)) => Some(p.clone()),
            _ => None,
        };
        let (mut nth_completionist, mut nth_workshop) = (0, 0);
        for s in &list {
            let from_page = match s.kind {
                kind::COMPLETIONIST => {
                    nth_completionist += 1;
                    page.as_ref().and_then(|p| p.completionist.get(nth_completionist - 1)).filter(|c| !c.games.is_empty()).map(PageShowcase::Completionist)
                }
                kind::MY_WORKSHOP => {
                    nth_workshop += 1;
                    page.as_ref().and_then(|p| p.workshop.get(nth_workshop - 1)).map(PageShowcase::Workshop)
                }
                _ => None,
            };
            if from_page.is_none() && !self.has_content(ctx, id, s) {
                continue;
            }
            panel(ui, t, |ui| {
                // Steam shows artwork on its own, without a title bar.
                if !matches!(s.kind, kind::ARTWORK | kind::SINGLE_ARTWORK) {
                    super::section_title(ui, t, &title_of(s));
                    ui.add_space(8.0);
                }
                match from_page {
                    Some(PageShowcase::Completionist(c)) => completionist(ui, t, ctx, c),
                    Some(PageShowcase::Workshop(w)) => my_workshop(ui, t, ctx, w),
                    None => self.showcase_body(ui, t, ctx, id, s),
                }
            });
            ui.add_space(4.0);
        }
    }

    /// Whether there's anything to draw (empty showcases are skipped, like Steam).
    fn has_content(&self, ctx: &Ctx, id: SteamId, s: &Showcase) -> bool {
        match s.kind {
            kind::FAVORITE_GROUP => s.group.is_some(),
            kind::CUSTOM_TEXT => s.slots.iter().any(|x| !x.notes.trim().is_empty() || !x.title.trim().is_empty()),
            kind::BADGES => matches!(ctx.state.badges.get(&id), Some(Ok(b)) if !b.items.is_empty()),
            kind::GAME_COLLECTOR => true,
            kind::MY_WORKSHOP | kind::MY_GUIDES | kind::VIDEO | kind::GREENLIGHT | kind::MY_GREENLIGHT | kind::SALIEN | kind::REACTIONS | kind::REPLAY => true,
            kind::RARE_ACHIEVEMENTS | kind::ACHIEVEMENTS => s.slots.iter().any(|x| x.achievement.is_some()),
            kind::COMPLETIONIST | kind::FAVORITE_GAME => s.slots.iter().any(|x| x.app_id != 0),
            _ => s.slots.iter().any(|x| x.app_id != 0 || x.file.is_some() || x.asset_id != 0),
        }
    }

    fn showcase_body(&mut self, ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId, s: &Showcase) {
        match s.kind {
            kind::FAVORITE_GAME => {
                if let Some(app) = s.slots.iter().map(|x| x.app_id).find(|a| *a != 0) {
                    favorite_game(ui, t, ctx, id, app, s.games.iter().find(|g| g.app_id == app));
                }
            }
            kind::FAVORITE_GROUP => {
                if let Some(g) = &s.group {
                    favorite_group(ui, t, g);
                }
            }
            kind::CUSTOM_TEXT => {
                if let Some(body) = s.slots.iter().map(|x| x.notes.trim()).find(|n| !n.is_empty()) {
                    let url = ui::steam_text::show(ui, t, body, ui::steam_text::Mode::Markup, 14.0, t.text);
                    follow(ctx, url);
                }
            }
            kind::SCREENSHOTS | kind::ARTWORK | kind::SINGLE_ARTWORK | kind::WORKSHOP | kind::GUIDES => files(ui, t, ctx, s),
            kind::ITEMS | kind::TRADE => self.items(ui, t, ctx, id, s),
            kind::BADGES => badge_collector(ui, t, ctx, id, s),
            kind::GAME_COLLECTOR => game_collector(ui, t, ctx, id, s),
            kind::RARE_ACHIEVEMENTS | kind::ACHIEVEMENTS => {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for (slot, a) in s.slots.iter().filter_map(|x| x.achievement.as_ref().map(|a| (x, a))) {
                        if achievement_icon(ui, t, a, 64.0) {
                            ctx.navigate("achievements", Some(slot.app_id.to_string()));
                        }
                    }
                });
            }
            kind::COMPLETIONIST => {
                for g in &s.games {
                    game_progress_row(ui, t, ctx, g);
                    ui.add_space(6.0);
                }
            }
            kind::REVIEW => review(ui, t, ctx, id, s),
            _ => {
                // Showcases Aurora doesn't draw yet: open them on Steam.
                let url = match s.kind {
                    kind::MY_WORKSHOP => format!("https://steamcommunity.com/profiles/{id}/myworkshopfiles/"),
                    kind::MY_GUIDES => format!("https://steamcommunity.com/profiles/{id}/myworkshopfiles/?section=guides"),
                    kind::VIDEO => format!("https://steamcommunity.com/profiles/{id}/videos/"),
                    _ => format!("https://steamcommunity.com/profiles/{id}/"),
                };
                if w::button(ui, t, Some(icons::EXTERNAL), t!("View on Steam"), w::Kind::Secondary, w::Size::Sm).clicked() {
                    crate::links::open_http(&url);
                }
            }
        }
    }

    /// Item and trade showcases: the chosen inventory items.
    fn items(&mut self, ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId, s: &Showcase) {
        let size = 84.0;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for slot in s.slots.iter().filter(|x| x.asset_id != 0) {
                let (app, context) = (if slot.app_id == 0 { 753 } else { slot.app_id }, if slot.context_id == 0 { 6 } else { slot.context_id });
                if !ctx.state.inventories.contains_key(&(id, app, context)) && self.showcase_inventories.insert((id, app, context)) {
                    ctx.send(Command::LoadInventory { steam_id: id, app_id: app, context_id: context, language: i18n::language().steam_name().into() });
                }
                let item = ctx.state.inventories.get(&(id, app, context)).and_then(|inv| inv.items.iter().find(|i| i.asset_id == slot.asset_id)).cloned();
                let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
                ui.painter().rect_filled(r, t.corner_small(), t.raised);
                match &item {
                    Some(item) => {
                        if !w::paint_contain(ui, r.shrink(6.0), &art::econ(&item.icon, 192), Align2::CENTER_CENTER) {
                            w::skeleton(ui, t, r.shrink(6.0), CornerRadius::same(4));
                        }
                        if let Some(c) = crate::builtin::inventory::rarity_color(item) {
                            let bar = Rect::from_min_max(pos2(r.left() + 8.0, r.bottom() - 5.0), pos2(r.right() - 8.0, r.bottom() - 2.5));
                            ui.painter().rect_filled(bar, CornerRadius::same(2), c);
                        }
                        if resp.on_hover_text(format!("{}\n{}", item.name, item.kind)).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            ctx.navigate("inventory", Some(format!("{id}/{app}")));
                        }
                    }
                    None => w::skeleton(ui, t, r.shrink(6.0), CornerRadius::same(4)),
                }
            }
        });
        if s.kind == kind::TRADE {
            if let Some(notes) = s.slots.iter().map(|x| x.notes.trim()).find(|n| !n.is_empty()) {
                ui.add_space(8.0);
                let url = ui::steam_text::show(ui, t, notes, ui::steam_text::Mode::Markup, 13.5, t.text_dim);
                follow(ctx, url);
            }
        }
    }
}

/// A showcase read from Steam's profile page.
#[derive(Clone, Copy)]
enum PageShowcase<'a> {
    Completionist(&'a model::PageCompletionist),
    Workshop(&'a model::PageWorkshop),
}

/// The blue rosette Steam puts by a completed achievement set.
const RIBBON: Color32 = Color32::from_rgb(76, 160, 240);

/// Steam's Completionist Showcase: the games' headers two to a row, each
/// with its ribbon, achievement count and rare count under it, then the
/// stats row (completed games, completed sets, average completion).
fn completionist(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, c: &model::PageCompletionist) {
    const GAP: f32 = 8.0;
    const BAR_H: f32 = 44.0;
    let width = ui.available_width();
    let each = ((width - GAP) / 2.0).floor();
    // Steam's header pictures are 460×215.
    let pic_h = (each * 215.0 / 460.0).round();
    let mut open = None;
    for row in c.games.chunks(2) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for g in row {
                let (r, resp) = ui.allocate_exact_size(vec2(each, pic_h + BAR_H), Sense::click());
                let pic = Rect::from_min_size(r.min, vec2(each, pic_h));
                let image = if g.image.is_empty() { art::header(g.app_id) } else { g.image.clone() };
                w::image_or_skeleton(ui, t, pic, &image, CornerRadius::ZERO);
                let bar = Rect::from_min_max(pos2(r.left(), pic.bottom()), r.max);
                let p = ui.painter();
                p.text(pos2(bar.left() + 18.0, bar.center().y), Align2::CENTER_CENTER, icons::MEDAL, w::font(24.0), RIBBON);
                let right = bar.right() - 8.0;
                if g.rare.is_empty() {
                    p.text(pos2(right, bar.center().y), Align2::RIGHT_CENTER, &g.achievements, w::font_bold(14.0), Color32::WHITE);
                } else {
                    p.text(pos2(right, bar.center().y + 1.0), Align2::RIGHT_BOTTOM, &g.achievements, w::font_bold(14.0), Color32::WHITE);
                    p.text(pos2(right, bar.center().y + 2.0), Align2::RIGHT_TOP, &g.rare, w::font_bold(10.5), t.text_dim);
                }
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    open = Some(g.url.clone());
                }
            }
        });
        ui.add_space(4.0);
    }
    if !c.stats.is_empty() {
        ui.add_space(14.0);
        page_stats(ui, t, ctx, &c.stats);
    }
    if open.is_some() {
        follow(ctx, open);
    }
}

/// Steam's Workshop Showcase: the owner's avatar and workshop name, their
/// items side by side at their own shape, then submissions and followers.
fn my_workshop(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, ws: &model::PageWorkshop) {
    let mut open_url = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
        ui.painter().rect_filled(r, CornerRadius::ZERO, t.text_faint);
        match &ws.avatar {
            Some(u) => w::image_or_skeleton(ui, t, r.shrink(1.0), u, CornerRadius::ZERO),
            None => w::skeleton(ui, t, r.shrink(1.0), CornerRadius::ZERO),
        }
        let name = ui.add(egui::Label::new(w::text(&ws.title, 22.0, t.text)).sense(Sense::click()));
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() || name.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            open_url = Some(ws.url.clone());
        }
    });
    if !ws.items.is_empty() {
        ui.add_space(24.0);
        const GAP: f32 = 4.0;
        let n = ws.items.len() as f32;
        let each = ((ui.available_width() - GAP * (n - 1.0)) / n).floor();
        let mut open = None;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for item in &ws.items {
                // Each picture at its own shape, like Steam: tall artwork
                // split across the items lines up as one picture.
                if w::picture_fit_width(ui, t, &item.image, each, 420.0, 24.0).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    open = Some(item.id);
                }
            }
        });
        if let Some(id) = open {
            ctx.navigate("workshop_item", Some(id.to_string()));
        }
    }
    if !ws.stats.is_empty() {
        ui.add_space(14.0);
        page_stats(ui, t, ctx, &ws.stats);
    }
    if open_url.as_deref().is_some_and(|u| !u.is_empty()) {
        follow(ctx, open_url);
    }
}

/// A stats row from Steam's page; stats with a link open it.
fn page_stats(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, list: &[model::PageStat]) {
    let mut open = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 48.0;
        for s in list {
            let r = ui.scope(|ui| stat(ui, t, &s.value, &s.label, t.text)).response;
            let mut resp = ui.interact(r.rect, ui.id().with(("page_stat", &s.label)), if s.url.is_empty() { Sense::hover() } else { Sense::click() });
            if !s.tooltip.is_empty() {
                resp = resp.on_hover_text(&s.tooltip);
            }
            if !s.url.is_empty() && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                open = Some(s.url.clone());
            }
        }
    });
    if open.is_some() {
        follow(ctx, open);
    }
}

/// Big number with its label underneath, like Steam's showcase stats.
fn stat(ui: &mut Ui, t: &Theme, value: &str, label: &str, color: Color32) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(w::text(value, 26.0, color));
        ui.label(w::text(label, 13.5, t.text_dim));
    });
}

fn game_name(ctx: &Ctx, id: SteamId, app: AppId) -> String {
    ctx.state
        .player_games
        .get(&id)
        .and_then(|g| g.as_ref().ok())
        .and_then(|g| g.iter().find(|x| x.app_id == app))
        .map(|g| g.name.clone())
        .or_else(|| ctx.state.app_name(app).map(String::from))
        .unwrap_or_else(|| t!("App {id}", id = app).to_string())
}

fn game_header(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, app: AppId, size: egui::Vec2) {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    w::image_or_skeleton(ui, t, r, &art::header(app), t.corner_small());
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        ctx.act(crate::shell::game_page(ctx.state, app));
    }
}

fn favorite_game(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId, app: AppId, game: Option<&ShowcaseGame>) {
    ui.horizontal(|ui| {
        game_header(ui, t, ctx, app, vec2(184.0, 69.0));
        ui.add_space(8.0);
        ui.label(w::text(game_name(ctx, id, app), 22.0, t.text));
    });
    ui.add_space(10.0);
    let minutes = ctx.state.player_games.get(&id).and_then(|g| g.as_ref().ok()).and_then(|g| g.iter().find(|x| x.app_id == app)).map(|g| g.playtime_minutes);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 40.0;
        if let Some(m) = minutes {
            stat(ui, t, &format!("{:.1}", m as f64 / 60.0), t!("Hours played"), t.text);
        }
        if let Some(g) = game.filter(|g| g.total > 0) {
            stat(ui, t, &g.unlocked.to_string(), t!("Achievements"), t.text);
        }
    });
    if let Some(g) = game.filter(|g| g.total > 0) {
        ui.add_space(10.0);
        game_progress_row(ui, t, ctx, g);
    }
}

/// "12 / 20 Achievements", rare count, progress bar and the rarest icons.
fn game_progress_row(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, g: &ShowcaseGame) {
    ui.horizontal(|ui| {
        ui.label(w::text(icons::TROPHY, 22.0, t.accent));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let bar_w = (ui.available_width() - 6.0 * 44.0).clamp(160.0, 340.0);
            ui.allocate_ui_with_layout(vec2(bar_w, 18.0), Layout::left_to_right(Align::Center), |ui| {
                ui.label(w::text(t!("{n} / {total} Achievements", n = g.unlocked, total = g.total), 13.0, t.text));
                if g.rare > 0 {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(w::text(t!("{n} Rare", n = g.rare), 12.5, t.away));
                    });
                }
            });
            w::progress(ui, t, g.unlocked as f32 / g.total.max(1) as f32, bar_w);
        });
        ui.add_space(8.0);
        for a in g.recent.iter().take(5) {
            if achievement_icon(ui, t, a, 36.0) {
                ctx.navigate("achievements", Some(g.app_id.to_string()));
            }
        }
        let more = g.unlocked.saturating_sub(g.recent.len().min(5) as u32);
        if more > 0 {
            let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(r, t.corner_small(), t.raised);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("+{more}"), w::font_semibold(12.5), t.text);
        }
    });
}

fn achievement_icon(ui: &mut Ui, t: &Theme, a: &ShowcaseAchievement, size: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if a.icon.is_empty() || !w::paint_cover(ui, r, &a.icon, CornerRadius::same(3)) {
        w::skeleton(ui, t, r, CornerRadius::same(3));
    }
    let tip = format!("{}\n{}\n{}", a.name, a.description, t!("{pct}% of players have this achievement", pct = format!("{:.1}", a.percent)));
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn favorite_group(ui: &mut Ui, t: &Theme, g: &model::ShowcaseGroup) {
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(156.0, 156.0), Sense::click());
        match &g.avatar {
            Some(u) => w::image_or_skeleton(ui, t, r, u, t.corner_small()),
            None => w::placeholder(ui, t, r, "", t.corner_small()),
        }
        let open = resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
        ui.add_space(10.0);
        let mut open_name = false;
        ui.vertical(|ui| {
            open_name = ui.add(egui::Label::new(w::text(&g.name, 24.0, t.text)).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
            ui.add_space(40.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 26.0;
                stat(ui, t, &g.members.to_string(), t!("Members"), t.text);
                stat(ui, t, &g.in_game.to_string(), t!("In-Game"), t.in_game);
                stat(ui, t, &g.online.to_string(), t!("Online"), t.online);
                stat(ui, t, &g.in_chat.to_string(), t!("In Chat"), t.text);
            });
        });
        if (open || open_name) && !g.url.is_empty() {
            crate::links::open_http(&g.url);
        }
    });
}

/// Screenshots, artwork, workshop items and guides: the first large, then a row.
fn files(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, s: &Showcase) {
    let list: Vec<&model::ShowcaseFile> = s.slots.iter().filter_map(|x| x.file.as_ref()).collect();
    let width = ui.available_width();
    let big = matches!(s.kind, kind::SCREENSHOTS | kind::ARTWORK | kind::SINGLE_ARTWORK);
    let mut open = None;
    if big {
        // Like Steam: the main picture at its own shape (animated GIFs play,
        // capped at 24 fps), the rest in a row below.
        if let Some(first) = list.first() {
            if let Some(url) = &first.image {
                let resp = w::picture_fit_width(ui, t, url, width, 900.0, 24.0);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    open = Some(first.id);
                }
            }
        }
        if list.len() > 1 {
            ui.add_space(6.0);
            let n = (list.len() - 1).min(4) as f32;
            let each = (width - 6.0 * (n - 1.0)) / n;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for f in list.iter().skip(1).take(4) {
                    if let Some(url) = &f.image {
                        if w::picture_fit_width(ui, t, url, each, each, 24.0).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            open = Some(f.id);
                        }
                    }
                }
            });
        }
        if let Some(file) = open {
            ctx.navigate("workshop_item", Some(file.to_string()));
        }
        return;
    }
    let mut tile = |ui: &mut Ui, f: &model::ShowcaseFile, size: egui::Vec2| {
        let (r, resp) = ui.allocate_exact_size(size, Sense::click());
        match &f.image {
            Some(u) => w::image_or_skeleton(ui, t, r, u, t.corner_small()),
            None => w::placeholder(ui, t, r, &f.title, t.corner_small()),
        }
        let tip = if f.title.is_empty() { t!("Open").to_string() } else { f.title.clone() };
        if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            open = Some(f.id);
        }
    };
    let n = list.len().clamp(1, 4) as f32;
    let each = (width - 8.0 * (n - 1.0)) / n;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for f in list.iter().take(4) {
            ui.vertical(|ui| {
                ui.set_width(each);
                tile(ui, f, vec2(each, each * 9.0 / 16.0));
                ui.add(egui::Label::new(w::text(&f.title, 13.0, t.text)).truncate());
            });
        }
    });
    if let Some(file) = open {
        ctx.navigate("workshop_item", Some(file.to_string()));
    }
}

fn badge_collector(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId, s: &Showcase) {
    let Some(Ok(b)) = ctx.state.badges.get(&id).cloned() else { return };
    // The badges the owner picked (by game or badge id), else the first ones.
    let picked: Vec<&model::Badge> = s
        .slots
        .iter()
        .filter_map(|x| {
            b.items.iter().find(|badge| {
                (x.app_id != 0 && badge.url.contains(&format!("/gamecards/{}", x.app_id))) || (x.badge_id != 0 && badge.url.trim_end_matches('/').ends_with(&format!("/badges/{}", x.badge_id)))
            })
        })
        .collect();
    let shown: Vec<&model::Badge> = if picked.is_empty() { b.items.iter().take(6).collect() } else { picked };
    ui.horizontal(|ui| {
        stat(ui, t, &b.items.len().to_string(), t!("Badges Earned"), t.text);
        ui.add_space(20.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for badge in shown {
                if super::badge_icon(ui, t, badge, 64.0) {
                    super::open_badge(ctx, id, badge);
                }
            }
        });
    });
}

fn game_collector(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId, s: &Showcase) {
    let count = ctx.state.player_games.get(&id).and_then(|g| g.as_ref().ok()).map(|g| g.len());
    ui.horizontal(|ui| {
        if let Some(n) = count {
            stat(ui, t, &n.to_string(), t!("Games Owned"), t.text);
            ui.add_space(20.0);
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for app in s.slots.iter().map(|x| x.app_id).filter(|a| *a != 0).take(4) {
                game_header(ui, t, ctx, app, vec2(184.0, 69.0));
            }
        });
    });
}

fn review(ui: &mut Ui, t: &Theme, ctx: &mut Ctx, id: SteamId, s: &Showcase) {
    let Some(app) = s.slots.iter().map(|x| x.app_id).find(|a| *a != 0) else { return };
    let found = ctx.state.reviews.get(&(id, 1)).and_then(|r| r.as_ref().ok()).and_then(|r| r.items.iter().find(|x| x.app_id == app).cloned());
    ui.horizontal_top(|ui| {
        game_header(ui, t, ctx, app, vec2(184.0, 69.0));
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(w::text_semibold(game_name(ctx, id, app), 16.0, t.text));
            if let Some(r) = &found {
                let (icon, label, color) = if r.recommended { (icons::THUMBS_UP, t!("Recommended"), t.online) } else { (icons::THUMBS_DOWN, t!("Not Recommended"), t.danger) };
                ui.label(w::text(format!("{icon}  {label}"), 14.0, color));
                if !r.hours.is_empty() {
                    ui.label(w::text(&r.hours, 12.0, t.text_faint));
                }
            }
        });
    });
    if let Some(r) = found {
        ui.add_space(8.0);
        let text: String = r.text.chars().take(600).collect();
        let url = ui::steam_text::show(ui, t, &text, ui::steam_text::Mode::Markup, 13.5, t.text);
        follow(ctx, url);
    }
}
