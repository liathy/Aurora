//! Steam's profile page (steamcommunity.com/profiles/<id>): the right column
//! (level, featured badge, status, badges, count links, groups, top friends)
//! and Recent Activity, exactly as Steam picks and orders them. There is no
//! service for these, so they're read from the page.

use crate::market::Web;
use model::{PageBadge, PageCount, PageFriend, PageGroup, ProfilePage, RecentGame, SteamId};

const COMMUNITY: &str = "https://steamcommunity.com";
const BASE_ID: u64 = 76561197960265728;

pub async fn load(web: Option<&Web>, steam_id: SteamId, language: &str) -> Result<ProfilePage, String> {
    let http = web.map(|w| w.http.clone()).unwrap_or_else(crate::http);
    let lang = if language.is_empty() { "english" } else { language };
    let body = http
        .get(format!("{COMMUNITY}/profiles/{steam_id}/"))
        .query(&[("l", lang)])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    Ok(parse(&body))
}

// ---------- small HTML helpers ----------

fn after<'a>(s: &'a str, marker: &str) -> Option<&'a str> {
    s.find(marker).map(|i| &s[i + marker.len()..])
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let rest = after(s, start)?;
    Some(&rest[..rest.find(end)?])
}

/// The value of `attr="…"` after the start of `s`.
fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    between(s, &format!("{name}=\""), "\"")
}

fn decode(s: &str) -> String {
    let mut out = s.to_string();
    for (from, to) in [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'"), ("&apos;", "'"), ("&nbsp;", " ")] {
        out = out.replace(from, to);
    }
    out.replace("&amp;", "&")
}

/// Visible text: tags removed, entities decoded, whitespace collapsed.
fn text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lines of an HTML fragment split at `<br>`.
fn lines(html: &str) -> Vec<String> {
    html.replace("<br/>", "<br>").replace("<br />", "<br>").split("<br>").map(text).filter(|l| !l.is_empty()).collect()
}

/// Pieces of `s` that each start with `marker` (the first piece, before it, is dropped).
fn blocks<'a>(s: &'a str, marker: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find(marker) {
        let from = &rest[i..];
        let next = from[marker.len()..].find(marker).map(|n| n + marker.len()).unwrap_or(from.len());
        out.push(&from[..next]);
        rest = &from[next..];
    }
    out
}

fn count_key(url: &str) -> &'static str {
    let path = url.split("steamcommunity.com").nth(1).unwrap_or(url);
    let seg: Vec<&str> = path.split(['/', '?']).filter(|s| !s.is_empty()).collect();
    // /profiles/<id>/<list>/ or /id/<name>/<list>/
    match (seg.get(2).copied().unwrap_or_default(), url.contains("section=guides")) {
        ("myworkshopfiles", true) => "guides",
        ("myworkshopfiles", false) => "workshop",
        ("badges", _) => "badges",
        ("games", _) => "games",
        ("inventory", _) => "inventory",
        ("screenshots", _) => "screenshots",
        ("videos", _) => "videos",
        ("recommended", _) => "reviews",
        ("images", _) => "artwork",
        ("groups", _) => "groups",
        ("friends", _) => "friends",
        _ => "other",
    }
}

/// The line between a right-column section's count link and its first
/// entry: Steam's "You have <a>1 group</a> in common". Kept as HTML so the
/// count stays a link; "" when the section has no such line.
fn in_common(right: &str, section: &str, entries: &[&str]) -> String {
    let Some(s) = after(right, section) else { return String::new() };
    let Some(s) = after(s, "class=\"profile_count_link ellipsis\"").and_then(|s| after(s, "</a>")) else { return String::new() };
    let end = entries.iter().filter_map(|e| s.find(e)).min().unwrap_or(s.len().min(2000));
    let window = &s[..end];
    // Only the element holding the text, without the wrappers around it.
    let Some(a) = window.find("<a ") else { return String::new() };
    let start = window[..a].rfind("<div").map(|d| window[d..].find('>').map_or(d, |g| d + g + 1)).unwrap_or(0);
    let stop = window[a..].find("</div>").map_or(window.len(), |e| a + e);
    let html = window[start..stop].trim();
    if text(html).chars().any(|c| c.is_ascii_digit()) { html.to_string() } else { String::new() }
}

