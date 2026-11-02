//! Source game directory discovery, read-only runtime staging, binary patching,
//! save-game synchronization, and `dosbox-staging` invocation.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

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

/// Result of scanning a user's game folder.
#[derive(Debug, Clone)]
pub struct DiscoveredGame {
    pub root_dir: PathBuf,
    pub exe_dir: PathBuf,
    pub game: Game,
    pub executables: Vec<DiscoveredExe>,
    pub cd_image: Option<PathBuf>,
    pub glide2x_ovl_source: Option<PathBuf>,
    pub has_genuine_dos_glide2x: bool,
    pub has_windows_stub_glide2x: bool,
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

    for cand in &candidate_dirs {
        if !cand.is_dir() {
            continue;
        }
        let found = scan_exes_in_dir(cand)?;
        if !found.is_empty() {
            exe_dir = Some(cand.clone());
            executables = found;
            break;
        }
    }

    let exe_dir = exe_dir.ok_or_else(|| {
        format!(
            "No supported Screamer, Screamer 2, or Screamer Rally executables found in {}",
            dir.display()
        )
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
    let cd_image = find_cd_image(dir, &exe_dir);

    Ok(DiscoveredGame {
        root_dir: dir.to_path_buf(),
        exe_dir,
        game,
        executables,
        cd_image,
        glide2x_ovl_source,
        has_genuine_dos_glide2x,
        has_windows_stub_glide2x,
    })
}

fn scan_exes_in_dir(dir: &Path) -> Result<Vec<DiscoveredExe>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
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
        if let Some(ed) = edition::identify_sha256(&digest) {
            out.push(DiscoveredExe {
                path,
                file_name: upper,
                edition: ed,
                sha256: digest,
            });
        }
    }
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(out)
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

fn find_cd_image(root_dir: &Path, exe_dir: &Path) -> Option<PathBuf> {
    let mut search_dirs = vec![
        exe_dir.to_path_buf(),
        root_dir.to_path_buf(),
        root_dir.join("Screamer_Rally_ISO"),
        root_dir.join("iso"),
    ];
    if let Some(parent) = exe_dir.parent() {
        search_dirs.push(parent.to_path_buf());
        search_dirs.push(parent.join("Screamer_Rally_ISO"));
    }

    // Priority: .ins -> .cue -> CUE-formatted .dat (e.g. SCREAM.DAT, GAME.DAT) -> .iso -> .gog
    for ext_target in ["ins", "cue", "dat", "iso", "gog"] {
        for d in &search_dirs {
            let Ok(entries) = fs::read_dir(d) else {
                continue;
            };
            let mut candidates: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    if !p.is_file() {
                        return false;
                    }
                    let matches_ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case(ext_target));
                    if !matches_ext {
                        return false;
                    }
                    if ext_target == "dat" {
                        // Only treat a .DAT file as a CD image cue sheet if it starts with `FILE "`
                        let Ok(meta) = fs::metadata(p) else {
                            return false;
                        };
                        if meta.len() > 16_384 {
                            return false;
                        }
                        let Ok(head) = fs::read_to_string(p) else {
                            return false;
                        };
                        return head.trim_start().to_ascii_uppercase().starts_with("FILE ");
                    }
                    true
                })
                .collect();
            candidates.sort();
            if let Some(first) = candidates.into_iter().next() {
                return Some(first);
            }
        }
    }
    None
}

