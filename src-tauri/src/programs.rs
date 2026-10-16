//! Telling which program a process is running, on Windows and on Linux, where
//! a Windows game runs under Wine or Proton.
//!
//! Deliberately free of the rest of the crate, so it builds on its own: the
//! Linux side can only be checked on Linux, against a real process.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use sysinfo::{Process, ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

/// A program running now, offered so its path can be picked instead of typed.
#[derive(Debug, serde::Serialize)]
pub struct RunningProgram {
    pub name: String,
    pub path: String,
}

/// Every program running now, once each, by name.
pub fn running_programs() -> Vec<RunningProgram> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, program_refresh());

    let paths: BTreeSet<PathBuf> = system
        .processes()
        .values()
        .filter_map(program_path)
        .collect();
    let mut programs: Vec<RunningProgram> = paths
        .into_iter()
        .map(|path| RunningProgram {
            name: path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
            path: path.to_string_lossy().into_owned(),
        })
        .collect();

    programs.sort_by_cached_key(|program| (program.name.to_lowercase(), program.path.clone()));
    programs
}

/// What a scan has to read to tell which program a process is running.
pub fn program_refresh() -> ProcessRefreshKind {
    let kind = ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet);

    // Only Wine's command line names a program, and Wine rewrites it once it
    // is up, so on Linux it is read on every scan rather than cached from a
    // first look that may predate the rewrite.
    if cfg!(target_os = "linux") {
        kind.with_cmd(UpdateKind::Always)
    } else {
        kind
    }
}

/// The program a process is running: its executable, or, for a Windows
/// program run by Wine or Proton on Linux, the program itself.
///
/// Wine's executable is its own loader, the same file for every game, so it
/// says nothing about which one is running. Wine does rewrite the command line
/// to the program's Windows path, though, where the Linux root is drive `Z:`.
pub fn program_path(process: &Process) -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        let wine_program = process
            .cmd()
            .first()
            .and_then(|arg| arg.to_str())
            .and_then(wine_unix_path);

        if wine_program.is_some() {
            return wine_program;
        }
    }

    process.exe().map(Path::to_path_buf)
}

/// The Linux path of a Windows program Wine names as `Z:\...\name.exe`.
///
/// Only drive `Z:`, which is the Linux root, has a fixed meaning; the others
/// depend on the Wine prefix. Anything after the `.exe` is taken for
/// arguments, in case Wine leaves the whole command line in one argument.
fn wine_unix_path(command: &str) -> Option<PathBuf> {
    let command = command.trim_start_matches('"');
    let rest = command
        .strip_prefix("Z:\\")
        .or_else(|| command.strip_prefix("z:\\"))?;
    let end = rest.to_ascii_lowercase().find(".exe")? + ".exe".len();

    Some(PathBuf::from(format!(
        "/{}",
        rest[..end].replace('\\', "/")
    )))
}

/// A configured program, recognised by its file name.
///
/// The name rather than the whole path, so the program is found wherever it
/// is installed: another Steam library or drive, a path through a symlink, or
/// a Linux config still holding the Windows default. The path itself stays in
/// the config, because launching needs it.
pub struct ConfiguredProgram {
    name: Option<String>,
}

impl ConfiguredProgram {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let name = file_name(&path.as_ref().to_string_lossy()).map(str::to_owned);

        Self { name }
    }

    pub fn runs_in(&self, process: &Process) -> bool {
        // The name the system gives the process stands in when its executable
        // cannot be read.
        let found = program_path(process).map_or_else(
            || process.name().to_string_lossy().into_owned(),
            |path| path.to_string_lossy().into_owned(),
        );

        file_name(&found).is_some_and(|name| self.is_named(name))
    }

    fn is_named(&self, name: &str) -> bool {
        // Windows ignores case in file names, and two programs differing only
        // in case are not worth telling apart on Linux.
        self.name
            .as_deref()
            .is_some_and(|own| own.eq_ignore_ascii_case(name))
    }
}