// ---------- the page ----------

pub fn parse(html: &str) -> ProfilePage {
    let mut page = ProfilePage::default();
    // Theme: `<body class="… SummerTheme …">`; a game profile's colors are
    // in an inline `body.GameProfileTheme { --var: value; … }` rule.
    if let Some(class) = after(html, "<body").and_then(|b| attr(b, "class")) {
        page.theme = class.split_whitespace().find_map(|c| c.strip_suffix("Theme")).unwrap_or_default().to_string();
    }
    if !page.theme.is_empty() {
        if let Some(rule) = between(html, &format!("body.{}Theme {{", page.theme), "}") {
            page.theme_vars = rule
                .split(';')
                .filter_map(|decl| {
                    let (k, v) = decl.split_once(':')?;
                    let k = k.trim();
                    k.starts_with("--").then(|| (k.to_string(), v.trim().to_string()))
                })
                .collect();
        }
    }
    // `<img class="profile_flag" src="…/countryflags/au.gif">`, attributes in any order.
    if let Some(i) = html.find("profile_flag") {
        let start = html[..i].rfind("<img").unwrap_or(i);
        let tag = &html[start..];
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        page.country_flag = attr(tag, "src").filter(|s| s.contains("countryflags")).unwrap_or_default().to_string();
    }
    if let Some(info) = after(html, "class=\"profile_header_badgeinfo\"") {
        page.level = between(info, "friendPlayerLevelNum\">", "<").and_then(|n| n.trim().parse().ok());
    }
    if let Some(fav) = after(html, "class=\"favorite_badge\"") {
        let fav = &fav[..fav.find("profile_header_actions").unwrap_or(fav.len().min(3000))];
        let icon = after(fav, "<img").and_then(|i| attr(i, "src")).unwrap_or_default().to_string();
        let name = between(fav, "class=\"name ellipsis\">", "</div>").map(text).unwrap_or_default();
        let detail = between(fav, "class=\"xp\">", "</div>").map(text).unwrap_or_default();
        let url = after(fav, "<a ").and_then(|a| attr(a, "href")).unwrap_or_default().to_string();
        if !icon.is_empty() {
            page.featured_badge = Some(PageBadge { name, detail, icon, url });
        }
    }
    if let Some(st) = after(html, "class=\"profile_in_game persona ") {
        page.status = st.split('"').next().unwrap_or_default().trim().to_string();
        page.status_header = between(st, "profile_in_game_header\">", "</div>").map(text).unwrap_or_default();
        let end = st.find("responsive_count_link_area").unwrap_or(st.len().min(2000));
        page.status_detail = between(&st[..end], "profile_in_game_name\">", "</div>").map(text).unwrap_or_default();
        page.join_link = after(&st[..end], "profile_in_game_joingame")
            .and_then(|j| after(j, "<a ").and_then(|a| attr(a, "href")))
            .filter(|h| h.starts_with("steam://"))
            .unwrap_or_default()
            .to_string();
    }

    // Right column: count links, the badges and groups previews, top friends.
    let right = after(html, "class=\"profile_rightcol\"").unwrap_or_default();
    let right = &right[..right.find("profile_leftcol").unwrap_or(right.len())];
    for block in blocks(right, "class=\"profile_count_link ellipsis\"") {
        let url = after(block, "<a ").and_then(|a| attr(a, "href")).unwrap_or_default().to_string();
        let label = between(block, "count_link_label\">", "</span>").map(text).unwrap_or_default();
        let head = &block[..block.find("</a>").unwrap_or(block.len())];
        let count = between(head, "profile_count_link_total\">", "</span>").and_then(|n| text(n).replace([',', '.', ' '], "").parse().ok());
        let key = count_key(&url).to_string();
        if !label.is_empty() && !page.counts.iter().any(|c| c.key == key && key != "other") {
            page.counts.push(PageCount { key, label, count, url });
        }
    }
    for b in blocks(right, "class=\"profile_badges_badge") {
        let tip = attr(b, "data-tooltip-html").unwrap_or_default();
        let mut parts = decode(tip).split("<br>").map(text).collect::<Vec<_>>().into_iter();
        let name = parts.next().unwrap_or_default();
        let detail = parts.collect::<Vec<_>>().join(" ");
        let url = after(b, "<a ").and_then(|a| attr(a, "href")).unwrap_or_default().to_string();
        let icon = after(b, "<img").and_then(|i| attr(i, "src")).unwrap_or_default().to_string();
        if !icon.is_empty() {
            page.badges.push(PageBadge { name, detail, icon, url });
        }
    }
    page.groups_in_common = in_common(right, "profile_group_links", &["class=\"profile_group\"", "class=\"profile_group "]);
    page.friends_in_common = in_common(right, "profile_friend_links", &["class=\"friendBlock", "profile_topfriends"]);
    // Group blocks are `class="profile_group"` or `class="profile_group …"`,
    // not their inner `profile_group_avatar` etc.
    let starts: Vec<usize> = right.match_indices("class=\"profile_group").map(|(i, m)| i + m.len()).filter(|i| matches!(right.as_bytes().get(*i), Some(b'"' | b' '))).collect();
    for (n, start) in starts.iter().enumerate() {
        let g = &right[*start..starts.get(n + 1).copied().unwrap_or(right.len())];
        let url = after(g, "<a ").and_then(|a| attr(a, "href")).unwrap_or_default().to_string();
        let avatar = after(g, "<img").and_then(|i| attr(i, "src")).map(String::from);
        let name = between(g, "class=\"whiteLink\"", "</a>").and_then(|s| s.split_once('>').map(|x| text(x.1))).unwrap_or_default();
        let members = between(g, "profile_group_membercount\">", "</div>").map(text).unwrap_or_default();
        if !name.is_empty() {
            page.groups.push(PageGroup { name, url, avatar, members });
        }
    }
    for f in blocks(right, "class=\"friendBlock persona ") {
        let status = after(f, "class=\"friendBlock persona ").and_then(|s| s.split('"').next()).unwrap_or_default().trim().to_string();
        let account: u64 = attr(f, "data-miniprofile").and_then(|a| a.parse().ok()).unwrap_or(0);
        let level = between(f, "friendPlayerLevelNum\">", "<").and_then(|n| n.trim().parse().ok()).unwrap_or(0);
        let avatar = after(f, "<img").and_then(|i| attr(i, "src")).map(String::from);
        let content = between(f, "friendBlockContent\">", "<span class=\"friendSmallText\">").unwrap_or_default();
        let name = text(content.split("<br>").next().unwrap_or_default());
        let small = after(f, "<span class=\"friendSmallText\">").unwrap_or_default();
        let small = &small[..small.rfind("</span>").unwrap_or(small.len())];
        if account != 0 {
            page.friends.push(PageFriend { steam_id: account + BASE_ID, name, avatar, status, lines: lines(small), level });
        }
    }
    page.friends.sort_by_key(|f| std::cmp::Reverse(f.level));

    // Recent Activity.
    if let Some(head) = after(html, "recentgame_recentplaytime\">") {
        page.recent_playtime = between(head, "<div>", "</div>").map(text).unwrap_or_default();
    }
    let recent = after(html, "class=\"recent_games\"").unwrap_or_default();
    let recent = &recent[..recent.find("recentgame_quicklinks\">").unwrap_or(recent.len())];
    for g in blocks(recent, "class=\"recent_game\"") {
        let app_id = after(g, "steamcommunity.com/app/").and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).unwrap_or(0);
        if app_id == 0 {
            continue;
        }
        let capsule = after(g, "class=\"game_capsule\"").and_then(|s| attr(s, "src")).map(String::from);
        let details = between(g, "game_info_details\">", "</div>").map(lines).unwrap_or_default();
        let name = between(g, "class=\"game_name\">", "</div>").map(text).unwrap_or_default();
        let achievements = between(g, "game_info_achievement_summary\">", "</a>").map(text).unwrap_or_default();
        let rare = between(g, "num_rare\">", "</span>").map(text).unwrap_or_default();
        let progress = between(g, "grid-template-columns:", ";")
            .map(|cols| {
                let fr: Vec<f32> = cols.split("fr").filter_map(|p| p.rsplit([' ', ',', '(']).next()?.trim().parse().ok()).collect();
                let total: f32 = fr.iter().sum();
                if total > 0.0 { fr.first().copied().unwrap_or(0.0) / total } else { 0.0 }
            })
            .unwrap_or(0.0);
        let icons = blocks(g, "class=\"game_info_achievement\"")
            .into_iter()
            .filter_map(|a| Some((decode(attr(a, "data-tooltip-text").unwrap_or_default()), after(a, "<img").and_then(|i| attr(i, "src"))?.to_string())))
            .collect();
        let more = between(g, "plus_more\"", "</div>").and_then(|s| s.split_once('>').map(|x| text(x.1))).unwrap_or_default();
        // "Screenshots 12", "Review 1": the review links to /recommended/.
        let (mut screenshots, mut review, mut review_url) = (String::new(), String::new(), String::new());
        for link in blocks(g, "class=\"published_file_link\"") {
            let label = link.split_once('>').map(|x| text(&x.1[..x.1.find("</a>").unwrap_or(x.1.len())])).unwrap_or_default();
            let href = attr(link, "href").unwrap_or_default();
            if href.contains("/recommended/") {
                (review, review_url) = (label, href.to_string());
            } else if screenshots.is_empty() {
                screenshots = label;
            }
        }
        // The game's badge, when this profile has one:
        // `game_info_badge` > icon img, `.name`, `.xp`.
        let badge = after(g, "class=\"game_info_badge\"").and_then(|b| {
            let b = &b[..b.find("game_info_stats").unwrap_or(b.len())];
            let icon = after(b, "<img").and_then(|i| attr(i, "src"))?.to_string();
            let name = between(b, "class=\"name\">", "</div>").map(text).unwrap_or_default();
            let detail = between(b, "class=\"xp\">", "</div>").map(text).unwrap_or_default();
            let url = after(b, "<a ").and_then(|a| attr(a, "href")).unwrap_or_default().to_string();
            Some(PageBadge { name, detail, icon, url })
        });
        page.recent.push(RecentGame { app_id, name, capsule, details, achievements, rare, progress, icons, more, screenshots, badge, review, review_url });
    }
    page
}

