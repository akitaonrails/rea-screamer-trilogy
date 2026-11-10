//! Source game directory discovery, read-only runtime staging, binary patching,
//! save-game synchronization, and `dosbox-staging` invocation.

use std::fs;
use std::path::{Path, PathBuf};

use rea_launcher::fsx;
use rea_launcher::paths::{self, native, normalize};
use rea_launcher::runtime::{self, Policy, Stamp, SyncReport};
use rea_launcher::saves::{FirstRun, Mirror};

use crate::config::{RendererChoice, Settings, TextureFilter};
use crate::edition::{
    self, Edition, Game, Renderer, S2_GLIDE2X_DOS_SHA256, S2_GLIDE2X_WIN_STUB_SHA256,
};
use crate::game_cfg::{
    DosboxConfOptions, KeyBindings, build_controls_cfg, build_dosbox_conf, build_screamer_cfg,
    build_setup_cfg,
};
use crate::le::Image;

/// A matched executable inside a user's game directory.
#[derive(Debug, Clone)]
pub struct DiscoveredExe {
    pub path: PathBuf,
    pub file_name: String,
    pub edition: &'static Edition,
    pub sha256: [u8; 32],
}

/// Where players report a build the launcher does not know (the issue
/// template asks for `launcher.log` and the hash).
pub const ISSUE_URL: &str =
    "https://github.com/akitaonrails/rea-screamer-trilogy/issues/new?template=unknown-edition.yml";

/// The bug report form (`.github/ISSUE_TEMPLATE/bug-report.yml`).
pub const BUG_URL: &str =
    "https://github.com/akitaonrails/rea-screamer-trilogy/issues/new?template=bug-report.yml";

/// A failed `--check`'s message with where to report it, unless it already
/// names a form (an unknown build links the unknown-edition form).
pub fn with_bug_link(message: String) -> String {
    if message.contains("/issues/new") {
        message
    } else {
        format!("{message}\nIf this looks wrong, open a bug report with launcher.log: {BUG_URL}")
    }
}

/// A game executable (by its name) whose SHA-256 is in no edition table:
/// never patched or run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownExe {
    pub path: PathBuf,
    pub file_name: String,
    pub sha256: [u8; 32],
}

impl UnknownExe {
    /// What the player reads: the file, its SHA-256 and where to report it.
    pub fn message(&self) -> String {
        format!(
            "{} ({}) is a build this launcher does not know, SHA-256 {}. It is not used. Please report it with launcher.log: {ISSUE_URL}",
            self.file_name,
            self.path.display(),
            rea_launcher::identify::hex(&self.sha256)
        )
    }
}

/// Result of scanning a user's game folder.
#[derive(Debug, Clone)]
pub struct DiscoveredGame {
    pub root_dir: PathBuf,
    pub exe_dir: PathBuf,
    pub game: Game,
    pub executables: Vec<DiscoveredExe>,
    pub cd_image: Option<PathBuf>,
    /// Why no CD image will be mounted, for `launcher.log` and `--check`.
    pub cd_warning: Option<String>,
    pub glide2x_ovl_source: Option<PathBuf>,
    pub has_genuine_dos_glide2x: bool,
    pub has_windows_stub_glide2x: bool,
    /// Executables with a known name but an unknown build, left out.
    pub unknown: Vec<UnknownExe>,
}

/// Result of preparing a writable runtime folder for launch.
#[derive(Debug, Clone)]
pub struct PreparedRuntime {
    pub runtime_dir: PathBuf,
    pub conf_path: PathBuf,
    pub selected_exe: String,
    pub selected_edition: &'static Edition,
    pub patched_exes: Vec<(String, usize)>,
    /// Problems worth logging that did not stop the launch.
    pub warnings: Vec<String>,
    pub plan: LaunchPlan,
}

const KNOWN_EXE_NAMES: &[&str] = &[
    "STARTH.EXE",
    "STARTL.EXE",
    "SETUP.EXE",
    "S2_3DFX.EXE",
    "START65H.EXE",
    "STRT3FX.EXE",
    "ST.EXE",
];

/// Scans `dir` (and common GOG / ISO subdirectories such as `app/` or `hdd/SR/`)
/// without modifying any file in `dir`.
pub fn scan_game_dir(dir: &Path) -> Result<DiscoveredGame, String> {
    if !dir.exists() {
        return Err(format!("Game directory does not exist: {}", dir.display()));
    }

    let candidate_dirs = [
        dir.to_path_buf(),
        dir.join("app"),
        dir.join("hdd").join("SR"),
        dir.join("SR"),
    ];

    let mut exe_dir = None;
    let mut executables = Vec::new();
    let mut unknown = Vec::new();

    for cand in &candidate_dirs {
        if !cand.is_dir() {
            continue;
        }
        let (found, strangers) = scan_exes_in_dir(cand)?;
        if !found.is_empty() {
            exe_dir = Some(cand.clone());
            executables = found;
            unknown = strangers;
            break;
        }
        unknown.extend(strangers);
    }

    let exe_dir = exe_dir.ok_or_else(|| match unknown.first() {
        Some(stranger) => stranger.message(),
        None => format!(
            "No supported Screamer, Screamer 2, or Screamer Rally executables found in {}",
            dir.display()
        ),
    })?;

    // Determine primary game from main racing executables first, falling back to setup.
    let game = executables
        .iter()
        .find(|e| e.edition.role != Renderer::Setup)
        .or_else(|| executables.first())
        .map(|e| e.edition.game)
        .ok_or_else(|| "No recognized edition found".to_string())?;

    let (has_genuine_dos_glide2x, has_windows_stub_glide2x, glide2x_ovl_source) =
        check_glide2x_ovl(dir, &exe_dir);
    let cd = find_cd_image(dir, &exe_dir);

    Ok(DiscoveredGame {
        root_dir: dir.to_path_buf(),
        exe_dir,
        game,
        executables,
        cd_image: cd.image.clone(),
        cd_warning: cd.warning(game, dir),
        glide2x_ovl_source,
        has_genuine_dos_glide2x,
        has_windows_stub_glide2x,
        unknown,
    })
}

/// The known builds in `dir`, and the files with a game executable's name
/// but an unknown SHA-256.
fn scan_exes_in_dir(dir: &Path) -> Result<(Vec<DiscoveredExe>, Vec<UnknownExe>), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
    let mut unknown = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let upper = name.to_ascii_uppercase();
        if !KNOWN_EXE_NAMES.contains(&upper.as_str()) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let digest = edition::sha256(&bytes);
        match edition::identify_sha256(&digest) {
            Some(ed) => out.push(DiscoveredExe {
                path,
                file_name: upper,
                edition: ed,
                sha256: digest,
            }),
            None => unknown.push(UnknownExe {
                path,
                file_name: upper,
                sha256: digest,
            }),
        }
    }
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    unknown.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok((out, unknown))
}

fn check_glide2x_ovl(root_dir: &Path, exe_dir: &Path) -> (bool, bool, Option<PathBuf>) {
    let mut has_win_stub = false;
    if let Ok(entries) = fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.eq_ignore_ascii_case("glide2x.ovl")
                && let Ok(bytes) = fs::read(&path)
            {
                let d = edition::sha256(&bytes);
                if d == S2_GLIDE2X_DOS_SHA256 || bytes.len() > 100_000 {
                    return (true, false, Some(path));
                }
                if d == S2_GLIDE2X_WIN_STUB_SHA256 || bytes.len() < 50_000 {
                    has_win_stub = true;
                }
            }
        }
    }

    // Check explicit SCREAMER_GLIDE2X_OVL env var or sibling Screamer Rally installation.
    let mut fallback_candidates = Vec::new();
    if let Ok(env_ovl) = std::env::var("SCREAMER_GLIDE2X_OVL") {
        fallback_candidates.push(PathBuf::from(env_ovl));
    }
    for anc in [root_dir.parent(), exe_dir.parent().and_then(|p| p.parent())]
        .into_iter()
        .flatten()
    {
        fallback_candidates.push(
            anc.join("screamer-rally")
                .join("hdd")
                .join("SR")
                .join("GLIDE2X.OVL"),
        );
        fallback_candidates.push(anc.join("screamer-rally").join("SR").join("GLIDE2X.OVL"));
        fallback_candidates.push(anc.join("screamer-rally").join("GLIDE2X.OVL"));
    }
    for cand in fallback_candidates {
        if cand.is_file()
            && let Ok(bytes) = fs::read(&cand)
            && (edition::sha256(&bytes) == S2_GLIDE2X_DOS_SHA256 || bytes.len() > 100_000)
        {
            return (true, has_win_stub, Some(cand));
        }
    }

    (false, has_win_stub, None)
}

/// Raw `.bin`/`.img` files smaller than this are not CD images (a 1.44 MB
/// floppy image, a data file); the smallest Screamer CD is hundreds of MB.
const MIN_RAW_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

/// What the CD image search found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdSearch {
    /// The image (or cue sheet) to mount on `D:`.
    pub image: Option<PathBuf>,
    /// Cue sheets passed over because a file they name is missing: DOSBox
    /// could not mount them.
    pub broken: Vec<PathBuf>,
}