/// Result of scanning a folder (or parent trilogy folder) for all 3 Screamer games.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredTrilogy {
    /// Discovered games indexed by `Game::index()` (`0` = Screamer, `1` = Screamer 2, `2` = Screamer Rally).
    pub games: [Option<DiscoveredGame>; 3],
    /// If the user's path pointed directly at one specific game folder, which game it was.
    pub direct_game: Option<Game>,
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

    if let Ok(direct) = scan_game_dir(dir) {
        let idx = direct.game.index();
        out.direct_game = Some(direct.game);
        out.games[idx] = Some(direct);
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
            if cand.is_dir()
                && let Ok(disc) = scan_game_dir(&cand)
            {
                let idx = disc.game.index();
                if out.games[idx].is_none() {
                    out.games[idx] = Some(disc);
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

/// Prepares `<runtime_dir>` by staging game files from `discovered.exe_dir`
/// ([`stage_source_tree`]), writing patched executables into `<runtime_dir>`,
/// generating `SCREAMER.CFG`, `CONFIG/SETUP.CFG`, `CONFIG/CONTROLS.CFG`, and
/// `dosbox-screamer.conf`, and syncing saves from `saves_dir`.
pub fn prepare_runtime(
    discovered: &DiscoveredGame,
    settings: &Settings,
    runtime_dir: &Path,
    saves_dir: Option<&Path>,
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
    if let Some(ovl_src) = &discovered.glide2x_ovl_source {
        let ovl_dst = runtime_dir.join("GLIDE2X.OVL");
        if !ovl_dst.is_file() {
            let stamp = fs::metadata(ovl_src)
                .map(|m| stamp_of(&m))
                .map_err(|e| format!("{}: {e}", ovl_src.display()))?;
            bring_over(ovl_src, &ovl_dst, "GLIDE2X.OVL", stamp)
                .map_err(|e| format!("{}: {e}", ovl_dst.display()))?;
        }
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
        install(&dst, &bytes).map_err(|e| format!("{}: {e}", dst.display()))?;
        patched_exes.push((exe.file_name.clone(), count));
    }

    // Write SCREAMER.CFG, CONFIG/SETUP.CFG, and CONFIG/CONTROLS.CFG.
    let screamer_cfg_path = runtime_dir.join("SCREAMER.CFG");
    install(&screamer_cfg_path, build_screamer_cfg(discovered.game))
        .map_err(|e| format!("{}: {e}", screamer_cfg_path.display()))?;

    let setup_cfg_path = config_dir.join("SETUP.CFG");
    install(&setup_cfg_path, build_setup_cfg(discovered.game, 7))
        .map_err(|e| format!("{}: {e}", setup_cfg_path.display()))?;

    let controls_cfg_path = config_dir.join("CONTROLS.CFG");
    let keys = KeyBindings::default_for(discovered.game);
    install(
        &controls_cfg_path,
        build_controls_cfg(discovered.game, keys),
    )
    .map_err(|e| format!("{}: {e}", controls_cfg_path.display()))?;

    // Sync user saves in if a saves directory is configured.
    if let Some(saves) = saves_dir {
        sync_saves_in(saves, runtime_dir)?;
    }

    let selected = discovered.select_executable(settings.renderer_choice())?;
    let race_res = settings.race_resolution();
    let fullscreen = race_res == Some(crate::config::FULLSCREEN);
    let res = match race_res {
        None | Some(crate::config::FULLSCREEN) => (1280, 960),
        Some(wh) => wh,
    };
    let widescreen = settings.widescreen_enabled();
    let vsync = settings.vsync.unwrap_or(true);
    let voodoo_enabled = selected.edition.role == Renderer::Glide3dfx;
    let filter = settings.effective_filter();
    let texture_filtering = matches!(filter, TextureFilter::Smooth | TextureFilter::Fsr);
    let glshader = match filter {
        TextureFilter::Original => "crt-auto",
        TextureFilter::Fsr => "interpolation/fsr",
        TextureFilter::Sharp => "sharp",
        TextureFilter::Smooth => "bilinear",
    };

    let conf_text = build_dosbox_conf(&DosboxConfOptions {
        fullscreen,
        resolution: res,
        widescreen,
        vsync,
        voodoo_enabled,
        texture_filtering,
        glshader,
        c_drive_dir: runtime_dir,
        cd_image_path: discovered.cd_image.as_deref(),
        exe_name: &selected.file_name,
        exit_after_run: true,
    });
    let conf_path = runtime_dir.join("dosbox-screamer.conf");
    install(&conf_path, conf_text).map_err(|e| format!("{}: {e}", conf_path.display()))?;

    Ok(PreparedRuntime {
        runtime_dir: runtime_dir.to_path_buf(),
        conf_path,
        selected_exe: selected.file_name.clone(),
        selected_edition: selected.edition,
        patched_exes,
        warnings: staged.warnings,
    })
}

/// Name of the staging manifest inside the runtime folder.
pub const MANIFEST: &str = ".screamer-manifest";

/// Size and modification time (whole seconds) of a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub size: u64,
    pub mtime: u64,
}

/// Relative source path (with `/` separators) to its stamp.
pub type Listing = BTreeMap<String, Stamp>;

/// Which files to bring into the runtime folder, and the manifest to write
/// once they are there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub copy: Vec<String>,
    pub manifest: Listing,
}

/// The runtime-folder path for a source path: DOS names, upper case.
fn runtime_rel(rel: &str) -> String {
    rel.to_ascii_uppercase()
}

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

/// Decides which files to bring over: missing in the runtime folder, or
/// changed in the source since the last sync (per the manifest) unless the
/// game owns them.
pub fn plan(source: &Listing, runtime_has: &dyn Fn(&str) -> bool, manifest: &Listing) -> Plan {
    let mut copy = Vec::new();
    let mut next = Listing::new();
    for (rel, stamp) in source {
        let present = runtime_has(rel);
        let changed = manifest.get(rel) != Some(stamp);
        if !present || (changed && !is_game_written(rel)) {
            copy.push(rel.clone());
            next.insert(rel.clone(), *stamp);
        } else {
            next.insert(rel.clone(), *manifest.get(rel).unwrap_or(stamp));
        }
    }
    Plan {
        copy,
        manifest: next,
    }
}

pub fn parse_manifest(text: &str) -> Listing {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let size = parts.next()?.parse().ok()?;
            let mtime = parts.next()?.parse().ok()?;
            let rel = parts.next()?.to_owned();
            Some((rel, Stamp { size, mtime }))
        })
        .collect()
}

