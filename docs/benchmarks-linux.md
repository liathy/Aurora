# Linux benchmarks

Aurora 0.7.2 against the official Steam client on Linux, 16 November 2026.

## Setup

- AMD Ryzen 7 5700X, 16 GB RAM, NVIDIA GeForce RTX 5070 (driver 620.70.06)
- CachyOS, kernel 7.3.4, GNOME on Wayland
- Steam client build 1792618285 (Arch `steam` 1.0.0.87), updates off.
  The Millennium mod installed on this PC was unhooked for Steam's runs.
- Aurora 0.7.2 release build
- Same Steam account, signed in automatically by both clients

## Method

Script: [benchmarks-linux.sh](benchmarks-linux.sh). One client at a time, never both.
Each client ran 4 times; run 0 is a warm-up and isn't counted. Figures in the
README are the median of runs 1–3.

- **Start → first window / main window.** From launch to the window
  appearing. Aurora runs on XWayland here and is found with `xdotool`; its
  first window is the "Who's playing?" account picker, which the script clicks
  as soon as it shows. Steam's windows are native Wayland windows, so its own
  log line (`webhelper.txt`: `SP DesktopLoginWindow_uid0… WasHidden 0` for the
  sign-in window, `SP Desktop_uid0-'Steam': WasHidden 0` for the main window)
  is the signal, checked every 50 ms.
- **Memory.** 45 s after the main window opened: the sum of PSS
  (`/proc/<pid>/smaps_rollup`) over every process the client started,
  including Steam's container and runtime loggers.
- **CPU.** User + system time of those processes over the next 30 s, as a
  percentage of one core.
- **Processes.** The processes counted for memory.
- **Program size.** Steam: `~/.local/share/Steam` without `steamapps`,
  `userdata`, `appcache`, `depotcache`, `config`, `logs`, `package` (update
  downloads), `compatibilitytools.d` and the Millennium themes. Aurora: the
  release folder (`aurora`, `steamhost`, the two `steamclient` DLLs).

## Every run

| Client | Run | First window (s) | Main window (s) | PSS (MB) | Processes | CPU (% of a core) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Steam | 0 (warm-up) | 2.86 | 5.45 | 1,408 | 16 | 18.53 |
| Steam | 1 | 2.77 | 4.89 | 1,408 | 16 | 18.27 |
| Steam | 2 | 2.77 | 4.90 | 1,412 | 16 | 8.93 |
| Steam | 3 | 2.93 | 5.06 | 1,407 | 16 | 32.50 |
| Aurora | 0 (warm-up) | 0.14 | 14.08 | 161 | 1 | 0.10 |
| Aurora | 1 | 0.14 | 0.99 | 169 | 1 | 0.07 |
| Aurora | 2 | 0.14 | 2.35 | 166 | 1 | 0.20 |
| Aurora | 3 | 0.15 | 2.16 | 162 | 1 | 0.40 |

## Not measured

- Memory while a game runs. Aurora starts the windowless Steam host for
  native and Steam-DRM games; Steam's own processes keep running beside a game.
- Page-switching speed, download speed, and Wayland-native Aurora (it ran on
  XWayland so the script could find its window).
