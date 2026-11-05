//! The Windows executable's version information, icon and manifest
//! (rea-launcher-build): a named, versioned program with the launcher's
//! icon, instead of an anonymous one that antivirus heuristics distrust.
fn main() {
    rea_launcher_build::Resources::new(
        "Screamer Trilogy Launcher",
        "Launcher for Screamer, Screamer 2 and Screamer Rally on modern systems",
    )
    .company("AkitaOnRails")
    .file_name("ScreamerLauncher.exe")
    .icon("../../packaging/appimage/screamer-launcher.png")
    .embed()
    .unwrap();
}