pub fn format_manifest(listing: &Listing) -> String {
    listing
        .iter()
        .map(|(rel, s)| format!("{}\t{}\t{}\n", s.size, s.mtime, rel))
        .collect()
}

fn mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

fn stamp_of(meta: &fs::Metadata) -> Stamp {
    Stamp {
        size: meta.len(),
        mtime: mtime_secs(meta),
    }
}

/// Deepest folder level staged below the game folder (a guard against
/// symbolic-link loops where folder identities are not available).
const MAX_DEPTH: usize = 32;

/// Lists every regular file below `root` (symbolic links are followed, as
/// the user's folder may use them). A subfolder that cannot be read is
/// skipped with a warning, and a folder already visited (a symbolic-link
/// loop) is not entered again. Only an unreadable `root` is an error.
pub fn list(root: &Path) -> io::Result<(Listing, Vec<String>)> {
    struct Walk<'a> {
        root: &'a Path,
        out: Listing,
        warnings: Vec<String>,
        #[cfg(unix)]
        seen: std::collections::BTreeSet<(u64, u64)>,
    }
    impl Walk<'_> {
        /// False when `dir` was visited before.
        fn first_visit(&mut self, _meta: &fs::Metadata) -> bool {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                self.seen.insert((_meta.dev(), _meta.ino()))
            }
            #[cfg(not(unix))]
            true
        }

        fn dir(&mut self, dir: &Path, depth: usize) -> io::Result<()> {
            let entries = match fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(e) if depth > 0 => {
                    self.warnings
                        .push(format!("skipped unreadable folder {}: {e}", dir.display()));
                    return Ok(());
                }
                Err(e) => return Err(e),
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = fs::metadata(&path) else {
                    continue;
                };
                if meta.is_dir() {
                    if depth + 1 > MAX_DEPTH || !self.first_visit(&meta) {
                        self.warnings
                            .push(format!("skipped looping folder {}", path.display()));
                        continue;
                    }
                    self.dir(&path, depth + 1)?;
                } else if meta.is_file() {
                    let rel = path
                        .strip_prefix(self.root)
                        .expect("walk stays below root")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/");
                    self.out.insert(rel, stamp_of(&meta));
                }
            }
            Ok(())
        }
    }
    let mut walk = Walk {
        root,
        out: Listing::new(),
        warnings: Vec::new(),
        #[cfg(unix)]
        seen: Default::default(),
    };
    if let Ok(meta) = fs::metadata(root) {
        walk.first_visit(&meta);
    }
    walk.dir(root, 0)?;
    Ok((walk.out, walk.warnings))
}