#[cfg(test)]
mod tests {
    // Shaped like steamcommunity.com's profile page (trimmed, made-up data).
    const PAGE: &str = r#"
<style> body.GameProfileTheme { --gradient-right: rgba(255, 255, 255, 0); --color-showcase-header: rgb(120, 20, 40); --btn-background:rgba(0, 0, 0, .2); } </style>
<body class="flat_page profile_page has_profile_background GameProfileTheme responsive_page " >
<div class="header_real_name ellipsis"><bdi>Jadey</bdi>&nbsp;<img class="profile_flag" src="https://community.example/public/images/countryflags/au.gif">Melbourne</div>
<div class="profile_header_badgeinfo"><a class="persona_level_btn" href="x"><div class="persona_name persona_level">Level <div class="friendPlayerLevel lvl_60"><span class="friendPlayerLevelNum">60</span></div></div></a></div>
<div class="profile_rightcol"><div class="responsive_status_info"><div class="profile_in_game persona in-game"><div class="profile_in_game_header">Currently In-Game</div><div class="profile_in_game_name">Dead by Daylight</div>
<div class="profile_in_game_joingame"><a href="steam://joinlobby/381210/109775241333656222/76561198164781205" class="btn_green_white_innerfade btn_small_thin"><span>Join Game</span></a></div></div></div>
<div class="responsive_count_link_area"><div class="profile_badges"><div class="profile_count_link_preview_ctn"><div class="profile_count_link ellipsis"><a href="https://steamcommunity.com/id/x/badges/"><span class="count_link_label">Badges</span>&nbsp;<span class="profile_count_link_total"> 32 </span></a></div>
<div class="profile_count_link_preview"><div class="profile_badges_badge " data-tooltip-html="Years of Service<br>
Member since 23 November, 2014."><a href="https://steamcommunity.com/id/x/badges/1"><img src="https://img.example/y12.png" class="badge_icon small"></a></div></div></div></div>
<div class="profile_item_links"><div class="profile_count_link ellipsis"><a href="https://steamcommunity.com/id/x/games/?tab=all"><span class="count_link_label">Games</span>&nbsp;<span class="profile_count_link_total"> 1,124 </span></a></div>
<div class="profile_count_link ellipsis"><a href="https://steamcommunity.com/id/x/inventory/"><span class="count_link_label">Inventory</span>&nbsp;</a></div>
<div class="profile_count_link ellipsis"><a href="https://steamcommunity.com/id/x/myworkshopfiles/?section=guides"><span class="count_link_label">Guides</span></a></div></div></div>
<div class="profile_group_links profile_count_link_preview_ctn"><div class="profile_count_link ellipsis"><a href="https://steamcommunity.com/id/x/groups/"><span class="count_link_label">Groups</span>&nbsp;<span class="profile_count_link_total"> 1 </span></a></div>
<div class="profile_count_link_preview"><div class="profile_in_common">You have <a class="whiteLink" href="https://steamcommunity.com/id/x/groupscommon/">1 group</a> in common</div><div class="profile_group profile_primary_group"><div class="profile_group_avatar"><a href="https://steamcommunity.com/groups/g"><img src="https://avatars.example/g_medium.jpg"></a></div><a class="whiteLink" href="https://steamcommunity.com/groups/g"> yanna &lt;3 </a><div class="profile_group_membercount">2 Members</div></div></div></div>
<div class="profile_friend_links"><div class="profile_topfriends profile_count_link_preview">
<div class="friendBlock persona offline" data-miniprofile="10"><a class="friendBlockLinkOverlay" href="x"></a><div class="friendPlayerLevel"><span class="friendPlayerLevelNum">118</span></div><div class="playerAvatar offline"><img src="https://avatars.example/a.jpg"></div><div class="friendBlockContent"> Shaduwi<br> <span class="friendSmallText"> Last Online 29 hrs, 19 mins ago </span></div></div>
<div class="friendBlock persona in-game" data-miniprofile="11"><a class="friendBlockLinkOverlay" href="x"></a><div class="friendPlayerLevel"><span class="friendPlayerLevelNum">143</span></div><div class="playerAvatar in-game"><img src="https://avatars.example/b.jpg"></div><div class="friendBlockContent"> a!<br> <span class="friendSmallText"> <span class="linkFriend_in-game">In-Game<br/>Counter-Strike 2</span> </span></div></div>
</div></div></div>
<div class="profile_leftcol">
<div class="profile_recentgame_header profile_customization_header"><div>Recent Activity</div><div class="recentgame_quicklinks recentgame_recentplaytime"><div>47.8 hours past 2 weeks</div></div></div>
<div class="recent_games"><div class="recent_game"><div class="recent_game_content"><div class="game_info"><div class="game_info_cap "><a href="https://steamcommunity.com/app/381210"><img class="game_capsule" src="https://cdn.example/capsule_184x69.jpg"></a></div><div class="game_info_details"> 1,234 hrs on record<br> last played on 25 Nov </div><div class="game_name"><a class="whiteLink" href="https://steamcommunity.com/app/381210">Dead by Daylight</a></div></div>
<div class="game_info_stats"><span class="game_info_achievement_summary"> <a class="whiteLink" href="x">203 / 318 Achievements</a> <span class="num_rare">93 Rare</span> </span> <div class="achievement_progress_bar_ctn"> <div class="achievement_segmented_bar" style="grid-template-columns: minmax(1px, 203fr) 115fr;"></div></div>
<div class="achievement_icons"><div class="game_info_achievement" data-tooltip-text="Insult to Injury"><a href="x"><img src="https://cdn.example/a1.jpg"></a></div><div role="button" class="game_info_achievement plus_more" onclick="x"> +198 </div></div>
<div class="game_info_badge_border"><div class="game_info_badge"><div class="game_info_badge_icon"><a href="https://steamcommunity.com/id/x/gamecards/381210/"><img src="https://cdn.example/badge.png"></a></div><div class="game_info_badge_description"><div class="name"><a class="whiteLink" href="https://steamcommunity.com/id/x/gamecards/381210/">Clawing</a></div><div class="xp">200 XP</div></div></div></div>
<div class="game_info_stats_publishedfilecounts"><span class="published_file_count_ctn"><a class="published_file_link" href="x">Screenshots 5</a></span><span class="published_file_count_ctn"><a class="published_file_link" href="https://steamcommunity.com/id/x/recommended/381210/">Review 1</a></span></div></div></div></div>
<div class="recent_game"><div class="recent_game_content"><div class="game_info"><div class="game_info_cap "><a href="https://steamcommunity.com/app/570"><img class="game_capsule" src="https://cdn.example/570.jpg"></a></div><div class="game_info_details"> 3.1 hrs on record<br> last played on 22 Nov </div><div class="game_name"><a class="whiteLink" href="https://steamcommunity.com/app/570">Dota 2</a></div></div></div></div>
</div><div class="recentgame_quicklinks"> View <a class="whiteLink" href="x">All Recently Played</a></div></div>
"#;

