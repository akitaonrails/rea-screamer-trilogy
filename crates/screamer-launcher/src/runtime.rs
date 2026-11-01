//! Source game directory discovery, read-only runtime staging, binary patching,
//! save-game synchronization, and `dosbox-staging` invocation.

use std::fs;
use std::path::{Path, PathBuf};

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

/// Prepares `<runtime_dir>` by linking/copying game assets from `discovered.exe_dir`,
/// writing patched executables into `<runtime_dir>`, generating `SCREAMER.CFG`,
/// `CONFIG/SETUP.CFG`, `CONFIG/CONTROLS.CFG`, and `dosbox-screamer.conf`, and
/// syncing saves from `saves_dir`.
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
    stage_source_tree(&discovered.exe_dir, runtime_dir)?;
    if let Some(ovl_src) = &discovered.glide2x_ovl_source {
        let ovl_dst = runtime_dir.join("GLIDE2X.OVL");
        if !ovl_dst.exists() && ovl_dst.symlink_metadata().is_err() {
            link_or_copy(ovl_src, &ovl_dst)?;
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
        remove_if_exists(&dst)?;
        fs::write(&dst, &bytes).map_err(|e| format!("{}: {e}", dst.display()))?;
        patched_exes.push((exe.file_name.clone(), count));
    }

    // Write SCREAMER.CFG, CONFIG/SETUP.CFG, and CONFIG/CONTROLS.CFG.
    let screamer_cfg_path = runtime_dir.join("SCREAMER.CFG");
    remove_if_exists(&screamer_cfg_path)?;
    fs::write(&screamer_cfg_path, build_screamer_cfg(discovered.game))
        .map_err(|e| format!("{}: {e}", screamer_cfg_path.display()))?;

    let setup_cfg_path = config_dir.join("SETUP.CFG");
    remove_if_exists(&setup_cfg_path)?;
    fs::write(&setup_cfg_path, build_setup_cfg(discovered.game, 7))
        .map_err(|e| format!("{}: {e}", setup_cfg_path.display()))?;

    let controls_cfg_path = config_dir.join("CONTROLS.CFG");
    remove_if_exists(&controls_cfg_path)?;
    let keys = KeyBindings::default_for(discovered.game);
    fs::write(
        &controls_cfg_path,
        build_controls_cfg(discovered.game, keys),
    )
    .map_err(|e| format!("{}: {e}", controls_cfg_path.display()))?;

    // Sync user saves in if a saves directory is configured.
    if let Some(saves) = saves_dir {
        sync_saves_in(saves, runtime_dir)?;
    }

    let selected = discovered.select_executable(settings.renderer_choice())?;
    let res = settings.race_resolution().unwrap_or((1920, 1080));
    let widescreen = settings.widescreen.unwrap_or(false);
    let vsync = settings.vsync.unwrap_or(true);
    let voodoo_enabled = selected.edition.role == Renderer::Glide3dfx;
    let texture_filtering = settings.texture_filter == Some(TextureFilter::Smooth);

    let conf_text = build_dosbox_conf(&DosboxConfOptions {
        fullscreen: false,
        resolution: res,
        widescreen,
        vsync,
        voodoo_enabled,
        texture_filtering,
        c_drive_dir: runtime_dir,
        cd_image_path: discovered.cd_image.as_deref(),
        exe_name: &selected.file_name,
        exit_after_run: true,
    });
    let conf_path = runtime_dir.join("dosbox-screamer.conf");
    fs::write(&conf_path, conf_text).map_err(|e| format!("{}: {e}", conf_path.display()))?;

    Ok(PreparedRuntime {
        runtime_dir: runtime_dir.to_path_buf(),
        conf_path,
        selected_exe: selected.file_name.clone(),
        selected_edition: selected.edition,
        patched_exes,
    })
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() || path.symlink_metadata().is_ok() {
        fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn stage_source_tree(src_dir: &Path, dst_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(src_dir).map_err(|e| format!("{}: {e}", src_dir.display()))?;
    for entry in entries.flatten() {
        let src_path = entry.path();
        let Some(name) = src_path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let upper = name.to_ascii_uppercase();
        // Skip files we generate or patch ourselves.
        if upper == "SCREAMER.CFG" || KNOWN_EXE_NAMES.contains(&upper.as_str()) {
            continue;
        }
        if upper == "CONFIG" {
            if src_path.is_dir() {
                let dst_config = dst_dir.join("CONFIG");
                fs::create_dir_all(&dst_config)
                    .map_err(|e| format!("{}: {e}", dst_config.display()))?;
                if let Ok(cfg_entries) = fs::read_dir(&src_path) {
                    for cfg_entry in cfg_entries.flatten() {
                        let cp = cfg_entry.path();
                        if !cp.is_file() {
                            continue;
                        }
                        let Some(cn) = cp.file_name().and_then(|s| s.to_str()) else {
                            continue;
                        };
                        let cupper = cn.to_ascii_uppercase();
                        if cupper == "SETUP.CFG" || cupper == "CONTROLS.CFG" {
                            continue;
                        }
                        let target = dst_config.join(&cupper);
                        if target.exists() || target.symlink_metadata().is_ok() {
                            continue;
                        }
                        if is_save_file(&cupper) {
                            let _ = fs::copy(&cp, &target);
                        } else {
                            let _ = link_or_copy(&cp, &target);
                        }
                    }
                }
            }
            continue;
        }
        // If Screamer 2 has the 31 KB Windows stub glide2x.ovl, do not shadow DOSBox's built-in
        // Glide Voodoo driver unless it's a genuine DOS GLIDE2X.OVL (>100 KB).
        if upper == "GLIDE2X.OVL"
            && let Ok(meta) = fs::metadata(&src_path)
            && meta.len() < 50_000
        {
            continue;
        }
        if upper == "REPLAYS" {
            // Copy replay files into dst_dir/REPLAYS so new replays can be written.
            if src_path.is_dir() {
                let dst_replays = dst_dir.join("REPLAYS");
                fs::create_dir_all(&dst_replays)
                    .map_err(|e| format!("{}: {e}", dst_replays.display()))?;
                if let Ok(rep_entries) = fs::read_dir(&src_path) {
                    for rep in rep_entries.flatten() {
                        let rp = rep.path();
                        if rp.is_file()
                            && let Some(rn) = rp.file_name()
                        {
                            let target = dst_replays.join(rn);
                            if !target.exists() {
                                let _ = fs::copy(&rp, &target);
                            }
                        }
                    }
                }
            }
            continue;
        }

        let dst_path = dst_dir.join(&upper);
        if dst_path.exists() || dst_path.symlink_metadata().is_ok() {
            continue;
        }
        link_or_copy(&src_path, &dst_path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn link_or_copy(src: &Path, dst: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(src, dst).map_err(|e| format!("symlink {}: {e}", dst.display()))
}

#[cfg(not(unix))]
fn link_or_copy(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        for entry in fs::read_dir(src)
            .map_err(|e| format!("{}: {e}", src.display()))?
            .flatten()
        {
            let p = entry.path();
            if let Some(name) = p.file_name() {
                link_or_copy(&p, &dst.join(name))?;
            }
        }
        Ok(())
    } else {
        fs::copy(src, dst)
            .map(|_| ())
            .map_err(|e| format!("copy {}: {e}", dst.display()))
    }
}

fn is_save_file(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.ends_with(".SAV") || upper == "CHOICE.DAT"
}

/// Copies persistent saves (`*.SAV`, `CHOICE.DAT`, `REPLAYS/*.RPL`) from `saves_dir`
/// into `runtime_dir`.
pub fn sync_saves_in(saves_dir: &Path, runtime_dir: &Path) -> Result<(), String> {
    if !saves_dir.exists() {
        return Ok(());
    }
    copy_matching_files(saves_dir, runtime_dir, is_save_file)?;
    let saves_cfg = saves_dir.join("CONFIG");
    if saves_cfg.is_dir() {
        copy_matching_files(&saves_cfg, &runtime_dir.join("CONFIG"), is_save_file)?;
    }
    let saves_rep = saves_dir.join("REPLAYS");
    if saves_rep.is_dir() {
        copy_matching_files(&saves_rep, &runtime_dir.join("REPLAYS"), |n| {
            n.to_ascii_uppercase().ends_with(".RPL")
        })?;
    }
    Ok(())
}

/// Copies persistent saves (`*.SAV`, `CHOICE.DAT`, `REPLAYS/*.RPL`) from `runtime_dir`
/// back to `saves_dir`.
pub fn sync_saves_out(runtime_dir: &Path, saves_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(saves_dir).map_err(|e| format!("{}: {e}", saves_dir.display()))?;
    copy_matching_files(runtime_dir, saves_dir, is_save_file)?;
    let rt_cfg = runtime_dir.join("CONFIG");
    if rt_cfg.is_dir() {
        let saves_cfg = saves_dir.join("CONFIG");
        fs::create_dir_all(&saves_cfg).map_err(|e| format!("{}: {e}", saves_cfg.display()))?;
        copy_matching_files(&rt_cfg, &saves_cfg, is_save_file)?;
    }
    let rt_rep = runtime_dir.join("REPLAYS");
    if rt_rep.is_dir() {
        let saves_rep = saves_dir.join("REPLAYS");
        fs::create_dir_all(&saves_rep).map_err(|e| format!("{}: {e}", saves_rep.display()))?;
        copy_matching_files(&rt_rep, &saves_rep, |n| {
            n.to_ascii_uppercase().ends_with(".RPL")
        })?;
    }
    Ok(())
}

fn copy_matching_files<F: Fn(&str) -> bool>(
    src_dir: &Path,
    dst_dir: &Path,
    pred: F,
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(src_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if pred(name) {
            let dst = dst_dir.join(name);
            remove_if_exists(&dst)?;
            fs::copy(&p, &dst).map_err(|e| format!("copy {}: {e}", dst.display()))?;
        }
    }
    Ok(())
}

/// Locates the `dosbox-staging` (or `dosbox`) executable to run.
pub fn find_dosbox(explicit: Option<&Path>) -> Option<PathBuf> {
    find_dosbox_with_hint(explicit, None)
}

/// Locates the `dosbox-staging` (or `dosbox`) executable, checking `explicit`,
/// `SCREAMER_DOSBOX`, relative `dosbox-staging/` folders next to `game_dir`,
/// the launcher's own directory, and `PATH`.
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
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for cand in [
            "dosbox-staging",
            "dosbox",
            "dosbox.exe",
            "dosbox-staging/dosbox",
            "dosbox-staging/dosbox.exe",
        ] {
            let p = dir.join(cand);
            if p.is_file() {
                return Some(p);
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
