//! Games installed under a path with a colon, like a drive mounted at its
//! by-id name (`/mnt/usb-TOSHIBA_…-0:0-part1`). Unity's Mono splits its
//! search paths on ':' and gives up ("The assembly mscorlib.dll was not
//! found"), so Cities: Skylines and other Mono games can't start there, with
//! any launcher.
//!
//! Such a game starts in a private mount namespace: an unprivileged user
//! namespace mapping the player to themselves (no root, no new rights), in
//! which the folder holding the colon also appears at a colon-free path under
//! Aurora's data folder. The game's program, arguments, working folder and
//! the variables we set point there; everything else is unchanged and
//! nothing outside the game's processes sees the extra mount. Where user
//! namespaces aren't allowed, the game starts as before.

use std::ffi::{CString, OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The deepest folder on `path` (itself included) whose name has a colon:
/// everything below it is colon-free.
pub(super) fn colon_folder(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|a| a.file_name().is_some_and(|n| n.as_bytes().contains(&b':'))).map(Path::to_path_buf)
}

/// `cmd`, set to run with the colon folder of `install_dir` reachable at a
/// colon-free path; `cmd` itself when there's no colon or it can't be done.
pub(super) fn redirect(cmd: Command, install_dir: &Path) -> Command {
    let Some(folder) = colon_folder(install_dir) else { return cmd };
    let Some(alias) = alias_for(&folder) else { return cmd };
    // Try the namespace once with a no-op first: if this system forbids it,
    // keep the plain command (the game may still start).
    let mut probe = Command::new("true");
    if let Err(e) = enter_namespace(&mut probe, &folder, &alias, None).and_then(|()| probe.status().map_err(|e| e.to_string())).and_then(|s| if s.success() { Ok(()) } else { Err(s.to_string()) }) {
        tracing::warn!("{} has a colon in its path and a private mount isn't possible here ({e}); starting it as is", folder.display());
        return cmd;
    }
    let mut out = Command::new(swap(cmd.get_program(), &folder, &alias));
    out.args(cmd.get_args().map(|a| swap(a, &folder, &alias)));
    for (k, v) in cmd.get_envs() {
        match v {
            Some(v) => out.env(k, swap(v, &folder, &alias)),
            None => out.env_remove(k),
        };
    }
    // The working folder is entered after the mount (std would enter it
    // before, when the colon-free path doesn't exist yet).
    let cwd = cmd.get_current_dir().map(|d| PathBuf::from(swap(d.as_os_str(), &folder, &alias)));
    // Every game command here gets its own process group (see `kill_tree`).
    out.process_group(0);
    if enter_namespace(&mut out, &folder, &alias, cwd.as_deref()).is_err() {
        return cmd;
    }
    tracing::info!("{} has a colon in its path: the game sees it at {}", folder.display(), alias.display());
    out
}

/// A stable, colon-free mount point for `folder` in Aurora's data folder.
fn alias_for(folder: &Path) -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("dev", "SteamInRust", "SteamInRust")?.data_dir().join("mnt");
    let hash = folder.as_os_str().as_bytes().iter().fold(0x811c_9dc5u32, |h, b| (h ^ *b as u32).wrapping_mul(0x0100_0193));
    let alias = base.join(format!("{hash:08x}"));
    if alias.as_os_str().as_bytes().contains(&b':') {
        return None;
    }
    std::fs::create_dir_all(&alias).ok()?;
    Some(alias)
}

/// `value` with every occurrence of `folder` replaced by `alias`.
fn swap(value: &OsStr, folder: &Path, alias: &Path) -> OsString {
    let (v, from, to) = (value.as_bytes(), folder.as_os_str().as_bytes(), alias.as_os_str().as_bytes());
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        if v[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(v[i]);
            i += 1;
        }
    }
    OsString::from_vec(out)
}

/// In the child, just before the program starts: a user namespace mapping
/// us to ourselves, a mount namespace of our own, `folder` bound at `alias`
/// in it, then the working folder. Only async-signal-safe calls after the fork.
fn enter_namespace(cmd: &mut Command, folder: &Path, alias: &Path, cwd: Option<&Path>) -> Result<(), String> {
    let c = |p: &Path| CString::new(p.as_os_str().as_bytes()).map_err(|e| e.to_string());
    let (src, dst) = (c(folder)?, c(alias)?);
    let cwd = cwd.map(c).transpose()?;
    // SAFETY: plain libc getters.
    let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
    let uid_map = format!("{uid} {uid} 1");
    let gid_map = format!("{gid} {gid} 1");
    // SAFETY: the closure only makes raw system calls on data prepared above.
    unsafe {
        cmd.pre_exec(move || {
            let fail = || Err(std::io::Error::last_os_error());
            if libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) != 0 {
                return fail();
            }
            write_proc(c"/proc/self/setgroups", b"deny")?;
            write_proc(c"/proc/self/uid_map", uid_map.as_bytes())?;
            write_proc(c"/proc/self/gid_map", gid_map.as_bytes())?;
            // Our mounts stay ours.
            if libc::mount(std::ptr::null(), c"/".as_ptr(), std::ptr::null(), libc::MS_REC | libc::MS_PRIVATE, std::ptr::null()) != 0 {
                return fail();
            }
            if libc::mount(src.as_ptr(), dst.as_ptr(), std::ptr::null(), libc::MS_BIND | libc::MS_REC, std::ptr::null()) != 0 {
                return fail();
            }
            if let Some(dir) = &cwd {
                if libc::chdir(dir.as_ptr()) != 0 {
                    return fail();
                }
            }
            Ok(())
        });
    }
    Ok(())
}