fn native(root: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(root.to_path_buf(), |p, part| p.join(part))
}

/// Lets the owner write `path`: the owner write bit on Unix (not
/// `set_readonly(false)`, which would make it writable by everyone), the
/// read-only attribute cleared on Windows.
fn make_writable(path: &Path) -> io::Result<()> {
    let mut perm = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(perm.mode() | 0o200);
    }
    #[cfg(not(unix))]
    {
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
    }
    fs::set_permissions(path, perm)
}

/// Removes a file (or a link) from the runtime folder. A read-only copy is
/// made writable first only if a plain removal fails (Windows), so the
/// attributes of a hard-linked original are never touched otherwise.
fn remove_file(path: &Path) -> io::Result<()> {
    if path.symlink_metadata().is_err() {
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(_) => {
            make_writable(path)?;
            fs::remove_file(path)
        }
    }
}

/// Writes `bytes` to `path` as a fresh file, never through a link to the
/// user's folder: an existing file or link is removed first.
pub fn install(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    remove_file(path)?;
    fs::write(path, bytes)
}

/// Copies `from` over `path` as a fresh, writable file (never through a link).
pub fn install_copy(from: &Path, path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    remove_file(path)?;
    fs::copy(from, path)?;
    make_writable(path)
}

/// Brings one source file into the runtime folder as a whole-file swap: a
/// hard link for static files when both folders share a volume, a writable
/// copy otherwise. Copies keep the source's modification time so save
/// mirroring can compare them later. Returns true when hard-linked.
fn bring_over(from: &Path, to: &Path, rel: &str, stamp: Stamp) -> io::Result<bool> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    remove_file(to)?;
    // A read-only original is copied: the link would share its read-only
    // state, and making it writable later would change the user's file.
    let linkable = is_static(rel) && fs::metadata(from).is_ok_and(|m| !m.permissions().readonly());
    // A hard link to a symbolic link would be the link itself (dangling
    // when relative); link the file it points to.
    if linkable
        && let Ok(target) = fs::canonicalize(from)
        && fs::hard_link(&target, to).is_ok()
    {
        return Ok(true);
    }
    install_copy(from, to)?;
    let file = fs::OpenOptions::new().write(true).open(to)?;
    file.set_modified(UNIX_EPOCH + Duration::from_secs(stamp.mtime))?;
    Ok(false)
}

/// Removes symbolic links left in the runtime folder by older launchers,
/// which pointed straight into the user's folder.
fn remove_legacy_links(dir: &Path) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            fs::remove_file(&path).or_else(|_| fs::remove_dir(&path))?;
        } else if meta.is_dir() {
            remove_legacy_links(&path)?;
        }
    }
    Ok(())
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

/// What [`stage_source_tree`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Staged {
    pub brought: usize,
    pub linked: usize,
    pub warnings: Vec<String>,
}

/// Brings `dst_dir` up to date with `src_dir` without ever writing into
/// `src_dir`: real directories in the runtime folder, one file at a time
/// (hard-linked or copied), skipping files the launcher generates and CD
/// images. When the launcher mounts a CD image from the source, every file
/// with a CD-image extension and every file its cue sheet names stays there.
pub fn stage_source_tree(
    src_dir: &Path,
    dst_dir: &Path,
    cd_image: Option<&Path>,
) -> Result<Staged, String> {
    let err = |e: io::Error| format!("{}: {e}", dst_dir.display());
    fs::create_dir_all(dst_dir).map_err(err)?;
    remove_legacy_links(dst_dir).map_err(err)?;
    let skip = cd_image_files(cd_image);
    let (listing, warnings) = list(src_dir).map_err(|e| format!("{}: {e}", src_dir.display()))?;
    let source: Listing = listing
        .into_iter()
        .filter(|(rel, stamp)| {
            let path = normalize(&native(src_dir, rel));
            !is_generated(rel, *stamp)
                && !(cd_image.is_some() && has_cd_image_extension(rel))
                && !skip.iter().any(|s| same_path_ignoring_case(s, &path))
        })
        .collect();
    let manifest_path = dst_dir.join(MANIFEST);
    let manifest = fs::read_to_string(&manifest_path)
        .map(|t| parse_manifest(&t))
        .unwrap_or_default();
    let plan = plan(
        &source,
        &|rel| native(dst_dir, &runtime_rel(rel)).is_file(),
        &manifest,
    );
    let mut linked = 0;
    for rel in &plan.copy {
        let from = native(src_dir, rel);
        let to = native(dst_dir, &runtime_rel(rel));
        if bring_over(&from, &to, rel, source[rel]).map_err(|e| format!("{}: {e}", to.display()))? {
            linked += 1;
        }
    }
    fs::write(&manifest_path, format_manifest(&plan.manifest))
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    Ok(Staged {
        brought: plan.copy.len(),
        linked,
        warnings,
    })
}