impl CdSearch {
    /// What to tell the player when no usable image was found: the game
    /// will stop at its "insert the CD" screen.
    pub fn warning(&self, game: Game, root_dir: &Path) -> Option<String> {
        if self.image.is_some() {
            return None;
        }
        let mut note = format!(
            "No CD image found for {} in {} or its subfolders, so the game will ask for its CD. \
             Put the CD image (a .cue sheet with its .bin, an .iso, or GOG's .dat and .gog) in the game folder or a folder inside it.",
            game.short_title(),
            root_dir.display()
        );
        for cue in &self.broken {
            let missing: Vec<String> = cd_image_files(Some(cue))
                .into_iter()
                .skip(1)
                .filter(|f| find_ignoring_case(f).is_none())
                .map(|f| {
                    f.file_name()
                        .map_or_else(|| f.display().to_string(), |n| n.to_string_lossy().into())
                })
                .collect();
            note.push_str(&format!(
                " {} names {}, which is not in its folder.",
                cue.display(),
                missing.join(", ")
            ));
        }
        Some(note)
    }
}

/// `path` itself, or the file in its folder whose name matches it ignoring
/// case (how DOSBox finds the files a cue sheet names).
fn find_ignoring_case(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let name = path.file_name()?.to_str()?;
    fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
}

/// True when `path` is a cue sheet: a `.cue`/`.ins`, or a small `.dat`
/// starting with `FILE ` (GOG's `SCREAM.DAT`, `GAME.DAT`).
fn is_cue_sheet(path: &Path, ext: &str) -> bool {
    if ext == "cue" || ext == "ins" {
        return true;
    }
    ext == "dat"
        && fs::metadata(path).is_ok_and(|m| m.len() <= 16_384)
        && fs::read_to_string(path)
            .is_ok_and(|head| head.trim_start().to_ascii_uppercase().starts_with("FILE "))
}

/// The CD image to mount, searched nearest first: the executables' folder,
/// the game folder, every folder directly inside either of them (`CD`,
/// `ISO`, `Screamer_Rally_ISO`, any name), then the folder above the
/// executables. In each folder a cue sheet (`.ins`, `.cue`, GOG's `.dat`)
/// wins, since it carries the CD audio tracks, then `.iso`, `.gog`, and
/// last a raw `.bin`/`.img` (a CloneCD rip without a cue sheet).
fn find_cd_image(root_dir: &Path, exe_dir: &Path) -> CdSearch {
    let mut search_dirs = vec![exe_dir.to_path_buf(), root_dir.to_path_buf()];
    for base in [exe_dir, root_dir] {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        let mut subdirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        search_dirs.extend(subdirs);
    }
    if let Some(parent) = exe_dir.parent() {
        search_dirs.push(parent.to_path_buf());
        search_dirs.push(parent.join("Screamer_Rally_ISO"));
    }
    let mut seen = Vec::new();
    search_dirs.retain(|d| {
        let key = normalize(d);
        let fresh = !seen.contains(&key);
        seen.push(key);
        fresh
    });

    let mut out = CdSearch::default();
    for d in &search_dirs {
        let Ok(entries) = fs::read_dir(d) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for ext_target in ["ins", "cue", "dat", "iso", "gog", "bin", "img"] {
            for p in &files {
                let matches_ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(ext_target));
                if !matches_ext {
                    continue;
                }
                if is_cue_sheet(p, ext_target) {
                    let complete = cd_image_files(Some(p))
                        .iter()
                        .skip(1)
                        .all(|f| find_ignoring_case(f).is_some());
                    if complete {
                        out.image = Some(p.clone());
                        return out;
                    }
                    out.broken.push(p.clone());
                    continue;
                }
                if ext_target == "dat" {
                    continue;
                }
                let raw = ext_target == "bin" || ext_target == "img";
                if raw && !fs::metadata(p).is_ok_and(|m| m.len() >= MIN_RAW_IMAGE_BYTES) {
                    continue;
                }
                out.image = Some(p.clone());
                return out;
            }
        }
    }
    out
}

/// Result of scanning a folder (or parent trilogy folder) for all 3 Screamer games.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredTrilogy {
    /// Discovered games indexed by `Game::index()` (`0` = Screamer, `1` = Screamer 2, `2` = Screamer Rally).
    pub games: [Option<DiscoveredGame>; 3],
    /// If the user's path pointed directly at one specific game folder, which game it was.
    pub direct_game: Option<Game>,
    /// Unknown builds met in folders where no known game was found.
    pub unknown: Vec<UnknownExe>,
}

impl DiscoveredTrilogy {
    pub fn get(&self, game: Game) -> Option<&DiscoveredGame> {
        self.games[game.index()].as_ref()
    }

    pub fn any_found(&self) -> bool {
        self.games.iter().any(|g| g.is_some())
    }

    pub fn count(&self) -> usize {
        self.games.iter().filter(|g| g.is_some()).count()
    }
}

/// Scans `dir`, its `games/` child, and sibling directories to discover all
/// installed games in the Screamer trilogy (`Screamer`, `Screamer 2`, and
/// `Screamer Rally`) from a single path.
pub fn discover_trilogy(dir: &Path) -> DiscoveredTrilogy {
    let mut out = DiscoveredTrilogy::default();
    if !dir.exists() {
        return out;
    }

    match scan_game_dir(dir) {
        Ok(direct) => {
            let idx = direct.game.index();
            out.direct_game = Some(direct.game);
            out.games[idx] = Some(direct);
        }
        Err(_) => out.unknown.extend(unknown_in(dir)),
    }

    let mut containers = vec![dir.to_path_buf(), dir.join("games")];
    if let Some(parent) = dir.parent() {
        containers.push(parent.to_path_buf());
        containers.push(parent.join("games"));
        if let Some(grand) = parent.parent() {
            containers.push(grand.to_path_buf());
            containers.push(grand.join("games"));
        }
    }

    for container in containers {
        if !container.is_dir() {
            continue;
        }
        // Check well-known trilogy subdirectory names first for deterministic ordering
        for sub_name in [
            "screamer",
            "screamer1",
            "Screamer",
            "screamer2",
            "Screamer 2",
            "Screamer2",
            "screamer-rally",
            "screamer_rally",
            "Screamer Rally",
            "ScreamerRally",
        ] {
            let cand = container.join(sub_name);
            if !cand.is_dir() {
                continue;
            }
            match scan_game_dir(&cand) {
                Ok(disc) => {
                    let idx = disc.game.index();
                    if out.games[idx].is_none() {
                        out.games[idx] = Some(disc);
                    }
                }
                Err(_) => {
                    for stranger in unknown_in(&cand) {
                        if !out.unknown.contains(&stranger) {
                            out.unknown.push(stranger);
                        }
                    }
                }
            }
        }
        // Also scan any other immediate child directory
        if let Ok(entries) = fs::read_dir(&container) {
            let mut subdirs: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            subdirs.sort();
            for sub in subdirs {
                if out.games.iter().all(|g| g.is_some()) {
                    break;
                }
                if let Ok(disc) = scan_game_dir(&sub) {
                    let idx = disc.game.index();
                    if out.games[idx].is_none() {
                        out.games[idx] = Some(disc);
                    }
                }
            }
        }
    }

    out
}

/// The unknown builds in `dir` and its usual game subfolders.
fn unknown_in(dir: &Path) -> Vec<UnknownExe> {
    [
        dir.to_path_buf(),
        dir.join("app"),
        dir.join("hdd").join("SR"),
        dir.join("SR"),
    ]
    .iter()
    .filter(|d| d.is_dir())
    .filter_map(|d| scan_exes_in_dir(d).ok())
    .flat_map(|(_, unknown)| unknown)
    .collect()
}

/// Resolves `dir` (which may be a parent trilogy directory or a single game
/// directory) to the [`DiscoveredGame`] for `preferred` (or falls back to the
/// directly pointed-at game if `dir` is a single game folder).
pub fn resolve_game_dir(dir: &Path, preferred: Game) -> Result<DiscoveredGame, String> {
    if !dir.exists() {
        return Err(format!("Game directory does not exist: {}", dir.display()));
    }
    let trilogy = discover_trilogy(dir);
    if let Some(disc) = trilogy.get(preferred) {
        return Ok(disc.clone());
    }
    if let Some(direct) = trilogy.direct_game
        && let Some(disc) = trilogy.get(direct)
    {
        return Ok(disc.clone());
    }
    if let Some(stranger) = trilogy.unknown.first() {
        return Err(stranger.message());
    }
    if let Some(first) = trilogy.games.into_iter().flatten().next() {
        return Ok(first);
    }
    Err(format!(
        "No supported Screamer, Screamer 2, or Screamer Rally executables found in {}",
        dir.display()
    ))
}

impl DiscoveredGame {
    pub fn can_use_dos_glide(&self) -> bool {
        self.game == Game::ScreamerRally
            || (self.game == Game::Screamer2
                && (!self.has_windows_stub_glide2x || self.has_genuine_dos_glide2x))
    }

