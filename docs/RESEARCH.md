# Community work and research

What the community already made for the Screamer trilogy, and what this
launcher does with it (`docs/STANDARDS.md` in the SDK, "Research before
building"). Observations from our own reverse engineering are in
`docs/03-…`, `docs/04-…` and `docs/RUNTIME_NOTES.md`; drafts to share
back are in `docs/CONTRIBUTIONS.md`.

## Components

| Project | What it does for these games | Licence | Maintained | Decision |
|---|---|---|---|---|
| [dosbox-staging](https://github.com/dosbox-staging/dosbox-staging) v0.82.2 | Runs the DOS games, with built-in 3dfx Voodoo 1 emulation (`[voodoo]`), OpenGL shaders, CD images with audio tracks | GPL-2.0-or-later | yes | **Bundled**, built in CI from the pinned submodule with our patch (`patches/`): the `screamer-hooks` gamepad/rumble hooks and the 2× Voodoo scale and FSR/frame-generation shader. Corresponding source is attached to every release |
| GOG's releases (DOSBox 0.74 bundle, nGlide, cue-sheet `.DAT` images) | Their layouts are what the launcher discovers and mounts (`SCREAM.DAT` → `SCREAM.gog`, `GAME.DAT` → `GAME.GOG`) | proprietary (the player's copy) | — | **Read, never changed**; the CD image is mounted read-only from the game folder |
| nGlide / dgVoodoo2 (Glide wrappers) | Glide for the Windows-hosted DOS box in GOG's bundle | freeware / own licences | yes | **Not used**: DOS/4GW loads `GLIDE2X.OVL` inside emulated DOS memory and talks to the emulated Voodoo over PCI; a host-side wrapper never sees those calls (`RUNTIME_NOTES.md` §10–11) |
| Screamer Rally's DOS `GLIDE2X.OVL` (348,791 bytes) | Lets Screamer 2's 3dfx build run in DOSBox's own Voodoo emulation (GOG's Screamer 2 ships a 31 KB nGlide stub) | the player's copy | — | **Configured**: staged from a sibling Screamer Rally install or `SCREAMER_GLIDE2X_OVL`, never copied into the repository |
| PCGamingWiki pages *Screamer*, *Screamer 2*, *Screamer Rally* | Frame-rate rows (Screamer 2 "50 FPS cap"; Screamer listed with 60 FPS and a "70 FPS cap"), the known SVGA track-5 crash in Screamer 1 | CC BY-NC-SA | — | **Cross-checked**; our corrections and fixes the wiki lacks are drafted in `docs/CONTRIBUTIONS.md` (S1–S9) |

No community widescreen, frame-rate, deadzone, draw-distance or texture
cache fix for these games was found, so the launcher's own `E2`, `E5`,
`E6` and `F1`–`F7` fixes (`fixes.rs`) fill those gaps; each is keyed to
exact executables by SHA-256 (`edition.rs`).

## How other packagers mount the CD

GOG's own DOSBox configurations mount the cue sheet, not the raw image:
`imgmount d "..\scream.dat" -t iso -fs iso` (Screamer) and
`imgmount d .\GAME.DAT -t iso` (Screamer 2), so the audio tracks come
with it. The launcher does the same. Other copies keep the image in a
folder of their own (`CD`, `ISO`, a rip's folder name), as CloneCD
`.img`/`.ccd`/`.sub` without a cue sheet, or as a `.cue` + `.bin` pair;
DOSBox Staging's `IMGMOUNT` reads a cue sheet, an `.iso`, or a raw
2352-byte `.bin`/`.img` as a data-only disc (`LoadIsoFile` probes 2048,
2352 and 2336-byte sectors). A Quiver player (2026-11-09, Windows, 0.8.2)
had Screamer and Screamer Rally stop at "PLEASE INSERT … DISK" with the
image in the game folder: 0.8.2 searched only a few fixed folders and
started DOSBox with an empty CD drive otherwise. Reproduced with the
0.8.4 Windows build under Wine and the owner's GOG/ISO files moved into
an unsearched subfolder (`RUNTIME_NOTES.md` §17); fixed in the next
release by searching every folder directly inside the game folder.

## Dead ends (recorded so nobody ships them)

- **Frame-rate unlocks.** Every change to the 2-tick wait of the 100 Hz
  SOS timer or the `dt ≥ 0.02 s` clamp broke the physics: 1.4–2× speed,
  out-of-table reads, cars jumping on the grid (`docs/04-…`,
  `RUNTIME_NOTES.md` §7). `E1` is kept as an empty fix to say so. Frame
  generation (100 FPS presented, game at 50 Hz) is the substitute.
- **Voodoo memory above 4 MB** exits with `MAXCACHE` in Screamer 2/Rally
  (`RUNTIME_NOTES.md` §9): the launcher always emulates a 4 MB Voodoo 1.
- **The Screamer 1 "3DFX Voodoo 3 Patch (2026)"** ([TgamesFR,
  Vogons](https://www.vogons.org/viewtopic.php?t=112964), October 2026)
  replaces `STARTH.EXE`/`STARTL.EXE` with closed-source Glide 2.x
  renderers: Screamer 1 never shipped a 3dfx build, so this is a
  community patch, not an edition to key. It targets a real Voodoo 3
  (the author: Voodoo 3/4/5 yes, Voodoo 1 no), while DOSBox Staging
  emulates a Voodoo 1 only — `voodoo.cpp` builds either `VOODOO_1` or
  `VOODOO_1_DTMU` and nothing newer — and its binaries sit behind a
  members-only download, so we cannot hash or test them. Revisit if a
  Voodoo 1-compatible version runs under DOSBox Staging (issue #1).
  A SHA-256 alone is not enough to key such a build: `STARTH.EXE` needs
  the required `F2` fix (DPMI memory probe), whose offset and expected
  bytes (`fixes.rs`) must be read from the file itself, and a Glide
  renderer would also need the Voodoo path set up for Screamer 1, which
  today runs only its SVGA/VGA programs. On 2026-11-08 the reporter
  posted one `STARTH.EXE` SHA-256
  (`f5b074560c3adba70f891430717c12dea9c389d593d2aa62386ffc01e97bb380`,
  not one of ours) and "it works with dosbox", without saying whether
  it is the CD original or the patched file, or which DOSBox; the file
  was offered only through a reporter-hosted download, which we do not
  take. Not keyed.

## Old-game trouble spots (SDK checklist)

| Spot | Applies? | How it is handled |
|---|---|---|
| Timers and frame pacing on fast CPUs | yes (DOS timer loop) | DOSBox emulates the PIT/SOS timer; `cpu_cycles = max` is safe because the game paces on the timer, not the CPU |
| Multi-core affinity | no | DOSBox runs the game on its own emulated CPU |
| DirectSound3D/EAX | no | Sound Blaster 16 + OPL3 emulated (`SETUP.CFG` generated to match) |
| CD audio | yes | images with audio tracks mounted by DOSBox |
| Intro-video codecs | no | the games' own DOS players |
| Resolution and aspect tables | yes | fixed 640×480/320×200 renderers; Hor+ via `E2`, the window size and shaders by DOSBox |
| Focus loss | not checked | to verify (`RUNTIME_NOTES.md` §13) |