fn is_save_file(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.ends_with(".SAV") || upper == "CHOICE.DAT"
}

/// Name of the save-sync state inside the runtime folder.
pub const SAVE_STATE: &str = ".screamer-saves";

/// A save file's size and modification time (nanoseconds), precise enough
/// to see a rewrite within the same second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveStamp {
    pub size: u64,
    pub mtime_ns: u128,
}

/// Per save (upper-case relative path): its stamp in the saves folder and in
/// the runtime folder as of the last sync.
pub type SaveState = BTreeMap<String, (SaveStamp, SaveStamp)>;

fn save_stamp(path: &Path) -> Option<SaveStamp> {
    let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
    Some(SaveStamp {
        size: meta.len(),
        mtime_ns: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos()),
    })
}

pub fn parse_save_state(text: &str) -> SaveState {
    text.lines()
        .filter_map(|line| {
            let mut p = line.splitn(5, '\t');
            let mut num = || p.next()?.parse::<u128>().ok();
            let saves = SaveStamp {
                size: num()? as u64,
                mtime_ns: num()?,
            };
            let runtime = SaveStamp {
                size: num()? as u64,
                mtime_ns: num()?,
            };
            Some((p.next()?.to_owned(), (saves, runtime)))
        })
        .collect()
}

pub fn format_save_state(state: &SaveState) -> String {
    state
        .iter()
        .map(|(rel, (s, r))| {
            format!(
                "{}\t{}\t{}\t{}\t{rel}\n",
                s.size, s.mtime_ns, r.size, r.mtime_ns
            )
        })
        .collect()
}

/// The persistent saves below `root` (`*.SAV` and `CHOICE.DAT` in the root
/// and `CONFIG/`, `REPLAYS/*.RPL`), by upper-case relative path.
fn save_files(root: &Path) -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    let mut scan = |dir: &str, pred: &dyn Fn(&str) -> bool| {
        let Ok(entries) = fs::read_dir(native(root, dir)) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if path.is_file() && pred(name) {
                let rel = if dir.is_empty() {
                    name.to_ascii_uppercase()
                } else {
                    format!("{dir}/{}", name.to_ascii_uppercase())
                };
                out.insert(rel, path);
            }
        }
    };
    scan("", &is_save_file);
    scan("CONFIG", &is_save_file);
    scan("REPLAYS", &|n| n.to_ascii_uppercase().ends_with(".RPL"));
    out
}

/// Copies a save as a fresh file that keeps its exact modification time.
fn copy_save(from: &Path, to: &Path) -> io::Result<()> {
    let modified = fs::metadata(from)?.modified()?;
    install_copy(from, to)?;
    fs::OpenOptions::new()
        .write(true)
        .open(to)?
        .set_modified(modified)
}

fn read_save_state(runtime_dir: &Path) -> Option<SaveState> {
    fs::read_to_string(runtime_dir.join(SAVE_STATE))
        .ok()
        .map(|t| parse_save_state(&t))
}

fn write_save_state(runtime_dir: &Path, state: &SaveState) -> Result<(), String> {
    let path = runtime_dir.join(SAVE_STATE);
    install(&path, format_save_state(state)).map_err(|e| format!("{}: {e}", path.display()))
}