    /// Chooses the best executable in `self.executables` for `choice`, falling
    /// back to `Auto` if `choice` is not available for `self.game` so shared
    /// trilogy settings work across all three games without error.
    pub fn select_executable(&self, choice: RendererChoice) -> Result<&DiscoveredExe, String> {
        let find_renderer = |r: Renderer| self.executables.iter().find(|e| e.edition.role == r);

        let lookup = match choice {
            RendererChoice::Auto => None,
            RendererChoice::Voodoo => {
                if self.can_use_dos_glide() {
                    find_renderer(Renderer::Glide3dfx)
                } else {
                    None
                }
            }
            RendererChoice::Svga16Bit => find_renderer(Renderer::Svga16Bit),
            RendererChoice::Svga8Bit => find_renderer(Renderer::Svga8Bit),
            RendererChoice::Vga => find_renderer(Renderer::VgaLow),
        };
        if let Some(exe) = lookup {
            return Ok(exe);
        }

        // Auto / graceful fallback for options not present in this game:
        if let Some(voodoo) = find_renderer(Renderer::Glide3dfx)
            && self.can_use_dos_glide()
        {
            return Ok(voodoo);
        }
        if let Some(svga16) = find_renderer(Renderer::Svga16Bit) {
            return Ok(svga16);
        }
        if let Some(svga8) = find_renderer(Renderer::Svga8Bit) {
            return Ok(svga8);
        }
        if let Some(vga) = find_renderer(Renderer::VgaLow) {
            return Ok(vga);
        }
        Err("No playable racing executable found in game folder".into())
    }
}

/// Validates every discovered executable's `FixSet` against the on-disk binary
/// (verifying both `expected` bytes and zero `LE` fixup table collisions).
pub fn verify_discovered_game(
    discovered: &DiscoveredGame,
    skip: &[&str],
) -> Result<Vec<(String, &'static Edition, usize)>, String> {
    let mut results = Vec::new();
    for exe in &discovered.executables {
        let bytes = fs::read(&exe.path).map_err(|e| format!("{}: {e}", exe.path.display()))?;
        let img = Image::parse(&bytes).map_err(|e| format!("{}: {e}", exe.file_name))?;
        let writes = crate::prepare(&img, exe.edition.fixes, skip, 0)
            .map_err(|e| format!("{}: {e}", exe.file_name))?;
        results.push((exe.file_name.clone(), exe.edition, writes.len()));
    }
    Ok(results)
}

/// The path DOSBox is given for `path`: as it is when plain ASCII, else
/// `short`, Windows' short (8.3) name for it. DOSBox Staging opens host
/// paths through narrow (ANSI) calls on Windows, so a folder such as
/// `Jogos Ação` could not be mounted (its MinGW build even ended at
/// `IMGMOUNT` under Wine).
pub fn path_for_dosbox(path: &Path, short: Option<String>) -> PathBuf {
    let long = path.to_string_lossy();
    paths::fit_path(&long, short.as_deref(), usize::MAX)
        .map_or_else(|| path.to_path_buf(), PathBuf::from)
}

/// What to tell a Windows player whose game or runtime folder DOSBox
/// cannot open: `path` (as handed to DOSBox) still has other than ASCII.
pub fn dosbox_path_warning(path: &Path) -> Option<String> {
    let shown = path.to_string_lossy();
    (!shown.is_ascii()).then(|| {
        format!(
            "DOSBox may not open {shown}: its name has letters outside plain ASCII and this drive keeps no short (8.3) names. \
             If the game asks for its CD or does not start, move it to a folder whose path has only plain letters."
        )
    })
}

/// [`path_for_dosbox`] with the short name Windows keeps for `path`.
fn host_path_for_dosbox(path: &Path, warnings: &mut Vec<String>) -> PathBuf {
    #[cfg(windows)]
    {
        let chosen = path_for_dosbox(path, rea_launcher_win32::short_path::short_path(path));
        warnings.extend(dosbox_path_warning(&chosen));
        chosen
    }
    #[cfg(not(windows))]
    {
        // DOSBox's POSIX build opens UTF-8 paths as they are.
        let _ = warnings;
        path.to_path_buf()
    }
}

/// Prepares `<runtime_dir>` by staging game files from `discovered.exe_dir`
/// ([`stage_source_tree`]), writing patched executables into `<runtime_dir>`,
/// generating `SCREAMER.CFG`, `CONFIG/SETUP.CFG`, `CONFIG/CONTROLS.CFG`, and
/// `dosbox-screamer.conf`, and syncing saves from `saves_dir`.
pub fn prepare_runtime(
    discovered: &DiscoveredGame,
    settings: &Settings,
    runtime_dir: &Path,
    saves_dir: Option<&Path>,
    gpu: Gpu,
) -> Result<PreparedRuntime, String> {
    fs::create_dir_all(runtime_dir).map_err(|e| format!("{}: {e}", runtime_dir.display()))?;
    let config_dir = runtime_dir.join("CONFIG");
    let replays_dir = runtime_dir.join("REPLAYS");
    fs::create_dir_all(&config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
    fs::create_dir_all(&replays_dir).map_err(|e| format!("{}: {e}", replays_dir.display()))?;

    // Stage top-level entries from exe_dir into runtime_dir without modifying exe_dir.
    let staged = stage_source_tree(
        &discovered.exe_dir,
        runtime_dir,
        discovered.cd_image.as_deref(),
    )?;
    if let Some(ovl_src) = &discovered.glide2x_ovl_source
        && !runtime_dir.join("GLIDE2X.OVL").is_file()
    {
        bring_fallback_ovl(ovl_src, runtime_dir)?;
    }

    // Patch and write all recognized executables into runtime_dir.
    let skip = settings.skipped_fixes();
    let mut patched_exes = Vec::new();
    for exe in &discovered.executables {
        let mut bytes = fs::read(&exe.path).map_err(|e| format!("{}: {e}", exe.path.display()))?;
        let writes = {
            let img = Image::parse(&bytes).map_err(|e| format!("{}: {e}", exe.file_name))?;
            crate::prepare(&img, exe.edition.fixes, &skip, 0)
                .map_err(|e| format!("{}: {e}", exe.file_name))?
        };
        let count = crate::apply_to_image(&mut bytes, &writes)
            .map_err(|e| format!("{}: {e}", exe.file_name))?;
        let dst = runtime_dir.join(&exe.file_name);
        fsx::install(&dst, &bytes).map_err(|e| e.to_string())?;
        patched_exes.push((exe.file_name.clone(), count));
    }

    let selected = discovered.select_executable(settings.renderer_choice())?;
    let mut warnings: Vec<String> = discovered.cd_warning.iter().cloned().collect();
    let dosbox_runtime = host_path_for_dosbox(runtime_dir, &mut warnings);
    let dosbox_cd = discovered
        .cd_image
        .as_deref()
        .map(|cd| host_path_for_dosbox(cd, &mut warnings));
    warnings.extend(staged.warnings);
    let plan = plan(
        settings,
        discovered.game,
        &selected.file_name,
        selected.edition.role,
        &dosbox_runtime,
        dosbox_cd.as_deref(),
        gpu,
    );

    // SCREAMER.CFG, CONFIG/SETUP.CFG, CONFIG/CONTROLS.CFG and the DOSBox
    // configuration: generated afresh each launch (the game reads them; it
    // writes only CHOICE.DAT, saves and replays).
    for (rel, bytes) in plan.files() {
        fsx::install(&native(runtime_dir, rel), bytes).map_err(|e| e.to_string())?;
    }

    // Sync user saves in if a saves directory is configured.
    if let Some(saves) = saves_dir {
        sync_saves_in(saves, runtime_dir)?;
    }

    Ok(PreparedRuntime {
        runtime_dir: runtime_dir.to_path_buf(),
        conf_path: runtime_dir.join(CONF_NAME),
        selected_exe: selected.file_name.clone(),
        selected_edition: selected.edition,
        patched_exes,
        warnings,
        plan,
    })
}

/// The DOSBox configuration file the launcher writes into the runtime.
pub const CONF_NAME: &str = "dosbox-screamer.conf";

/// The GPU DOSBox should run on (hybrid laptops).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gpu {
    Nvidia,
    Amd,
    Default,
}

impl Gpu {
    pub fn of(machine: &crate::config::MachineCapabilities) -> Gpu {
        if machine.has_nvidia_dgpu {
            Gpu::Nvidia
        } else if machine.has_amd_dgpu {
            Gpu::Amd
        } else {
            Gpu::Default
        }
    }
}

/// Everything a launch writes and passes on, from the settings alone:
/// the files generated into the runtime folder, DOSBox's arguments and
/// environment, and the optional fixes left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub game: Game,
    pub exe: String,
    pub skipped_fixes: Vec<&'static str>,
    pub screamer_cfg: Vec<u8>,
    pub setup_cfg: Vec<u8>,
    pub controls_cfg: Vec<u8>,
    pub dosbox_conf: String,
    pub dosbox_args: Vec<String>,
    pub env: Vec<(&'static str, String)>,
}