/// open/write/close, nothing that allocates.
fn write_proc(path: &std::ffi::CStr, data: &[u8]) -> std::io::Result<()> {
    // SAFETY: a NUL-terminated path and a byte buffer that outlive the calls.
    unsafe {
        let fd = libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let n = libc::write(fd, data.as_ptr().cast(), data.len());
        let err = std::io::Error::last_os_error();
        libc::close(fd);
        if n < 0 { Err(err) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_folder_with_the_colon() {
        let p = Path::new("/mnt/usb-TOSHIBA-0:0-part1/SteamLibrary/steamapps/common/Cities_Skylines");
        assert_eq!(colon_folder(p).as_deref(), Some(Path::new("/mnt/usb-TOSHIBA-0:0-part1")));
        assert_eq!(colon_folder(Path::new("/a:b/c/d:e/game")).as_deref(), Some(Path::new("/a:b/c/d:e")));
        assert_eq!(colon_folder(Path::new("/home/me/Games/x")), None);
        let swapped = swap(OsStr::new("--gameDir=/mnt/u:1/lib/x"), Path::new("/mnt/u:1"), Path::new("/data/mnt/abc"));
        assert_eq!(swapped, OsString::from("--gameDir=/data/mnt/abc/lib/x"));
    }

    /// A program under a colon path runs, from the colon-free path.
    #[test]
    fn runs_a_game_from_a_colon_free_path() {
        let base = std::env::temp_dir().join(format!("sir-colon-{}", std::process::id()));
        let game = base.join("usb-0:0-part1").join("steamapps").join("common").join("Game");
        std::fs::create_dir_all(&game).unwrap();
        let script = game.join("run.sh");
        std::fs::write(&script, "#!/bin/sh\nprintf '%s|%s|%s' \"$(pwd)\" \"$0\" \"$GAME_DIR\"\n").unwrap();
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let mut cmd = Command::new(&script);
        cmd.current_dir(&game).env("GAME_DIR", &game);
        let mut out = redirect(cmd, &game);
        let text = String::from_utf8(out.output().unwrap().stdout).unwrap();
        let alias = alias_for(&base.join("usb-0:0-part1")).unwrap();
        let _ = std::fs::remove_dir_all(&base);
        let expected = alias.join("steamapps/common/Game");
        assert_eq!(text, format!("{0}|{0}/run.sh|{0}", expected.display()), "{text}");
        assert!(!text.contains(':'));
    }

    /// With a launch-options wrapper (`env %command%`) the mount still
    /// applies: it's added after the wrapper.
    #[test]
    fn works_with_a_launch_options_wrapper() {
        let base = std::env::temp_dir().join(format!("sir-colon-w-{}", std::process::id()));
        let game = base.join("drive-1:2").join("Game");
        std::fs::create_dir_all(&game).unwrap();
        let script = game.join("run.sh");
        std::fs::write(&script, "#!/bin/sh\nprintf '%s|%s' \"$(pwd)\" \"$FLAG\"\n").unwrap();
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let opts = super::super::UserOptions::parse("FLAG=on env %command%");
        let mut cmd = Command::new(&script);
        cmd.current_dir(&game);
        let mut out = redirect(opts.wrap(cmd), &game);
        let text = String::from_utf8(out.output().unwrap().stdout).unwrap();
        let alias = alias_for(&base.join("drive-1:2")).unwrap();
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(text, format!("{}|on", alias.join("Game").display()), "{text}");
    }
}

/// The real case: Cities: Skylines (Unity 5 Mono) on the colon-named
/// drive. Started as is, Mono gives up at once (exit code 1, "mscorlib.dll
/// was not found"); redirected, it gets past that (without Steam running it
/// then quits by itself, exit code 0). `AURORA_TEST_MONO_GAME=<game folder>`,
/// a display, and the native build installed.
#[cfg(test)]
#[test]
#[ignore]
fn mono_game_starts_from_a_colon_path() {
    let Ok(game) = std::env::var("AURORA_TEST_MONO_GAME") else { return };
    let game = Path::new(&game);
    let run = |redirected: bool| {
        let mut cmd = Command::new(game.join("Cities.x64"));
        cmd.current_dir(game).process_group(0).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let mut child = if redirected { redirect(cmd, game) } else { cmd }.spawn().unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let code = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s.code();
            }
            if std::time::Instant::now() > end {
                let _ = std::process::Command::new("kill").args(["-KILL", &format!("-{}", child.id())]).status();
                break None; // still running: past Mono for sure
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        code
    };
    assert_eq!(run(false), Some(1), "expected Mono to fail from the colon path");
    assert_ne!(run(true), Some(1), "Mono still failed when redirected");
}