/// Brings persistent saves (`*.SAV`, `CHOICE.DAT`, `REPLAYS/*.RPL`) into
/// `runtime_dir` at launch. Against the state recorded at the last sync:
/// a save changed in `saves_dir` (the player restored or copied one in, even
/// an older file) wins; otherwise a save changed in the runtime folder (the
/// last session ended before its saves were copied back) is copied out.
/// Saves with no recorded state yet: the newer one wins ([`mirror_file`]).
pub fn sync_saves_in(saves_dir: &Path, runtime_dir: &Path) -> Result<(), String> {
    if !saves_dir.exists() {
        return Ok(());
    }
    let err = |p: &Path, e: io::Error| format!("copy {}: {e}", p.display());
    let recorded = read_save_state(runtime_dir).unwrap_or_default();
    let in_saves = save_files(saves_dir);
    let in_runtime = save_files(runtime_dir);
    let mut state = SaveState::new();
    for rel in in_saves.keys().chain(in_runtime.keys()) {
        if state.contains_key(rel) {
            continue;
        }
        let sp = in_saves
            .get(rel)
            .cloned()
            .unwrap_or_else(|| native(saves_dir, rel));
        let rp = in_runtime
            .get(rel)
            .cloned()
            .unwrap_or_else(|| native(runtime_dir, rel));
        let (s, r) = (save_stamp(&sp), save_stamp(&rp));
        match recorded.get(rel) {
            None => {
                mirror_file(&sp, &rp).map_err(|e| err(&rp, e))?;
            }
            Some((rs, rr)) => {
                if s.is_some() && (s != Some(*rs) || r.is_none()) {
                    copy_save(&sp, &rp).map_err(|e| err(&rp, e))?;
                } else if r.is_some() && (r != Some(*rr) || s.is_none()) {
                    copy_save(&rp, &sp).map_err(|e| err(&sp, e))?;
                }
            }
        }
        if let (Some(s), Some(r)) = (save_stamp(&sp), save_stamp(&rp)) {
            state.insert(rel.clone(), (s, r));
        }
    }
    write_save_state(runtime_dir, &state)
}

/// Copies persistent saves (`*.SAV`, `CHOICE.DAT`, `REPLAYS/*.RPL`) from
/// `runtime_dir` back to `saves_dir` when their contents differ, and records
/// the synced state for the next launch.
pub fn sync_saves_out(runtime_dir: &Path, saves_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(saves_dir).map_err(|e| format!("{}: {e}", saves_dir.display()))?;
    let in_saves = save_files(saves_dir);
    let mut state = SaveState::new();
    for (rel, rp) in save_files(runtime_dir) {
        let sp = in_saves
            .get(&rel)
            .cloned()
            .unwrap_or_else(|| native(saves_dir, &rel));
        let same = matches!((fs::read(&rp), fs::read(&sp)), (Ok(a), Ok(b)) if a == b);
        if !same {
            copy_save(&rp, &sp).map_err(|e| format!("copy {}: {e}", sp.display()))?;
        }
        if let (Some(s), Some(r)) = (save_stamp(&sp), save_stamp(&rp)) {
            state.insert(rel, (s, r));
        }
    }
    write_save_state(runtime_dir, &state)
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() && !normalized.has_root() {
                    normalized.push("..");
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Resolves a path even when its final components do not exist yet.
/// Canonicalizing the nearest existing ancestor resolves symlinks and
/// Windows path aliases; the remaining suffix is normalized lexically. A
/// path that cannot be canonicalized for another reason is normalized
/// lexically as a whole.
pub fn resolve(path: &Path, cwd: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(base) => {
                let mut resolved = base;
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(normalize(&resolved));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let Some(component) = ancestor.components().next_back() else {
                    return Err(format!("cannot resolve {}", path.display()));
                };
                match component {
                    std::path::Component::Normal(name) => suffix.push(name.to_os_string()),
                    std::path::Component::ParentDir => suffix.push("..".into()),
                    std::path::Component::CurDir => {}
                    _ => return Err(format!("cannot resolve {}: {error}", path.display())),
                }
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| format!("cannot resolve {}: {error}", path.display()))?;
            }
            // Some volumes cannot be canonicalized at all (Windows RAM
            // disks, `subst` drives, some network shares: "Incorrect
            // function"); compare their absolute path as written instead of
            // refusing to launch.
            Err(_) => return Ok(normalize(&absolute)),
        }
    }
}