impl LaunchPlan {
    /// The generated files, by their path in the runtime folder.
    pub fn files(&self) -> [(&'static str, &[u8]); 4] {
        [
            ("SCREAMER.CFG", &self.screamer_cfg),
            ("CONFIG/SETUP.CFG", &self.setup_cfg),
            ("CONFIG/CONTROLS.CFG", &self.controls_cfg),
            (CONF_NAME, self.dosbox_conf.as_bytes()),
        ]
    }
}

/// The plan for running `exe` (of `role`) of `game` from `runtime_dir` with
/// `settings`. Pure: nothing is read or written.
pub fn plan(
    settings: &Settings,
    game: Game,
    exe: &str,
    role: Renderer,
    runtime_dir: &Path,
    cd_image: Option<&Path>,
    gpu: Gpu,
) -> LaunchPlan {
    let race_res = settings.race_resolution();
    let fullscreen = race_res == Some(crate::config::FULLSCREEN);
    let res = match race_res {
        None | Some(crate::config::FULLSCREEN) => (1280, 960),
        Some(wh) => wh,
    };
    let filter = settings.effective_filter();
    let voodoo_enabled = role == Renderer::Glide3dfx;
    let dosbox_conf = build_dosbox_conf(&DosboxConfOptions {
        fullscreen,
        resolution: res,
        widescreen: settings.widescreen_enabled(),
        vsync: settings.vsync_enabled(),
        voodoo_enabled,
        texture_filtering: matches!(filter, TextureFilter::Smooth | TextureFilter::Fsr),
        glshader: match filter {
            TextureFilter::Original => "crt-auto",
            TextureFilter::Fsr => "interpolation/fsr",
            TextureFilter::Sharp => "sharp",
            TextureFilter::Smooth => "bilinear",
        },
        c_drive_dir: runtime_dir,
        cd_image_path: cd_image,
        exe_name: exe,
        exit_after_run: true,
    });
    let on = |b: bool| if b { "1" } else { "0" }.to_string();
    let mut env: Vec<(&'static str, String)> = match gpu {
        Gpu::Nvidia => vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".into()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".into()),
        ],
        Gpu::Amd => vec![("DRI_PRIME", "1".into())],
        Gpu::Default => Vec::new(),
    };
    // Read by the screamer-hooks and the Voodoo renderer linked into DOSBox.
    env.extend([
        ("SCREAMER_GAME", game.slug().to_string()),
        ("SCREAMER_EXE", exe.to_string()),
        (
            "SCREAMER_VOODOO_SCALE",
            if voodoo_enabled && race_res.is_some() {
                "2"
            } else {
                "1"
            }
            .to_string(),
        ),
        (
            "SCREAMER_FRAME_GEN",
            on(settings.frame_generation_enabled()),
        ),
        (
            "SCREAMER_GAMEPAD_AXES",
            on(settings.gamepad_axes != Some(false)),
        ),
        (
            "SCREAMER_DEADZONE",
            settings.deadzone.unwrap_or(0).to_string(),
        ),
        ("SCREAMER_RUMBLE", settings.rumble_strength().to_string()),
        (
            "SCREAMER_BUTTONS",
            crate::config::buttons_value(&settings.button_map()),
        ),
    ]);
    LaunchPlan {
        game,
        exe: exe.to_string(),
        skipped_fixes: settings.skipped_fixes(),
        screamer_cfg: build_screamer_cfg(game),
        setup_cfg: build_setup_cfg(game, 7),
        controls_cfg: build_controls_cfg(game, KeyBindings::default_for(game)),
        dosbox_conf,
        dosbox_args: vec![
            "-noprimaryconf".into(),
            "-nolocalconf".into(),
            "-conf".into(),
            native(runtime_dir, CONF_NAME).display().to_string(),
        ],
        env,
    }
}

/// Name of the staging manifest inside the runtime folder.
pub const MANIFEST: &str = ".screamer-manifest";

/// True for files the game only reads (code and drivers), which may be
/// hard-linked instead of copied. Everything else is copied, so nothing the
/// game opens for writing inside DOSBox can reach the user's folder.
fn is_static(rel: &str) -> bool {
    let upper = rel.to_ascii_uppercase();
    [".EXE", ".COM", ".386", ".OVL", ".DLL", ".ICO", ".PDF"]
        .iter()
        .any(|ext| upper.ends_with(ext))
}

/// True for files the game writes while playing (saves and replays): copied
/// once, then owned by the game; a later change in the source never
/// replaces them.
fn is_game_written(rel: &str) -> bool {
    let upper = rel.to_ascii_uppercase();
    let name = upper.rsplit('/').next().unwrap_or_default();
    is_save_file(name) || upper.starts_with("REPLAYS/")
}

/// True for source files the launcher generates or patches itself, or that
/// must not shadow DOSBox's own drivers.
fn is_generated(rel: &str, stamp: Stamp) -> bool {
    let upper = rel.to_ascii_uppercase();
    upper == "SCREAMER.CFG"
        || KNOWN_EXE_NAMES.contains(&upper.as_str())
        || upper == "CONFIG/SETUP.CFG"
        || upper == "CONFIG/CONTROLS.CFG"
        // Screamer 2's 31 KB Windows stub would shadow DOSBox's built-in
        // Glide driver; only a genuine DOS GLIDE2X.OVL (>100 KB) is staged.
        || (upper == "GLIDE2X.OVL" && stamp.size < 50_000)
}

/// Extensions of CD images and their cue sheets and side files.
const CD_IMAGE_EXTENSIONS: &[&str] = &[
    "iso", "gog", "bin", "img", "cue", "ins", "ccd", "sub", "mds", "mdf",
];

fn has_cd_image_extension(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| {
        CD_IMAGE_EXTENSIONS
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    })
}

/// The CD image and the files its cue sheet names (DOS-style `\` and
/// `..\` accepted). They are mounted read-only straight from the user's
/// folder, so they are never staged.
fn cd_image_files(cd_image: Option<&Path>) -> Vec<PathBuf> {
    let Some(image) = cd_image else {
        return Vec::new();
    };
    let mut out = vec![normalize(image)];
    let small = fs::metadata(image).is_ok_and(|m| m.len() <= 16_384);
    if small && let Ok(text) = fs::read_to_string(image) {
        let dir = image.parent().unwrap_or(Path::new(""));
        for line in text.lines() {
            let line = line.trim();
            if !line.to_ascii_uppercase().starts_with("FILE ") {
                continue;
            }
            let rest = line[5..].trim();
            let name = if let Some(quoted) = rest.strip_prefix('"') {
                quoted.split('"').next().unwrap_or_default()
            } else {
                rest.split_whitespace().next().unwrap_or_default()
            };
            if !name.is_empty() {
                let relative: PathBuf = name.split(['\\', '/']).collect();
                out.push(normalize(&dir.join(relative)));
            }
        }
    }
    out
}

fn same_path_ignoring_case(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

/// Screamer's rules for the runtime folder: DOS names in upper case, code
/// and drivers hard-linked, saves and replays left to the game, and the
/// files the launcher generates or mounts never brought over. When a CD
/// image is mounted from the source, every file with a CD-image extension
/// and every file its cue sheet names stays there.
struct Staging<'a> {
    source: &'a Path,
    cd_mounted: bool,
    mounted: Vec<PathBuf>,
}

impl Policy for Staging<'_> {
    fn link(&self, rel: &str) -> bool {
        is_static(rel)
    }

    fn game_written(&self, rel: &str, _source: Stamp) -> bool {
        is_game_written(rel)
    }

    fn owned(&self, rel: &str, source: Stamp) -> bool {
        is_generated(rel, source) || (self.cd_mounted && has_cd_image_extension(rel)) || {
            let path = normalize(&native(self.source, rel));
            self.mounted
                .iter()
                .any(|m| same_path_ignoring_case(m, &path))
        }
    }

    fn runtime_rel(&self, rel: &str) -> String {
        rel.to_ascii_uppercase()
    }
}

/// Brings `dst_dir` up to date with `src_dir` without ever writing into
/// `src_dir`: real directories in the runtime folder, one file at a time
/// (hard-linked or copied), skipping files the launcher generates and CD
/// images (see [`Staging`]). Symbolic links left by older launchers are
/// removed first.
pub fn stage_source_tree(
    src_dir: &Path,
    dst_dir: &Path,
    cd_image: Option<&Path>,
) -> Result<SyncReport, String> {
    fs::create_dir_all(dst_dir).map_err(|e| format!("{}: {e}", dst_dir.display()))?;
    runtime::remove_legacy_links(dst_dir).map_err(|e| e.to_string())?;
    let policy = Staging {
        source: src_dir,
        cd_mounted: cd_image.is_some(),
        mounted: cd_image_files(cd_image),
    };
    runtime::sync(src_dir, dst_dir, &policy, MANIFEST).map_err(|e| e.to_string())
}

/// The genuine DOS `GLIDE2X.OVL` found outside the game folder (a sibling
/// Screamer Rally, `SCREAMER_GLIDE2X_OVL`): staged as `GLIDE2X.OVL`
/// whatever its own name, hard-linked when possible.
struct FallbackOvl;

impl Policy for FallbackOvl {
    fn link(&self, _rel: &str) -> bool {
        true
    }

    fn runtime_rel(&self, _rel: &str) -> String {
        "GLIDE2X.OVL".into()
    }
}

fn bring_fallback_ovl(source: &Path, runtime_dir: &Path) -> Result<(), String> {
    let dir = source.parent().unwrap_or(Path::new(""));
    let name = source.file_name().unwrap_or_default().to_string_lossy();
    runtime::restore(dir, runtime_dir, &name, &FallbackOvl).map_err(|e| e.to_string())
}

fn is_save_file(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.ends_with(".SAV") || upper == "CHOICE.DAT"
}

/// Name of the save-sync state inside the runtime folder.
pub const SAVE_STATE: &str = ".screamer-saves";

/// The lock file inside a runtime folder while the game plays from it.
pub const RUN_LOCK: &str = ".screamer-running";

