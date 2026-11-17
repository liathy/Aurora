# Changelog

## [0.8.1](https://github.com/mollie2999/Aurora/compare/v0.8.0...v0.8.1) (2026-11-17)

### New

* Launch options work like Steam's `%command%`: `MANGOHUD=1 %command%` sets variables for the game, `gamemoderun mangohud %command% -dx11` runs it through a wrapper with arguments after it. Without `%command%`, everything stays a game argument as before ([#15](https://github.com/mollie2999/Aurora/pull/15)) ([e792e1b](https://github.com/mollie2999/Aurora/commit/e792e1b6f5a59e50af6150bd1940c4fda4c9b6bc))
* Linux: games with both a Windows and a Linux build ask which one to install (Windows with Proton stays the default); updates keep the one you have ([#17](https://github.com/mollie2999/Aurora/pull/17)) ([9a465f9](https://github.com/mollie2999/Aurora/commit/9a465f92bb2eaf5a6928e5faa963991c0d1a3ef8))
* Badges, reviews and groups open in Aurora instead of the browser: all badges with your level and XP, reviews with playtime and helpful votes, groups with their member counts ([#20](https://github.com/mollie2999/Aurora/pull/20)) ([920794b](https://github.com/mollie2999/Aurora/commit/920794ba191c70bcc5aab6cc4d2046f7f2d29049))
* Trading-card collections: a game badge opens its card set (which cards you have, how many, the badge so far, normal and foil), and a card in the inventory has a See set button ([#20](https://github.com/mollie2999/Aurora/pull/20)) ([5c2623c](https://github.com/mollie2999/Aurora/commit/5c2623c1f081984e14d10941c815d02fab3edb94)) ([f1c5bba](https://github.com/mollie2999/Aurora/commit/f1c5bbab4eec6230b354065a5cfce3d2117c3adf))

### Fixes

* Linux: a Proton game's first launch has Steam features (online, achievements). It used to get them only from the second launch; a short "Proton setup" now runs first ([#18](https://github.com/mollie2999/Aurora/pull/18)) ([6ba3137](https://github.com/mollie2999/Aurora/commit/6ba3137caaa26ff79cd834995ada1bc58796aae4))
* Linux: games with a launcher (Cities: Skylines' Paradox Launcher, War Thunder) no longer lose Steam when the launcher hands over: Aurora waits for everything the game started ([#23](https://github.com/mollie2999/Aurora/pull/23)) ([3b2ab7e](https://github.com/mollie2999/Aurora/commit/3b2ab7e9fb30afb2a185d010bd50193049c64d75))
* Linux: native games with a newer Steam SDK find Steam ("Steam is not running" before) ([#19](https://github.com/mollie2999/Aurora/pull/19)) ([f62bda6](https://github.com/mollie2999/Aurora/commit/f62bda6920cee652551245c3131dea1de3a5522c))
* Linux: games on a drive whose path has a colon (like `/mnt/usb-…-0:0-part1`) start; Unity games like Cities: Skylines quit with exit code 1 there ([#22](https://github.com/mollie2999/Aurora/pull/22)) ([53ea3cf](https://github.com/mollie2999/Aurora/commit/53ea3cff814572aa620aa98ae23d1e20faaf5f52))
* Linux: Linux builds installed by Aurora start ("Permission denied" before) ([#21](https://github.com/mollie2999/Aurora/pull/21)) ([890bfce](https://github.com/mollie2999/Aurora/commit/890bfce0587c9d89f3d0d6a05225a1cabba6dd96))
* Linux: a game with only its Windows files installed runs them with Proton by default instead of looking for the missing Linux build ("No such file or directory") ([#16](https://github.com/mollie2999/Aurora/pull/16)) ([958e3b6](https://github.com/mollie2999/Aurora/commit/958e3b6e72f70ef651c38d10b8eb0860f5d5602a))

## [0.8.0](https://github.com/mollie2999/Aurora/compare/v0.7.2...v0.8.0) (2026-11-17)

**SteamInRust is now Aurora everywhere:** the program is `aurora`, the Windows setup `Aurora-Setup`, the Linux archive `Aurora-Linux-x86_64`, the Arch package `aurora-bin` (it replaces `steamonrust-bin`). Your settings, sign-ins and caches stay where they are, and installed copies update and move over by themselves ([f87388a](https://github.com/mollie2999/Aurora/commit/f87388a5f5f79ad8a00c12bdd11a433650439e1e))

Thanks to [madonchik123](https://github.com/madonchik123) for the clipboard and font fixes below, ported from upstream SteamInRust.

### New

* Windows: PlayStation, Switch, Steam Controller and other non-Xbox controllers work in games, like with Steam. Aurora reads them (SDL3) and games see them as Xbox pads, through Steam Input and XInput; rumble works. Button prompts stay Xbox's, and there's no gyro yet ([cb79247](https://github.com/mollie2999/Aurora/commit/cb79247b22178dd21a9bc5de686d1b422fdf950a))
* Linux: a tray icon (KDE, GNOME with the AppIndicator extension, Cinnamon, XFCE…) with the desktop's own menu. Closing the window keeps Aurora running in the tray; the icon brings it back, also on Wayland ([920412e](https://github.com/mollie2999/Aurora/commit/920412ef2f0eceb86d7ae8d67479754e0e5d8226))
* Steam install scripts run on a game's first launch, like Steam's "Running install script": the registry keys and setup steps some games need to start (e.g. EasyAntiCheat). On Linux inside the game's Proton prefix ([dcb5676](https://github.com/mollie2999/Aurora/commit/dcb56760c73c282b9b4b49593d2d00ef19bacacf))
* Store pages and game news are saved on disk: a game opened before shows at once, also after a restart or offline ([57e1d4a](https://github.com/mollie2999/Aurora/commit/57e1d4a65ce560f6f25c2b1d6e448f83241e269d))
* A right-click Cut / Copy / Paste menu in text fields. By [madonchik123](https://github.com/madonchik123), from [upstream #40](https://github.com/madonchik123/SteamInRust/pull/40) ([67800bd](https://github.com/mollie2999/Aurora/commit/67800bd28541453e52576e3a0f1681ff9c2859a9))
* A new Aurora logo ([b5c3e31](https://github.com/mollie2999/Aurora/commit/b5c3e3161dc25cfcdd0639f53e341f9f71dad559))
* Debian/Ubuntu package (`aurora_<version>_amd64.deb`) next to the Arch one ([467aa30](https://github.com/mollie2999/Aurora/commit/467aa30309c6fa998948902bbf645bbffe2a9a89))

### Faster and lighter

* Clicking a game in the library no longer freezes the window for a moment: how the game launches is worked out in the background, and game and workshop manifests are no longer read every frame ([59bda3d](https://github.com/mollie2999/Aurora/commit/59bda3d2281341d734ddf9a43035841ea71cad1b))
* Pictures are decoded at the size they're shown, a few at a time: a store page's pictures take a quarter of the memory (44.6 → 11.7 MB on Hades), the library's less than half ([7ea2cd3](https://github.com/mollie2999/Aurora/commit/7ea2cd37a683e3243a673c3f0bb12dfcd2cdeccc))
* Signing in only fetches the apps that changed since last time, instead of the whole library ([1ab86fe](https://github.com/mollie2999/Aurora/commit/1ab86fe7e65658a3bf78b178a3449acaff5c1575))
* Pictures and Steam's web API load over HTTP/2, on one connection ([c094472](https://github.com/mollie2999/Aurora/commit/c094472aa41e1ce57a5dd299422100732514f007))
* Windows: the fallback fonts for Chinese, Japanese, Korean and other scripts are memory-mapped instead of read whole: about 70 MB less memory. By [madonchik123](https://github.com/madonchik123), from [upstream #57](https://github.com/madonchik123/SteamInRust/pull/57) ([c7e7c37](https://github.com/mollie2999/Aurora/commit/c7e7c37aeafd184f38e456d99595ea3d9586eefa))
* Memory no longer grows as you open game pages: about a third less after browsing the library ([57e1d4a](https://github.com/mollie2999/Aurora/commit/57e1d4a65ce560f6f25c2b1d6e448f83241e269d))

### Fixes

* Paste, copy and cut (Ctrl+V / Ctrl+C / Ctrl+X) did nothing in any text field. By [madonchik123](https://github.com/madonchik123), from [upstream #40](https://github.com/madonchik123/SteamInRust/pull/40) ([67800bd](https://github.com/mollie2999/Aurora/commit/67800bd28541453e52576e3a0f1681ff9c2859a9))
* Non-Steam games launch: on Linux, Windows programs run with Proton and others are made executable; on Windows, shortcuts, quoted paths and games that need admin rights work ([dcb5676](https://github.com/mollie2999/Aurora/commit/dcb56760c73c282b9b4b49593d2d00ef19bacacf))
* Notifications start with a capital letter: "A game from your wishlist is on sale" ([28f5aee](https://github.com/mollie2999/Aurora/commit/28f5aeec2c722c8ecb7d0d6298a7698df4c91e6a))
* Linux: the app no longer needs GTK ([920412e](https://github.com/mollie2999/Aurora/commit/920412ef2f0eceb86d7ae8d67479754e0e5d8226))

## [0.7.2](https://github.com/mollie2999/Aurora/compare/v0.7.1-1...v0.7.2) (2026-11-16)

### New

* Linux: store trailers play, with sound. Needs `ffmpeg` installed ([f3f4a0a](https://github.com/mollie2999/Aurora/commit/f3f4a0a7bf2f3641d740f67fa5f9185ee3a2eb0a))
* Linux: Manage > Compatibility picks the Proton version for one game ([19918af](https://github.com/mollie2999/Aurora/commit/19918afd0dcb9935de477cdf2eac0acac9fc3218))
* Linux: games with a Linux build get "Native" in Manage > Compatibility, selected by default. Before, the menu said these games used the default Proton, though they already ran natively ([6561add](https://github.com/mollie2999/Aurora/commit/6561add8115aa964c829654f3ed99e57bc5f8362))

### Fixes

* The game right-click menu was about 600 px wide; it now fits its items ([3a857d8](https://github.com/mollie2999/Aurora/commit/3a857d84edbb7e5a666640c9c9f264d888cfcb3f))
* Library sidebar: the filter buttons and Filters now line up with the search box ([eda24e1](https://github.com/mollie2999/Aurora/commit/eda24e15f120c750623d0cabaaf637207d4141b9))

## [0.7.1-1](https://github.com/mollie2999/Aurora/compare/v0.7.1...v0.7.1-1) (2026-11-16)

**The GitHub page is now [mollie2999/Aurora](https://github.com/mollie2999/Aurora).**

### Fixes

* Updates: the update check uses the renamed repository, and a revision release like 0.7.1-1 now counts as newer than 0.7.1. Before, installed copies never offered 0.6.3-1 to 0.6.3
* Arch package: releases with a revision (0.7.1-1) now get a package too (pkgver 0.7.1, pkgrel 2)

## [0.7.1](https://github.com/mollie2999/Aurora/compare/v0.7.0...v0.7.1) (2026-11-16)

**Linux: shaders like Steam, and much less memory for the windowless Steam host.**

### New

* Linux: games now use Steam's shader cache (`steamapps/shadercache/<game>`), like when Steam starts them. Before, shaders Steam had pre-cached for your GPU were never used ([0e06ee7](https://github.com/mollie2999/Aurora/commit/0e06ee745c093991f85c48e7345747cd29506cd2))
* Linux: shaders are compiled before a game starts, like Steam's "Processing Vulkan shaders": the Play button shows "Compiling shaders… N%" with a Skip button. It runs once per game, and again only after a driver or cache update ([0e06ee7](https://github.com/mollie2999/Aurora/commit/0e06ee745c093991f85c48e7345747cd29506cd2))
* Settings > Downloads: "Allow downloads during gameplay" (off by default), shader pre-caching, and compiling shaders before a game starts. SteamInRust puts these into Steam's own settings before every game while Steam is closed; while it compiles shaders itself, Steam's background Vulkan shader processing is turned off so the work isn't done twice ([0e06ee7](https://github.com/mollie2999/Aurora/commit/0e06ee745c093991f85c48e7345747cd29506cd2))

### Fixes

* Linux: the windowless Steam host used 350–475 MB instead of about 150 MB while a game ran: with Steam's "Allow downloads during gameplay" on, it kept starting game updates and shader downloads in the background ([0e06ee7](https://github.com/mollie2999/Aurora/commit/0e06ee745c093991f85c48e7345747cd29506cd2))

## [0.7.0](https://github.com/mollie2999/Aurora/compare/v0.6.3...v0.7.0) (2026-11-16)

**Mostly Linux fixes.** Nothing changes for Windows in this release, so updating there is optional.

### New

* Arch Linux package on the release page: `sudo pacman -U steamonrust-bin-0.7.0-1-x86_64.pkg.tar.zst` (installs to `/opt/steamonrust`, adds an Aurora menu entry and the `steamonrust` command)
* Linux, experimental: Settings > Library can start anti-cheat games (Easy Anti-Cheat, BattlEye…) on the windowless Steam host instead of the official client. Off by default, and turning it on first warns about the risks (kicks, game or VAC bans). Tried with Helldivers 2 ([cd285ef](https://github.com/mollie2999/Aurora/commit/cd285ef1fa27bd446f443718d5f8fd887e936f19))

### Fixes

* Linux (X11): the window froze ("not responding") right after it opened, on the sign-in screen, e.g. on Linux Mint Cinnamon. The Steam Guard code prompt, the nickname dialog and the collection name dialog could freeze the same way. First shipped in 0.6.3-1 ([f9cd0e2](https://github.com/mollie2999/Aurora/commit/f9cd0e23c5bfe5029939c271648a1ad056b04e21))
* Linux: a game with Proton forced in Steam but only its Linux build installed (e.g. Dead Island Riptide Definitive Edition) started through the official Steam client; it now runs its Linux build on the windowless Steam host ([e27fa2b](https://github.com/mollie2999/Aurora/commit/e27fa2b264af2fc726addb3b757c15b998054b81))

## [0.5.0](https://github.com/madonchik123/SteamInRust/compare/v0.4.1...v0.5.0) (2026-11-10)


### New

* Steam-style cart: one Add to Cart, an Added to your cart popup, gifts per copy ([#35](https://github.com/madonchik123/SteamInRust/issues/35)) ([0b010ce](https://github.com/madonchik123/SteamInRust/commit/0b010ce1d757acbed42491d17b139fb56417d4df))
* Steam-style discount price tags ([#33](https://github.com/madonchik123/SteamInRust/issues/33)) ([f748477](https://github.com/madonchik123/SteamInRust/commit/f748477237465e2a68bb2e2fd441b94b9f31c302))


### Fixes

* cancelling a download frees its disk space ([#36](https://github.com/madonchik123/SteamInRust/issues/36)) ([b147abe](https://github.com/madonchik123/SteamInRust/commit/b147abe9cc01ee0c5ff4725632b5ad334dfb89b9))
* downloads, updates and verify work like Steam ([#37](https://github.com/madonchik123/SteamInRust/issues/37)) ([7755bb9](https://github.com/madonchik123/SteamInRust/commit/7755bb9df9e67842201a59536b0cfdb717ea69dd))
* library filters fit, cart names, Back in the store, free games, tag search ([#32](https://github.com/madonchik123/SteamInRust/issues/32)) ([42e188b](https://github.com/madonchik123/SteamInRust/commit/42e188b410c131fcf1a08a0c95f455016ce5fe2c))

## [0.4.1](https://github.com/madonchik123/SteamInRust/compare/v0.4.0...v0.4.1) (2026-11-09)


### Fixes

* readable update dialog, hover states in menus, no update prompts for builds from source ([#30](https://github.com/madonchik123/SteamInRust/issues/30)) ([8a60364](https://github.com/madonchik123/SteamInRust/commit/8a60364f8b570e21c5512071e0fa0a7df77066dd))

## [0.4.0](https://github.com/madonchik123/SteamInRust/compare/v0.3.0...v0.4.0) (2026-11-08)


### New

* compact Workshop filters; items download when subscribed ([#17](https://github.com/madonchik123/SteamInRust/issues/17)) ([7562a6d](https://github.com/madonchik123/SteamInRust/commit/7562a6d30e28e0814f6cf5b2d9c45cfe01bd1056))
* ISteamNetworkingSockets / Messages on Valve's protocol (certs, rendezvous, SNP, ICE) ([#22](https://github.com/madonchik123/SteamInRust/issues/22)) ([417a283](https://github.com/madonchik123/SteamInRust/commit/417a283eae3740a51475a6c5d73cb1a9b087424b))
* live player count on game pages; easy 32-bit DLL for dev builds ([#14](https://github.com/madonchik123/SteamInRust/issues/14)) ([19cebea](https://github.com/madonchik123/SteamInRust/commit/19cebea4a5cdf19bc230e2b9650cc37bf679ed96))
* native account details, purchase history, licenses and Steam Family ([#18](https://github.com/madonchik123/SteamInRust/issues/18)) ([1a119b5](https://github.com/madonchik123/SteamInRust/commit/1a119b59390b5773733f87f997377fe090820e86))
* P2P with official-client players (ISteamNetworking over Valve's libjingle protocol) ([#21](https://github.com/madonchik123/SteamInRust/issues/21)) ([57a2f56](https://github.com/madonchik123/SteamInRust/commit/57a2f56fd64b18594f00e8ea1c283c7f655c8ba5))
* real Steam services in our steamclient (sockets, Workshop, inventory, server browser, screenshots, Steam Input) ([#20](https://github.com/madonchik123/SteamInRust/issues/20)) ([4ab1c87](https://github.com/madonchik123/SteamInRust/commit/4ab1c87a2312b247dee2475df014f085ce600514))
* Shift+Tab overlay drawn inside the game; Steam-style invites in chat ([#19](https://github.com/madonchik123/SteamInRust/issues/19)) ([62b93ea](https://github.com/madonchik123/SteamInRust/commit/62b93eaaa61de5624e2eb3e6dcfe860d5b873bd2))


### Fixes

* 32-bit games on older Steamworks SDKs crashed at start (Human Fall Flat) ([#15](https://github.com/madonchik123/SteamInRust/issues/15)) ([4b9dd49](https://github.com/madonchik123/SteamInRust/commit/4b9dd49aa9e2ce9098c9420326f6a7b80e7f2655))
* uninstalling no longer freezes or crashes the app ([#23](https://github.com/madonchik123/SteamInRust/issues/23)) ([cf676e8](https://github.com/madonchik123/SteamInRust/commit/cf676e8103fbf43371bb491114263aa7e3195121))


### Faster

* library images no longer fetch their fallback while loading ([#26](https://github.com/madonchik123/SteamInRust/issues/26)) ([eacdedd](https://github.com/madonchik123/SteamInRust/commit/eacdedd9c1c18e6f71bbab59437f4043f91772ad))
* the library shows at once instead of "Loading your games" ([#27](https://github.com/madonchik123/SteamInRust/issues/27)) ([b2d7170](https://github.com/madonchik123/SteamInRust/commit/b2d71701990a1cfa97a0499ab89df9da4223b307))

## [0.3.0](https://github.com/madonchik123/SteamInRust/compare/v0.2.0...v0.3.0) (2026-11-08)


### New

* install states, download activity, friends window and tray polish ([#10](https://github.com/madonchik123/SteamInRust/issues/10)) ([29d9fa1](https://github.com/madonchik123/SteamInRust/commit/29d9fa114ec9c2a4d1bea441aa8f0fa19f1a705f))
* online play in our steamclient (lobbies, P2P, 32-bit games, leaderboards, HTTP) ([#11](https://github.com/madonchik123/SteamInRust/issues/11)) ([e289050](https://github.com/madonchik123/SteamInRust/commit/e289050fcdf4778fa8c5f07651ba212e0d4043db))
* Steam-style Friends & Chat menu, own title bar; fix stuck downloads ([#9](https://github.com/madonchik123/SteamInRust/issues/9)) ([70f4eba](https://github.com/madonchik123/SteamInRust/commit/70f4eba0348a9dd8445e81ed88f35ee3a3ac04b2))


### Fixes

* crash from a negative progress bar width during downloads ([#7](https://github.com/madonchik123/SteamInRust/issues/7)) ([d2e3a3e](https://github.com/madonchik123/SteamInRust/commit/d2e3a3e55f9b6b4bafbb6b0ad218bdcfe7688cca))

## [0.2.0](https://github.com/madonchik123/SteamInRust/compare/v0.1.1...v0.2.0) (2026-11-08)


### New

* gifts for games you own, picture cache, store polish ([#5](https://github.com/madonchik123/SteamInRust/issues/5)) ([04f4c4c](https://github.com/madonchik123/SteamInRust/commit/04f4c4cc9c1ddddca75ca47b6d2203dfa254e3f9))

## [0.1.1](https://github.com/madonchik123/SteamInRust/compare/v0.1.0...v0.1.1) (2026-11-07)


### Fixes

* single slashes in Steam Cloud file names for Auto-Cloud folders ([af8a0ac](https://github.com/madonchik123/SteamInRust/commit/af8a0ac4656953171c046c48938dbebb24c40d03))