fn check_separate_from(game: &Path, folder: &Path, cwd: &Path) -> Result<(), String> {
    let game_real = resolve(game, cwd)?;
    let folder_real = resolve(folder, cwd)?;
    if folder_real.starts_with(&game_real) || game_real.starts_with(&folder_real) {
        return Err(format!(
            "{} must be outside the game folder {}",
            folder.display(),
            game.display()
        ));
    }
    Ok(())
}

/// Refuses a runtime (or saves) folder that overlaps the game folder, since
/// the launcher writes there and the game folder must stay untouched.
/// Nothing is created while checking.
pub fn check_separate(game: &Path, folder: &Path) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|error| format!("current directory: {error}"))?;
    check_separate_from(game, folder, &cwd)
}

/// Copies `from` over `to` when `to` is missing or older: an earlier
/// modification time, or the same second with another size. The copy keeps
/// `from`'s modification time. Returns true when a copy was made. Used for
/// saves that have no recorded sync state yet.
pub fn mirror_file(from: &Path, to: &Path) -> io::Result<bool> {
    let Ok(meta) = fs::metadata(from) else {
        return Ok(false);
    };
    let stamp = stamp_of(&meta);
    if let Ok(dst) = fs::metadata(to) {
        let dst_mtime = mtime_secs(&dst);
        if dst_mtime > stamp.mtime || (dst_mtime == stamp.mtime && dst.len() == stamp.size) {
            return Ok(false);
        }
    }
    install_copy(from, to)?;
    let file = fs::OpenOptions::new().write(true).open(to)?;
    file.set_modified(UNIX_EPOCH + Duration::from_secs(stamp.mtime))?;
    Ok(true)
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
    use std::time::SystemTime;

    fn stamp(size: u64, mtime: u64) -> Stamp {
        Stamp { size, mtime }
    }

    fn listing(items: &[(&str, Stamp)]) -> Listing {
        items.iter().map(|(r, s)| (r.to_string(), *s)).collect()
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
        list(root)
            .unwrap()
            .0
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
    fn first_sync_brings_everything_over() {
        let src = listing(&[
            ("DATA/a.dat", stamp(2, 2)),
            ("CONFIG/CHOICE.DAT", stamp(3, 3)),
        ]);
        let p = plan(&src, &|_| false, &Listing::new());
        assert_eq!(p.copy, ["CONFIG/CHOICE.DAT", "DATA/a.dat"]);
        assert_eq!(p.manifest, src);
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
        let p = plan(&new, &|_| true, &old);
        assert_eq!(p.copy, ["DATA/a.dat"]);
        assert_eq!(p.manifest["CONFIG/CHOICE.DAT"], stamp(1, 1));
        assert!(plan(&new, &|_| true, &new).copy.is_empty());
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
    fn manifest_round_trips() {
        let l = listing(&[("DATA/a b.dat", stamp(10, 20))]);
        assert_eq!(parse_manifest(&format_manifest(&l)), l);
    }

    #[test]
    fn staging_never_modifies_the_user_folder() {
        let game = temp("stage-game");
        let rt = temp("stage-rt");
        make_game(&game);
        let before = snapshot(&game);

        let cd = game.join("SCREAM.DAT");
        let Staged {
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
            Staged::default()
        );

        // The game writes data files in place; the launcher swaps whole files.
        fs::write(rt.join("DATA/TRACK.DAT"), b"game wrote").unwrap();
        fs::write(rt.join("CONFIG/CHOICE.DAT"), b"played").unwrap();
        install(rt.join("DOS4GW.EXE").as_path(), b"replaced").unwrap();
        install(rt.join("SCREAMER.CFG").as_path(), b"generated").unwrap();
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
        assert!(check_separate(&game, &game.join("runtime")).is_err());
        assert!(check_separate(&game.join("."), game.parent().unwrap()).is_err());
        assert!(check_separate(&game, &temp("elsewhere")).is_ok());
        fs::remove_dir_all(game).unwrap();
    }

    #[test]
    fn nonexistent_relative_and_dotdot_children_are_refused_without_creation() {
        let game = temp("relative-overlap");
        fs::create_dir_all(&game).unwrap();
        let accepted = resolve(Path::new("../outside-runtime"), &game).unwrap();
        assert!(accepted.is_absolute());
        assert_eq!(
            accepted,
            fs::canonicalize(game.parent().unwrap())
                .unwrap()
                .join("outside-runtime")
        );
        assert!(!accepted.exists());
        assert!(check_separate_from(&game, Path::new("child"), &game).is_err());
        assert!(check_separate_from(&game, Path::new("missing/../child"), &game).is_err());
        assert!(!game.join("child").exists());
        assert!(check_separate_from(&game, Path::new("../outside"), &game).is_ok());
        fs::remove_dir_all(game).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn paths_that_cannot_be_canonicalized_are_compared_lexically() {
        use std::os::unix::fs::PermissionsExt;
        let base = temp("uncanonical");
        let locked = base.join("locked");
        fs::create_dir_all(locked.join("game")).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let blocked = fs::canonicalize(locked.join("game"))
            .is_err_and(|e| e.kind() != io::ErrorKind::NotFound);
        if blocked {
            // Not running as root: canonicalize fails with "permission denied".
            let game = locked.join("game");
            assert_eq!(
                resolve(&game.join("x/../rt"), &base).unwrap(),
                game.join("rt")
            );
            assert!(check_separate_from(&game, &game.join("rt"), &base).is_err());
            assert!(check_separate_from(&game, &locked.join("rt"), &base).is_ok());
        }
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn aliased_existing_parent_is_resolved_before_containment_check() {
        let game = temp("alias-game");
        let alias = temp("alias-link");
        fs::create_dir_all(&game).unwrap();
        std::os::unix::fs::symlink(&game, &alias).unwrap();
        assert!(check_separate(&game, &alias.join("new-runtime")).is_err());
        assert!(!game.join("new-runtime").exists());
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(game).unwrap();
    }

    fn set_mtime(path: &Path, secs: u64) {
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }

    #[test]
    fn mirror_file_copies_only_what_is_newer() {
        let a = temp("mirror-a");
        let b = temp("mirror-b");
        fs::create_dir_all(&a).unwrap();
        fs::write(a.join("CHOICE.DAT"), b"one").unwrap();
        assert!(mirror_file(&a.join("CHOICE.DAT"), &b.join("CHOICE.DAT")).unwrap());
        assert!(!mirror_file(&a.join("CHOICE.DAT"), &b.join("CHOICE.DAT")).unwrap());
        assert!(!mirror_file(&a.join("missing"), &b.join("CHOICE.DAT")).unwrap());
        fs::write(a.join("CHOICE.DAT"), b"two!").unwrap();
        assert!(mirror_file(&a.join("CHOICE.DAT"), &b.join("CHOICE.DAT")).unwrap());
        assert_eq!(fs::read(b.join("CHOICE.DAT")).unwrap(), b"two!");
        touch_later(&b.join("CHOICE.DAT"));
        fs::write(a.join("CHOICE.DAT"), b"old").unwrap();
        assert!(!mirror_file(&a.join("CHOICE.DAT"), &b.join("CHOICE.DAT")).unwrap());
        assert_eq!(fs::read(b.join("CHOICE.DAT")).unwrap(), b"two!");
        for d in [a, b] {
            fs::remove_dir_all(d).unwrap();
        }
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
        let dst = rt.join("FALLBACK.OVL");
        bring_over(&ovl_link, &dst, "GLIDE2X.OVL", stamp(200_000, 1)).unwrap();
        assert!(fs::symlink_metadata(&dst).unwrap().is_file());
        assert_eq!(fs::read(&dst).unwrap().len(), 200_000);

        for d in [game, rt, outside] {
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
    fn save_state_round_trips() {
        let mut state = SaveState::new();
        let a = SaveStamp {
            size: 3,
            mtime_ns: 1_700_000_000_123_456_789,
        };
        state.insert(
            "REPLAYS/A B.RPL".into(),
            (
                a,
                SaveStamp {
                    size: 4,
                    mtime_ns: 9,
                },
            ),
        );
        assert_eq!(parse_save_state(&format_save_state(&state)), state);
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