    #[test]
    fn reads_the_profile_page() {
        let p = super::parse(PAGE);
        assert_eq!(p.theme, "GameProfile");
        assert_eq!(p.theme_vars.len(), 3);
        assert_eq!(p.theme_vars[1], ("--color-showcase-header".to_string(), "rgb(120, 20, 40)".to_string()));
        assert_eq!(p.level, Some(60));
        assert_eq!((p.status.as_str(), p.status_header.as_str(), p.status_detail.as_str()), ("in-game", "Currently In-Game", "Dead by Daylight"));
        let counts: Vec<(&str, Option<u32>)> = p.counts.iter().map(|c| (c.key.as_str(), c.count)).collect();
        assert_eq!(counts, [("badges", Some(32)), ("games", Some(1124)), ("inventory", None), ("guides", None), ("groups", Some(1))]);
        assert_eq!(p.badges.len(), 1);
        assert_eq!((p.badges[0].name.as_str(), p.badges[0].detail.as_str()), ("Years of Service", "Member since 23 November, 2014."));
        assert_eq!(p.groups.len(), 1);
        assert_eq!((p.groups[0].name.as_str(), p.groups[0].members.as_str()), ("yanna <3", "2 Members"));
        // Highest level first.
        assert_eq!(p.friends.iter().map(|f| f.level).collect::<Vec<_>>(), [143, 118]);
        assert_eq!(p.friends[0].steam_id, 76561197960265739);
        assert_eq!((p.friends[0].name.as_str(), p.friends[0].status.as_str()), ("a!", "in-game"));
        assert_eq!(p.friends[0].lines, ["In-Game", "Counter-Strike 2"]);
        assert_eq!(p.friends[1].lines, ["Last Online 29 hrs, 19 mins ago"]);
        assert_eq!(p.recent_playtime, "47.8 hours past 2 weeks");
        assert_eq!(p.recent.len(), 2);
        let g = &p.recent[0];
        assert_eq!((g.app_id, g.name.as_str()), (381210, "Dead by Daylight"));
        assert_eq!(g.details, ["1,234 hrs on record", "last played on 25 Nov"]);
        assert_eq!((g.achievements.as_str(), g.rare.as_str(), g.more.as_str(), g.screenshots.as_str()), ("203 / 318 Achievements", "93 Rare", "+198", "Screenshots 5"));
        assert!((g.progress - 203.0 / 318.0).abs() < 0.001);
        assert_eq!(g.icons, [("Insult to Injury".to_string(), "https://cdn.example/a1.jpg".to_string())]);
        assert_eq!((p.recent[1].achievements.as_str(), p.recent[1].icons.len()), ("", 0));
        // The game's badge and review, only where the profile has them.
        let badge = g.badge.as_ref().unwrap();
        assert_eq!((badge.name.as_str(), badge.detail.as_str(), badge.icon.as_str()), ("Clawing", "200 XP", "https://cdn.example/badge.png"));
        assert_eq!((g.review.as_str(), g.review_url.as_str()), ("Review 1", "https://steamcommunity.com/id/x/recommended/381210/"));
        assert!(p.recent[1].badge.is_none() && p.recent[1].review.is_empty());
        assert_eq!(p.country_flag, "https://community.example/public/images/countryflags/au.gif");
        // "You have 1 group in common", kept as HTML; no friends line here.
        assert_eq!(p.groups_in_common, r#"You have <a class="whiteLink" href="https://steamcommunity.com/id/x/groupscommon/">1 group</a> in common"#);
        assert_eq!(p.friends_in_common, "");
        assert_eq!(p.join_link, "steam://joinlobby/381210/109775241333656222/76561198164781205");
    }
}