/// Held while the game plays from a runtime folder, so a second launcher
/// never rewrites the runtime under a running game (STANDARDS "Clean exit,
/// single instance": Play is refused while the game from this runtime
/// runs). The system releases it when the launcher ends, however it ends.
#[derive(Debug)]
pub struct RunLock {
    _file: fs::File,
}

/// Why [`lock_runtime`] gave no lock.
#[derive(Debug)]
pub enum RunLockError {
    /// Another launcher plays from this runtime folder.
    Busy(PathBuf),
    /// The lock file can't be made or locked (a file system without
    /// locks): the launcher plays without the check.
    Unavailable(std::io::Error),
}

impl std::fmt::Display for RunLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunLockError::Busy(runtime) => write!(
                f,
                "The game is already running from {}. Quit it first, then press Play again.",
                runtime.display()
            ),
            RunLockError::Unavailable(e) => write!(f, "cannot lock the runtime folder: {e}"),
        }
    }
}

/// Takes the runtime folder's [`RUN_LOCK`] (creating the folder).
pub fn lock_runtime(runtime: &Path) -> Result<RunLock, RunLockError> {
    fs::create_dir_all(runtime).map_err(RunLockError::Unavailable)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(runtime.join(RUN_LOCK))
        .map_err(RunLockError::Unavailable)?;
    match file.try_lock() {
        Ok(()) => Ok(RunLock { _file: file }),
        Err(fs::TryLockError::WouldBlock) => Err(RunLockError::Busy(runtime.to_path_buf())),
        Err(fs::TryLockError::Error(e)) => Err(RunLockError::Unavailable(e)),
    }
}

/// The folders holding persistent saves (only the files directly in them).
const SAVE_FOLDERS: &[&str] = &["", "CONFIG", "REPLAYS"];

/// The persistent saves: `*.SAV` and `CHOICE.DAT` in the game's folder and
/// `CONFIG/`, and `REPLAYS/*.RPL` (any case).
fn is_save(rel: &str) -> bool {
    let upper = rel.to_ascii_uppercase();
    match upper.split_once('/') {
        None => is_save_file(&upper),
        Some(("CONFIG", name)) => is_save_file(name),
        Some(("REPLAYS", name)) => name.ends_with(".RPL"),
        Some(_) => false,
    }
}

/// The saves mirror between the runtime folder and the saves folder, by the
/// state recorded in [`SAVE_STATE`]. A save with no recorded state yet: the
/// newer copy wins.
fn saves_mirror<'a>(runtime_dir: &'a Path, saves_dir: &'a Path, state: &'a Path) -> Mirror<'a> {
    Mirror::folder(runtime_dir, saves_dir, state, FirstRun::NewerWins)
        .scan(SAVE_FOLDERS)
        .only(&is_save)
}

/// Brings persistent saves (`*.SAV`, `CHOICE.DAT`, `REPLAYS/*.RPL`) into
/// `runtime_dir` at launch. Against the state recorded at the last sync:
/// a save changed in `saves_dir` (the player restored or copied one in, even
/// an older file) wins; otherwise a save changed in the runtime folder (the
/// last session ended before its saves were copied back) is copied out.
pub fn sync_saves_in(saves_dir: &Path, runtime_dir: &Path) -> Result<(), String> {
    if !saves_dir.exists() {
        return Ok(());
    }
    let state = runtime_dir.join(SAVE_STATE);
    saves_mirror(runtime_dir, saves_dir, &state)
        .before()
        .map(|_| ())
        .map_err(|e| format!("copy {e}"))
}

/// Copies persistent saves from `runtime_dir` back to `saves_dir` when their
/// contents differ, and records the synced state for the next launch.
pub fn sync_saves_out(runtime_dir: &Path, saves_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(saves_dir).map_err(|e| format!("{}: {e}", saves_dir.display()))?;
    let state = runtime_dir.join(SAVE_STATE);
    saves_mirror(runtime_dir, saves_dir, &state)
        .after()
        .map(|_| ())
        .map_err(|e| format!("copy {e}"))
}

/// Refuses a runtime or saves folder that overlaps the game folder, since
/// the launcher writes there and the game folder must stay untouched.
/// Nothing is created while checking.
pub fn check_folders(game: &Path, runtime_dir: &Path, saves_dir: &Path) -> Result<(), String> {
    paths::check_separate(game, runtime_dir).map_err(|e| format!("Runtime folder: {e}"))?;
    paths::check_separate(game, saves_dir).map_err(|e| format!("Saves folder: {e}"))
}

/// Locates the `dosbox-staging` (or `dosbox`) executable to run.
pub fn find_dosbox(explicit: Option<&Path>) -> Option<PathBuf> {
    find_dosbox_with_hint(explicit, None)
}