/// The last part of a path written with either separator, so a Windows path
/// names its file on Linux too. `None` for an empty path or a folder.
fn file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{ConfiguredProgram, file_name, running_programs, wine_unix_path};
    use std::path::{Path, PathBuf};

    const KOVAAKS: &str = r"Z:\home\bassel\.local\share\Steam\steamapps\common\FPSAimTrainer\FPSAimTrainer\Binaries\Win64\FPSAimTrainer-Win64-Shipping.exe";

    #[test]
    fn a_wine_z_drive_path_is_the_linux_path() {
        assert_eq!(
            wine_unix_path(KOVAAKS),
            Some(PathBuf::from(
                "/home/bassel/.local/share/Steam/steamapps/common/FPSAimTrainer/FPSAimTrainer/Binaries/Win64/FPSAimTrainer-Win64-Shipping.exe"
            ))
        );
    }

    /// Spaces belong to the path; what follows the `.exe` does not.
    #[test]
    fn arguments_after_the_exe_are_not_part_of_the_path() {
        assert_eq!(
            wine_unix_path(r#""Z:\games\My Game\Game.EXE" -windowed"#),
            Some(PathBuf::from("/games/My Game/Game.EXE"))
        );
    }

    /// `C:` and the rest live inside a Wine prefix, which the path alone
    /// does not give; a Linux command line is not Wine's at all.
    #[test]
    fn other_command_lines_name_no_program() {
        assert_eq!(wine_unix_path(r"C:\windows\system32\steam.exe"), None);
        assert_eq!(wine_unix_path("/usr/bin/obs"), None);
        assert_eq!(wine_unix_path(r"Z:\home\bassel\notes.txt"), None);
    }

    /// Either separator, so a Windows default left in a Linux config still
    /// names its program.
    #[test]
    fn a_path_names_its_file_on_any_system() {
        assert_eq!(
            file_name(r"C:\Program Files\obs-studio\bin\64bit\obs64.exe"),
            Some("obs64.exe")
        );
        assert_eq!(file_name("/usr/bin/obs"), Some("obs"));
        assert_eq!(file_name(r"C:\Games\"), None);
        assert_eq!(file_name(""), None);
    }

    /// Another library, drive or system: the same program all the same.
    #[test]
    fn a_program_is_known_by_its_name_wherever_it_is() {
        let program = ConfiguredProgram::new(
            r"C:\Program Files (x86)\Steam\steamapps\common\FPSAimTrainer\FPSAimTrainer\Binaries\Win64\FPSAimTrainer-Win64-Shipping.exe",
        );

        assert!(program.is_named("FPSAimTrainer-Win64-Shipping.exe"));
        assert!(program.is_named("fpsaimtrainer-win64-shipping.exe"));
        assert!(!program.is_named("Aimbeast-Win64-Shipping.exe"));
    }

    #[test]
    fn an_unset_path_matches_nothing() {
        assert!(!ConfiguredProgram::new("").is_named(""));
        assert!(!ConfiguredProgram::new("").is_named("obs64.exe"));
    }

    /// The real process list, read the way detection reads it: this test is
    /// found by its own name under a folder it is not in, and nothing else is.
    #[test]
    fn this_process_is_found_by_its_name_from_anywhere() {
        let this = std::env::current_exe().expect("own path");
        let name = this.file_name().expect("a file").to_string_lossy();

        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            super::program_refresh(),
        );
        let own = system
            .process(sysinfo::get_current_pid().expect("own pid"))
            .expect("own process");

        assert!(ConfiguredProgram::new(format!(r"D:\elsewhere\{name}")).runs_in(own));
        assert!(ConfiguredProgram::new(format!("/elsewhere/{name}")).runs_in(own));
        assert!(!ConfiguredProgram::new(r"D:\elsewhere\not-this.exe").runs_in(own));
    }

    /// The real process list, read the way the picker reads it, holds the
    /// test itself.
    #[test]
    fn the_running_programs_include_this_one() {
        let this = std::env::current_exe().expect("own path");
        let programs = running_programs();

        assert!(
            programs
                .iter()
                .any(|program| Path::new(&program.path) == this),
            "{this:?} not in {} programs",
            programs.len()
        );
    }
}