/// Locates the `dosbox` (or `dosbox-staging`) executable, checking `explicit`,
/// `SCREAMER_DOSBOX`, the launcher's own directory (for the bundled `dosbox`
/// built with `libscreamer_hooks.a`), relative `dosbox-staging/` folders next
/// to `game_dir`, and `PATH`.
pub fn find_dosbox_with_hint(explicit: Option<&Path>, game_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit
        && p.is_file()
    {
        return Some(p.to_path_buf());
    }
    if let Ok(env_p) = std::env::var("SCREAMER_DOSBOX") {
        let p = PathBuf::from(env_p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for cand in [
            "dosbox",
            "dosbox.exe",
            "dosbox-staging",
            "dosbox-staging/dosbox",
            "dosbox-staging/dosbox.exe",
        ] {
            let p = dir.join(cand);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if let Some(mut cur) = game_dir {
        for _ in 0..4 {
            for rel in [
                "dosbox-staging/dosbox",
                "dosbox-staging/dosbox.exe",
                "dosbox/dosbox",
                "dosbox/dosbox.exe",
                "DOSBOX/dosbox.exe",
            ] {
                let cand = cur.join(rel);
                if cand.is_file() {
                    return Some(cand);
                }
            }
            if let Some(parent) = cur.parent() {
                cur = parent;
            } else {
                break;
            }
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for cand in ["dosbox-staging", "dosbox", "dosbox.exe"] {
                let p = dir.join(cand);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher::runtime::Listing;
    use std::collections::BTreeMap;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn stamp(size: u64, mtime: u64) -> Stamp {
        Stamp { size, mtime }
    }

    fn listing(items: &[(&str, Stamp)]) -> Listing {
        items.iter().map(|(r, s)| (r.to_string(), *s)).collect()
    }

    #[test]
    fn a_failed_check_links_one_issue_form() {
        let plain = with_bug_link("Screamer 2 (1996) was not found under /g".into());
        assert!(plain.ends_with(BUG_URL), "{plain}");
        let unknown = UnknownExe {
            path: PathBuf::from("/g/STARTH.EXE"),
            file_name: "STARTH.EXE".into(),
            sha256: [0xab; 32],
        };
        let message = with_bug_link(unknown.message());
        assert!(message.ends_with(ISSUE_URL), "{message}");
        assert!(!message.contains(BUG_URL));
        // Both forms exist in the repository.
        let forms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/ISSUE_TEMPLATE");
        for url in [ISSUE_URL, BUG_URL] {
            let file = url.rsplit("template=").next().unwrap();
            assert!(forms.join(file).is_file(), "{file}");
        }
    }

    #[test]
    fn a_second_play_from_the_same_runtime_is_refused() {
        let rt = temp("run lock José d'Ação");
        let first = lock_runtime(&rt).expect("first lock");
        let second = lock_runtime(&rt).unwrap_err();
        assert!(matches!(second, RunLockError::Busy(_)), "{second:?}");
        let message = second.to_string();
        assert!(message.contains("already running"), "{message}");
        assert!(message.contains("d'Ação"), "{message}");
        // Another runtime (another game) is free.
        let other = lock_runtime(&temp("run lock other")).expect("other runtime");
        drop(other);
        // The lock goes with the launcher that held it.
        drop(first);
        let again = lock_runtime(&rt).expect("free after the first launcher ended");
        drop(again);
        // The lock file is runtime-only: never a save, never synced.
        assert!(!is_save(RUN_LOCK));
        let _ = fs::remove_dir_all(&rt);
    }

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "screamer-launcher-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, Stamp)> {
        runtime::list(root)
            .unwrap()
            .into_iter()
            .map(|(rel, s)| (rel.clone(), (fs::read(native(root, &rel)).unwrap(), s)))
            .collect()
    }

    fn touch_later(path: &Path) {
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() + Duration::from_secs(60))
            .unwrap();
    }

    /// A small GOG-style Screamer folder.
    fn make_game(game: &Path) {
        fs::create_dir_all(game.join("DATA")).unwrap();
        fs::create_dir_all(game.join("CONFIG")).unwrap();
        fs::create_dir_all(game.join("REPLAYS")).unwrap();
        fs::write(game.join("DOS4GW.EXE"), b"extender").unwrap();
        fs::write(game.join("STARTH.EXE"), b"original exe").unwrap();
        fs::write(game.join("SCREAMER.CFG"), b"user cfg").unwrap();
        fs::write(game.join("DATA/track.dat"), b"track").unwrap();
        fs::write(game.join("CONFIG/CHOICE.DAT"), b"choices").unwrap();
        fs::write(game.join("CONFIG/SETUP.CFG"), b"setup").unwrap();
        fs::write(game.join("CONFIG/DEFCTRL.CFG"), b"defctrl").unwrap();
        fs::write(game.join("REPLAYS/A.RPL"), b"replay").unwrap();
        fs::write(
            game.join("SCREAM.DAT"),
            b"FILE \"SCREAM.gog\" BINARY\r\n  TRACK 01 MODE1/2352\r\n",
        )
        .unwrap();
        fs::write(game.join("SCREAM.gog"), b"big cd image").unwrap();
    }

    #[test]
    fn changed_data_is_brought_again_but_saves_are_kept() {
        let old = listing(&[
            ("DATA/a.dat", stamp(1, 1)),
            ("CONFIG/CHOICE.DAT", stamp(1, 1)),
            ("REPLAYS/A.RPL", stamp(1, 1)),
        ]);
        let new = listing(&[
            ("DATA/a.dat", stamp(1, 9)),
            ("CONFIG/CHOICE.DAT", stamp(5, 9)),
            ("REPLAYS/A.RPL", stamp(5, 9)),
        ]);
        let policy = Staging {
            source: Path::new("/game"),
            cd_mounted: false,
            mounted: Vec::new(),
        };
        let p = runtime::plan(&new, &|_| true, &old, &policy);
        assert_eq!(p.copy, ["DATA/a.dat"]);
        assert_eq!(p.manifest["CONFIG/CHOICE.DAT"], stamp(1, 1));
        assert!(
            runtime::plan(&new, &|_| true, &new, &policy)
                .copy
                .is_empty()
        );
    }

    #[test]
    fn an_unknown_build_is_refused_with_its_hash_and_the_issue_link() {
        let game = temp("unknown-build");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("starth.exe"), b"a build nobody listed").unwrap();
        let hash = rea_launcher::identify::hex(&edition::sha256(b"a build nobody listed"));
        let err = scan_game_dir(&game).unwrap_err();
        assert!(err.contains(&hash) && err.contains(ISSUE_URL), "{err}");
        assert!(err.contains("STARTH.EXE"), "{err}");
        let trilogy = discover_trilogy(&game);
        assert!(!trilogy.any_found());
        assert_eq!(trilogy.unknown.len(), 1);
        assert_eq!(
            trilogy.unknown[0].sha256,
            edition::sha256(b"a build nobody listed")
        );
        let err = resolve_game_dir(&game, Game::Screamer).unwrap_err();
        assert!(err.contains(&hash), "{err}");
        // Other names are not game executables.
        fs::remove_file(game.join("starth.exe")).unwrap();
        fs::write(game.join("README.EXE"), b"x").unwrap();
        assert!(discover_trilogy(&game).unknown.is_empty());
        fs::remove_dir_all(game).unwrap();
    }

    #[test]
    fn files_are_classified() {
        assert!(is_static("DOS4GW.EXE"));
        assert!(is_static("CONFIG/hmidrv.386"));
        assert!(!is_static("DATA/TRACK.DAT"));
        assert!(!is_static("CONFIG/CHOICE.DAT"));
        assert!(is_game_written("CONFIG/choice.dat"));
        assert!(is_game_written("GAME1.SAV"));
        assert!(is_game_written("replays/x.rpl"));
        assert!(!is_game_written("DATA/X.DAT"));
        assert!(is_generated("screamer.cfg", stamp(1, 1)));
        assert!(is_generated("STARTH.EXE", stamp(1, 1)));
        assert!(is_generated("config/setup.cfg", stamp(1, 1)));
        assert!(is_generated("GLIDE2X.OVL", stamp(31_000, 1)));
        assert!(!is_generated("GLIDE2X.OVL", stamp(200_000, 1)));
        assert!(!is_generated("CONFIG/DEFCTRL.CFG", stamp(1, 1)));
    }

    #[test]
    fn staging_never_modifies_the_user_folder() {
        let game = temp("stage-game");
        let rt = temp("stage-rt");
        make_game(&game);
        let before = snapshot(&game);

        let cd = game.join("SCREAM.DAT");
        let SyncReport {
            brought, linked, ..
        } = stage_source_tree(&game, &rt, Some(&cd)).unwrap();
        assert_eq!(brought, 5);
        assert!(linked <= 1, "only the extender may be hard-linked");
        // Real directories, no links, and nothing generated or mounted.
        assert!(
            !fs::symlink_metadata(rt.join("DATA"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(rt.join("DATA/TRACK.DAT")).unwrap(), b"track");
        assert_eq!(fs::read(rt.join("CONFIG/DEFCTRL.CFG")).unwrap(), b"defctrl");
        assert_eq!(fs::read(rt.join("REPLAYS/A.RPL")).unwrap(), b"replay");
        for absent in [
            "SCREAMER.CFG",
            "STARTH.EXE",
            "CONFIG/SETUP.CFG",
            "SCREAM.DAT",
            "SCREAM.GOG",
        ] {
            assert!(!native(&rt, absent).exists(), "{absent} must not be staged");
        }
        assert_eq!(
            stage_source_tree(&game, &rt, Some(&cd)).unwrap(),
            SyncReport::default()
        );

        // The game writes data files in place; the launcher swaps whole files.
        fs::write(rt.join("DATA/TRACK.DAT"), b"game wrote").unwrap();
        fs::write(rt.join("CONFIG/CHOICE.DAT"), b"played").unwrap();
        fsx::install(&rt.join("DOS4GW.EXE"), b"replaced").unwrap();
        fsx::install(&rt.join("SCREAMER.CFG"), b"generated").unwrap();
        assert_eq!(
            snapshot(&game),
            before,
            "the user's folder is never written"
        );

        for d in [game, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn changed_source_files_are_refreshed_and_game_files_kept() {
        let game = temp("refresh-game");
        let rt = temp("refresh-rt");
        make_game(&game);
        stage_source_tree(&game, &rt, None).unwrap();

        fs::write(rt.join("CONFIG/CHOICE.DAT"), b"played").unwrap();
        fs::write(rt.join("REPLAYS/NEW.RPL"), b"new replay").unwrap();
        fs::write(game.join("DATA/track.dat"), b"patched track").unwrap();
        touch_later(&game.join("DATA/track.dat"));
        fs::write(game.join("CONFIG/CHOICE.DAT"), b"other choices").unwrap();
        touch_later(&game.join("CONFIG/CHOICE.DAT"));

        assert_eq!(stage_source_tree(&game, &rt, None).unwrap().brought, 1);
        assert_eq!(
            fs::read(rt.join("DATA/TRACK.DAT")).unwrap(),
            b"patched track"
        );
        assert_eq!(fs::read(rt.join("CONFIG/CHOICE.DAT")).unwrap(), b"played");
        assert_eq!(fs::read(rt.join("REPLAYS/NEW.RPL")).unwrap(), b"new replay");

        // A game-written file deleted from the runtime folder comes back.
        fs::remove_file(rt.join("CONFIG/CHOICE.DAT")).unwrap();
        stage_source_tree(&game, &rt, None).unwrap();
        assert_eq!(
            fs::read(rt.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"other choices"
        );

        for d in [game, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn links_from_older_launchers_are_replaced_by_copies() {
        let game = temp("legacy-game");
        let rt = temp("legacy-rt");
        make_game(&game);
        fs::create_dir_all(&rt).unwrap();
        std::os::unix::fs::symlink(game.join("DATA"), rt.join("DATA")).unwrap();
        std::os::unix::fs::symlink(game.join("DOS4GW.EXE"), rt.join("DOS4GW.EXE")).unwrap();
        let before = snapshot(&game);

        stage_source_tree(&game, &rt, None).unwrap();
        for rel in ["DATA", "DOS4GW.EXE"] {
            assert!(
                !fs::symlink_metadata(rt.join(rel))
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }
        fs::write(rt.join("DATA/TRACK.DAT"), b"game wrote").unwrap();
        assert_eq!(snapshot(&game), before);

        for d in [game, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn overlapping_folders_are_refused() {
        let game = temp("overlap");
        fs::create_dir_all(&game).unwrap();
        let outside = temp("elsewhere");
        let err = check_folders(&game, &game.join("runtime"), &outside).unwrap_err();
        assert!(err.starts_with("Runtime folder: "), "{err}");
        let dotdot = game
            .join("..")
            .join(game.file_name().unwrap())
            .join("saves");
        let err = check_folders(&game, &outside, &dotdot).unwrap_err();
        assert!(err.starts_with("Saves folder: "), "{err}");
        assert!(!game.join("runtime").exists() && !game.join("saves").exists());
        assert!(check_folders(&game, &outside, &temp("elsewhere-saves")).is_ok());
        fs::remove_dir_all(game).unwrap();
    }

    fn set_mtime(path: &Path, secs: u64) {
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }

    #[test]
    fn save_sync_never_puts_an_older_save_over_a_newer_one() {
        let saves = temp("sync-saves");
        let rt = temp("sync-rt");
        for d in [&saves, &rt] {
            fs::create_dir_all(d.join("CONFIG")).unwrap();
            fs::create_dir_all(d.join("REPLAYS")).unwrap();
        }
        // The saves folder has a newer CHOICE.DAT, the runtime a newer replay.
        fs::write(saves.join("CONFIG/CHOICE.DAT"), b"saved later").unwrap();
        set_mtime(&saves.join("CONFIG/CHOICE.DAT"), 2_000);
        fs::write(rt.join("CONFIG/CHOICE.DAT"), b"from the game folder").unwrap();
        set_mtime(&rt.join("CONFIG/CHOICE.DAT"), 1_000);
        fs::write(saves.join("REPLAYS/A.RPL"), b"old replay").unwrap();
        set_mtime(&saves.join("REPLAYS/A.RPL"), 1_000);
        fs::write(rt.join("REPLAYS/A.RPL"), b"new replay").unwrap();
        set_mtime(&rt.join("REPLAYS/A.RPL"), 2_000);
        fs::write(saves.join("GAME.SAV"), b"save").unwrap();

        sync_saves_in(&saves, &rt).unwrap();
        assert_eq!(
            fs::read(rt.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"saved later"
        );
        assert_eq!(fs::read(rt.join("REPLAYS/A.RPL")).unwrap(), b"new replay");
        assert_eq!(fs::read(rt.join("GAME.SAV")).unwrap(), b"save");

        sync_saves_out(&rt, &saves).unwrap();
        assert_eq!(
            fs::read(saves.join("REPLAYS/A.RPL")).unwrap(),
            b"new replay"
        );
        assert_eq!(
            fs::read(saves.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"saved later"
        );

        for d in [saves, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_source_files_are_staged_as_their_targets() {
        let game = temp("symlink-game");
        let rt = temp("symlink-rt");
        let outside = temp("symlink-outside");
        make_game(&game);
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("REAL.OVL"), vec![7u8; 200_000]).unwrap();
        // A relative link, an absolute link, and a linked code file.
        std::os::unix::fs::symlink("track.dat", game.join("DATA/rel.dat")).unwrap();
        std::os::unix::fs::symlink(game.join("DATA/track.dat"), game.join("DATA/abs.dat")).unwrap();
        std::os::unix::fs::symlink(outside.join("REAL.OVL"), game.join("GLIDE2X.OVL")).unwrap();

        stage_source_tree(&game, &rt, None).unwrap();
        for rel in ["DATA/REL.DAT", "DATA/ABS.DAT", "GLIDE2X.OVL"] {
            let meta = fs::symlink_metadata(rt.join(rel)).unwrap();
            assert!(meta.is_file(), "{rel} must be a real file");
        }
        assert_eq!(fs::read(rt.join("DATA/REL.DAT")).unwrap(), b"track");
        assert_eq!(fs::read(rt.join("DATA/ABS.DAT")).unwrap(), b"track");
        assert_eq!(fs::read(rt.join("GLIDE2X.OVL")).unwrap().len(), 200_000);
        // Unchanged on the next launch.
        assert_eq!(stage_source_tree(&game, &rt, None).unwrap().brought, 0);

        // The fallback GLIDE2X.OVL (e.g. SCREAMER_GLIDE2X_OVL) may be a link too.
        let ovl_link = outside.join("link.ovl");
        std::os::unix::fs::symlink("REAL.OVL", &ovl_link).unwrap();
        let rt2 = temp("symlink-rt2");
        bring_fallback_ovl(&ovl_link, &rt2).unwrap();
        let dst = rt2.join("GLIDE2X.OVL");
        assert!(fs::symlink_metadata(&dst).unwrap().is_file());
        assert_eq!(fs::read(&dst).unwrap().len(), 200_000);

        for d in [game, rt, rt2, outside] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn copies_get_only_the_owner_write_bit_and_read_only_files_are_not_linked() {
        use std::os::unix::fs::PermissionsExt;
        let game = temp("perm-game");
        let rt = temp("perm-rt");
        make_game(&game);
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        fs::set_permissions(
            game.join("DATA/track.dat"),
            fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        fs::set_permissions(game.join("DOS4GW.EXE"), fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(
            game.join("CONFIG/CHOICE.DAT"),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();

        let linked = stage_source_tree(&game, &rt, None).unwrap().linked;
        assert_eq!(linked, 0, "the read-only extender is copied, not linked");
        assert_eq!(mode(&rt.join("DATA/TRACK.DAT")), 0o640);
        assert_eq!(mode(&rt.join("DOS4GW.EXE")), 0o644);
        assert_eq!(mode(&rt.join("CONFIG/CHOICE.DAT")), 0o600);
        assert_eq!(
            mode(&game.join("DOS4GW.EXE")),
            0o444,
            "the original is untouched"
        );

        for d in [game, rt] {
            fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
            fs::remove_dir_all(d).unwrap();
        }
    }

    /// A saves folder and a runtime folder synced once (state recorded).
    fn synced_pair(name: &str) -> (PathBuf, PathBuf) {
        let saves = temp(&format!("{name}-saves"));
        let rt = temp(&format!("{name}-rt"));
        fs::create_dir_all(saves.join("CONFIG")).unwrap();
        fs::create_dir_all(rt.join("CONFIG")).unwrap();
        fs::write(saves.join("CONFIG/CHOICE.DAT"), b"progress-1").unwrap();
        set_mtime(&saves.join("CONFIG/CHOICE.DAT"), 5_000);
        sync_saves_in(&saves, &rt).unwrap();
        sync_saves_out(&rt, &saves).unwrap();
        assert!(rt.join(SAVE_STATE).is_file());
        (saves, rt)
    }

    #[test]
    fn a_restored_older_save_wins() {
        let (saves, rt) = synced_pair("restore");
        // The player plays on, then restores a backup kept with its old
        // timestamp (cp -p, rsync, Syncthing).
        fs::write(rt.join("CONFIG/CHOICE.DAT"), b"progress-2").unwrap();
        sync_saves_out(&rt, &saves).unwrap();
        fs::write(saves.join("CONFIG/CHOICE.DAT"), b"backup-0!!").unwrap();
        set_mtime(&saves.join("CONFIG/CHOICE.DAT"), 1_000);

        sync_saves_in(&saves, &rt).unwrap();
        assert_eq!(
            fs::read(rt.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"backup-0!!"
        );
        sync_saves_out(&rt, &saves).unwrap();
        assert_eq!(
            fs::read(saves.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"backup-0!!"
        );
        for d in [saves, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn a_session_killed_before_write_back_is_recovered() {
        let (saves, rt) = synced_pair("crash");
        // The game saved, but the launcher never copied it back.
        fs::write(rt.join("CONFIG/CHOICE.DAT"), b"progress-2").unwrap();
        set_mtime(&rt.join("CONFIG/CHOICE.DAT"), 1_000);

        sync_saves_in(&saves, &rt).unwrap();
        assert_eq!(
            fs::read(rt.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"progress-2"
        );
        assert_eq!(
            fs::read(saves.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"progress-2"
        );
        for d in [saves, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn a_same_second_same_size_change_still_syncs() {
        let (saves, rt) = synced_pair("same-second");
        // CHOICE.DAT has a fixed size; the game rewrites it within the second.
        let path = rt.join("CONFIG/CHOICE.DAT");
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, b"progress-2").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(before)
            .unwrap();

        sync_saves_out(&rt, &saves).unwrap();
        assert_eq!(
            fs::read(saves.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"progress-2"
        );
        // And nothing changes at the next launch.
        sync_saves_in(&saves, &rt).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"progress-2");
        for d in [saves, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn a_save_state_from_older_launchers_is_honoured() {
        let saves = temp("legacy-state-saves");
        let rt = temp("legacy-state-rt");
        fs::create_dir_all(saves.join("CONFIG")).unwrap();
        fs::create_dir_all(rt.join("config")).unwrap();
        fs::write(saves.join("CONFIG/CHOICE.DAT"), b"synced").unwrap();
        set_mtime(&saves.join("CONFIG/CHOICE.DAT"), 5_000);
        // The game saved later, but its file is older than the saves copy;
        // the old upper-case state says only the runtime copy changed.
        fs::write(rt.join("config/choice.dat"), b"played on").unwrap();
        set_mtime(&rt.join("config/choice.dat"), 1_000);
        fs::write(
            rt.join(SAVE_STATE),
            "6\t5000000000000\t6\t5000000000000\tCONFIG/CHOICE.DAT\n",
        )
        .unwrap();

        sync_saves_in(&saves, &rt).unwrap();
        assert_eq!(
            fs::read(saves.join("CONFIG/CHOICE.DAT")).unwrap(),
            b"played on"
        );
        assert!(rt.join(SAVE_STATE).is_file());
        // Not a save: never mirrored.
        fs::write(rt.join("config/DEFCTRL.CFG"), b"x").unwrap();
        sync_saves_out(&rt, &saves).unwrap();
        assert!(!saves.join("CONFIG/DEFCTRL.CFG").exists());
        assert!(!saves.join(SAVE_STATE).exists());
        for d in [saves, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn saves_are_recognised_by_name_and_folder() {
        for save in [
            "GAME1.SAV",
            "choice.dat",
            "CONFIG/CHOICE.DAT",
            "config/x.sav",
            "Replays/a.rpl",
        ] {
            assert!(is_save(save), "{save}");
        }
        for other in [
            "CONFIG/SETUP.CFG",
            "REPLAYS/A.SAV",
            "DATA/CHOICE.DAT",
            ".screamer-saves",
            "A.RPL",
        ] {
            assert!(!is_save(other), "{other}");
        }
    }

    #[test]
    fn cue_sheet_names_its_image() {
        let dir = temp("cue");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("GAME.INS"),
            "FILE \"GAME.GOG\" BINARY\nFILE track2.ogg WAVE\nFILE \"..\\IMG\\Data.Bin\" BINARY\n",
        )
        .unwrap();
        assert_eq!(
            cd_image_files(Some(&dir.join("GAME.INS"))),
            [
                dir.join("GAME.INS"),
                dir.join("GAME.GOG"),
                dir.join("track2.ogg"),
                dir.parent().unwrap().join("IMG/Data.Bin"),
            ]
        );
        assert!(cd_image_files(None).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    /// A raw CD image big enough to be one, not a floppy or a data file.
    fn write_raw_image(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let f = fs::File::create(path).unwrap();
        f.set_len(MIN_RAW_IMAGE_BYTES).unwrap();
    }

    /// Quiver feedback (2026-11-09, v0.8.2 on Windows): Screamer and
    /// Screamer Rally asked for their CD although its image was in the game
    /// folder. Only a few fixed folders were searched, so an image in any
    /// other subfolder was missed and DOSBox got an empty CD drive.
    #[test]
    fn cd_image_in_any_subfolder_of_the_game_is_found() {
        let game = temp("cd in subfolder");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("STARTH.EXE"), b"exe").unwrap();
        fs::create_dir_all(game.join("DATA")).unwrap();
        fs::write(game.join("DATA/GEN.DAT"), b"\x01\x02 not a cue sheet").unwrap();
        fs::create_dir_all(game.join("CD")).unwrap();
        fs::write(
            game.join("CD/Screamer.cue"),
            "FILE \"Screamer.bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\n",
        )
        .unwrap();
        write_raw_image(&game.join("CD/Screamer.bin"));
        let found = find_cd_image(&game, &game);
        assert_eq!(found.image, Some(game.join("CD/Screamer.cue")));
        assert!(found.broken.is_empty());

        // One next to the game wins over one in a subfolder.
        fs::write(game.join("SCREAM.DAT"), "FILE \"SCREAM.gog\" BINARY\n").unwrap();
        write_raw_image(&game.join("SCREAM.gog"));
        assert_eq!(
            find_cd_image(&game, &game).image,
            Some(game.join("SCREAM.DAT"))
        );
        fs::remove_dir_all(game).unwrap();
    }

    /// Screamer Rally's own install (`hdd/SR`) beside a CloneCD rip
    /// (`.img`/`.ccd`/`.sub`, no cue sheet) in a folder of any name.
    #[test]
    fn a_bare_raw_image_is_mounted_when_there_is_no_cue_sheet() {
        let root = temp("bare img");
        let exe_dir = root.join("hdd/SR");
        fs::create_dir_all(&exe_dir).unwrap();
        fs::write(exe_dir.join("STRT3FX.EXE"), b"exe").unwrap();
        write_raw_image(&root.join("Screamer Rally CD/srally.img"));
        fs::write(root.join("Screamer Rally CD/srally.ccd"), "[CloneCD]\n").unwrap();
        fs::write(root.join("Screamer Rally CD/srally.sub"), b"sub").unwrap();
        assert_eq!(
            find_cd_image(&root, &exe_dir).image,
            Some(root.join("Screamer Rally CD/srally.img"))
        );
        // A small .img is a floppy or some data file, not a CD.
        let small = temp("small img");
        fs::create_dir_all(&small).unwrap();
        fs::write(small.join("disk.img"), vec![0u8; 1_474_560]).unwrap();
        assert_eq!(find_cd_image(&small, &small).image, None);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(small).unwrap();
    }

    /// A cue sheet whose image is missing (renamed, or left out of a copy)
    /// makes DOSBox's IMGMOUNT fail: it is skipped and reported.
    #[test]
    fn a_cue_sheet_without_its_image_is_skipped_and_reported() {
        let game = temp("broken cue");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("Screamer.cue"), "FILE \"Track 01.bin\" BINARY\n").unwrap();
        let found = find_cd_image(&game, &game);
        assert_eq!(found.image, None);
        assert_eq!(found.broken, [game.join("Screamer.cue")]);

        // The cue sheet's names are matched like DOSBox matches them,
        // ignoring case.
        write_raw_image(&game.join("TRACK 01.BIN"));
        let found = find_cd_image(&game, &game);
        assert_eq!(found.image, Some(game.join("Screamer.cue")));
        assert!(found.broken.is_empty());
        fs::remove_dir_all(game).unwrap();
    }

    /// DOSBox Staging on Windows opens host paths through narrow (ANSI)
    /// calls; under Wine its MinGW build ended at IMGMOUNT with "Cannot
    /// convert character sequence" for a game in `Jogos Ação`. A path with
    /// other than ASCII goes to DOSBox as Windows' short (8.3) name.
    #[test]
    fn dosbox_gets_a_plain_ascii_path() {
        let ascii = Path::new(r"C:\Games\Screamer\SCREAM.DAT");
        assert_eq!(
            path_for_dosbox(ascii, Some(r"C:\Games\SCREAM~1\SCREAM.DAT".into())),
            ascii
        );
        let accented = Path::new(r"D:\Jogos Ação\Screamer\SCREAM.DAT");
        assert_eq!(
            path_for_dosbox(accented, Some(r"D:\JOGOSA~1\Screamer\SCREAM.DAT".into())),
            Path::new(r"D:\JOGOSA~1\Screamer\SCREAM.DAT")
        );
        // No short names on that drive: the path as it is, and a warning.
        assert_eq!(path_for_dosbox(accented, None), accented);
        let note = dosbox_path_warning(accented).unwrap();
        assert!(note.contains("Jogos Ação"), "{note}");
        assert!(note.contains("plain letters"), "{note}");
        assert_eq!(dosbox_path_warning(ascii), None);
    }

    #[test]
    fn a_missing_cd_image_is_said_before_the_game_asks_for_its_disc() {
        let game = temp("no cd");
        fs::create_dir_all(&game).unwrap();
        let found = find_cd_image(&game, &game);
        let note = found.warning(Game::ScreamerRally, &game).unwrap();
        assert!(note.contains("Screamer Rally"), "{note}");
        assert!(note.contains("ask for its CD"), "{note}");
        assert!(note.contains(".cue"), "{note}");

        fs::write(game.join("Rally.cue"), "FILE \"Rally.bin\" BINARY\n").unwrap();
        let note = find_cd_image(&game, &game)
            .warning(Game::ScreamerRally, &game)
            .unwrap();
        assert!(note.contains("Rally.cue"), "{note}");
        assert!(note.contains("Rally.bin"), "{note}");

        write_raw_image(&game.join("Rally.bin"));
        assert_eq!(
            find_cd_image(&game, &game).warning(Game::ScreamerRally, &game),
            None
        );
        fs::remove_dir_all(game).unwrap();
    }

    #[test]
    fn a_mounted_image_set_is_never_staged() {
        let game = temp("imageset-game");
        let rt = temp("imageset-rt");
        make_game(&game);
        fs::create_dir_all(game.join("CD")).unwrap();
        fs::write(
            game.join("CD/game.cue"),
            "FILE \"..\\cd\\Track01.raw\" BINARY\n",
        )
        .unwrap();
        fs::write(game.join("CD/TRACK01.RAW"), b"raw track").unwrap();
        fs::write(game.join("CD/spare.iso"), b"iso").unwrap();
        fs::write(game.join("CD/extra.MDF"), b"mdf").unwrap();

        stage_source_tree(&game, &rt, Some(&game.join("CD/game.cue"))).unwrap();
        for absent in [
            "CD/GAME.CUE",
            "CD/TRACK01.RAW",
            "CD/SPARE.ISO",
            "CD/EXTRA.MDF",
            "SCREAM.GOG",
        ] {
            assert!(
                !native(&rt, absent).exists(),
                "{absent} must stay in the source"
            );
        }
        assert!(rt.join("DATA/TRACK.DAT").is_file());
        // Without a mounted image (directory mount), image-like files are staged.
        let rt2 = temp("imageset-rt2");
        stage_source_tree(&game, &rt2, None).unwrap();
        assert!(rt2.join("CD/SPARE.ISO").is_file());
        for d in [game, rt, rt2] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_and_looping_folders_are_skipped_with_a_warning() {
        use std::os::unix::fs::PermissionsExt;
        let game = temp("walk-game");
        let rt = temp("walk-rt");
        make_game(&game);
        fs::create_dir_all(game.join("LOCKED")).unwrap();
        fs::write(game.join("LOCKED/x.dat"), b"x").unwrap();
        fs::set_permissions(game.join("LOCKED"), fs::Permissions::from_mode(0o000)).unwrap();
        std::os::unix::fs::symlink("..", game.join("DATA/loop")).unwrap();

        let staged = stage_source_tree(&game, &rt, None).unwrap();
        assert!(rt.join("DATA/TRACK.DAT").is_file());
        assert!(!rt.join("DATA/LOOP").exists());
        assert!(
            staged.warnings.iter().any(|w| w.contains("loop")),
            "{staged:?}"
        );
        if fs::read_dir(game.join("LOCKED")).is_err() {
            assert!(
                staged.warnings.iter().any(|w| w.contains("LOCKED")),
                "{staged:?}"
            );
        }

        fs::set_permissions(game.join("LOCKED"), fs::Permissions::from_mode(0o755)).unwrap();
        for d in [game, rt] {
            fs::remove_dir_all(d).unwrap();
        }
    }
}
