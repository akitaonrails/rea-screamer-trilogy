# Contributing findings back

Drafts of what this launcher's own reverse engineering found that players
of the Screamer trilogy could use, for PCGamingWiki first (the SDK's
`docs/playbooks/contribute-back.md` and `docs/STANDARDS.md`, "Contribute
back"). Copied from the SDK's `docs/CONTRIBUTIONS.md` (S1–S9, prepared
2026-11-02), with the repository inconsistencies it found now fixed here.

**The owner reviews and posts; agents only draft.** Nothing here has been
submitted anywhere. A draft is posted only after the owner approves it by
number ("post S3"); then record the link and date in its status. PCGW
wikitext is pasted by hand. Before posting, the owner checks the live page
again, fills in `{{Refcheck|user=…}}`, checks each hex edit on a copy of
the exact executable named (same SHA-256), and never attaches game files,
patched executables or saves.

Only findings verified in the real game (`docs/RUNTIME_NOTES.md`) are
marked **ready**; the rest are under "After verification".

## Offsets: file offset, not VA

PCGW hex fixes give the file offset typed into a hex editor's "Go to".
`fixes.rs` uses LE virtual addresses: these are Watcom/DOS4GW LE
executables, so VA → file offset depends on the object and page tables
(`le.rs::va_to_file_offset`). Every file offset below was computed from the
LE page map, and the original bytes were read back from the real GOG/ISO
executables.

## Summary

| # | Finding | Ours or community | Target | Status |
|---|---|---|---|---|
| S1 | Screamer (1995): Hor+ 16:9 widescreen hex edit (camera matrix X-row × 0.75), `STARTH.EXE`/`STARTL.EXE` (GOG) | ours | PCGW *Screamer*, Video → Widescreen | **ready**, once the owner confirms the 1080p gameplay recording (Enhanced preset) shows Hor+ and not a stretch |
| S2 | Screamer 2 / Screamer Rally: the same Hor+ edit (6 executables) | ours | PCGW *Screamer 2*, *Screamer Rally* | needs verification (no in-race visual check on record) |
| S3 | Screamer (1995) runs a fixed 50 Hz game loop (2 ticks of a 100 Hz timer); physics comes from a 50-entry table, so unlocking breaks it | ours | PCGW *Screamer*, the 60/120 FPS rows (the wiki says 60 FPS true, "70 FPS cap") | **ready** as a correction note; owner to confirm in-game with an FPS counter |
| S4 | Screamer 2/Rally Glide mode in DOSBox Staging exits with `MAXCACHE` when `voodoo_memsize` is above 4 | ours | PCGW *Screamer 2*, *Screamer Rally*, Issues fixed | **ready** (exit reproduced, fix is a config value) |
| S5 | Screamer 2 (GOG): the bundled `glide2x.ovl` is a 31 KB Windows/nGlide stub; Glide mode in plain DOSBox needs Screamer Rally's 348 KB DOS `GLIDE2X.OVL` | ours | PCGW *Screamer 2*, API/Glide note | **ready** (3dfx mode entered with the genuine OVL); an in-race session is advisable |
| S6 | Screamer 2/Rally: `grTexDownloadMipMap: mipmap cannot span 2 Mbyte boundary` exit (F7 hex fix) | ours | PCGW Issues fixed | needs verification (the bug is reproduced; the fixed exe was not shown surviving the same track-preview sequence) |
| S7 | Remove the 10% joystick deadzone (E5) | ours | PCGW Input | needs verification (the launcher's own pad injection bypasses this code, so E5 was never tested with DOSBox's gameport joystick) |
| S8 | Extended draw distance (E6) | ours | PCGW / mod note | needs verification (headless race segment only, no visual check) |
| S9 | SVGA/VESA/SETUP fixes F2, F3, F5, F6 (DPMI probe, WinGranularity, VBE mode 0x120, SETUP divide-by-zero at high cycles) | ours | PCGW Issues fixed (F3 may explain the known SVGA track-5 crash) | needs verification (no before/after run recorded; the launcher never runs SETUP) |
| — | Screamer "60/120 FPS via smoothed_dt clamp + SOS timer" | — | — | **do not publish**: the code ships E1 as a no-op because every such unlock broke physics (see S3). The 120/240 Hz timer idea in `docs/04-…` was never tested |

---

## Ready drafts: PCGamingWiki

### S1 — Screamer (1995): Hor+ widescreen

Page: <https://www.pcgamingwiki.com/wiki/Screamer>. Change the Video
template rows and add a `===Widescreen resolution===` section.

```wikitext
|widescreen resolution      = hackable
|widescreen resolution notes= 3D view only; menus and HUD stay 4:3 and are stretched. See [[#Widescreen resolution|Widescreen resolution]].
```

```wikitext
===[[Glossary:Widescreen resolution|Widescreen resolution]]===
{{Fixbox|description=Hor+ 16:9 hex edit|ref=<ref>{{Refcheck|user=OWNER|date=2026-11-XX|comment=GOG release, DOSBox Staging 0.82.2 with aspect = stretch.}}</ref><ref>{{Refurl|url=https://github.com/akitaonrails/rea-screamer-trilogy/blob/master/crates/screamer-launcher/src/fixes.rs|title=rea-screamer-trilogy – fixes.rs (E2)|date=2026-11-02}}</ref>|fix=
{{ii}} Narrows the camera's horizontal axis to 3/4, so a 16:9 display shows the original 4:3 view in the centre plus more to the sides (horizontal FOV about 90° → 106°). Menus and HUD are 2D and get stretched.
{{ii}} Only for the executables below (GOG release); check the SHA-256 first. Back up the file.
# Open the executable with a hex editor such as [https://mh-nexus.de/en/downloads.php?product=HxD HxD].
# Press {{key|Ctrl|G}}, go to the first offset and replace the 31 bytes:
#* Original: <code>0F BF C9 0F BF F6 0F BF DB 0F BF FF 0F BF ED 0F BF C0 D1 F9 D1 FE D1 FB D1 FF D1 FD D1 F8 52</code>
#* New: <code>8B 06 C1 F8 02 29 06 8B 46 0C C1 F8 02 29 46 0C 8B 46 18 C1 F8 02 29 46 18 2B C0 89 46 24 C3</code>
# Go to the second offset and replace the 5 bytes <code>2B C0 89 46 24</code> with <code>E8 5B 0A 00 00</code>.
#* {{file|STARTH.EXE}} (SVGA, 1,034,923 bytes, SHA-256 <code>1246afc95463f96598d5fee764fc99c847b6d3e0bf8ca2268bd362c664bb28a3</code>): offsets '''095F22''' and '''0954C2'''.
#* {{file|STARTL.EXE}} (VGA, 1,034,919 bytes, SHA-256 <code>ad20fa3919ecf86da5931e2e0e495d4043f6be8c44e57299114528d87ee27ec7</code>): offsets '''095C72''' and '''095212'''.
# Make DOSBox stretch the picture to 16:9: in DOSBox Staging set <code>aspect = stretch</code> in <code>[render]</code> and a 16:9 <code>windowresolution</code>/fullscreen.
}}
```

Notes for the owner:
- The first edit writes a small routine into an unreferenced duplicate
  camera function (`SetCameraMatrix_Unused`). The second turns
  `sub eax,eax; mov [esi+24h],eax` into a `call` to it. The routine
  does `x -= x>>2` on matrix entries `[esi+0]`, `[esi+0Ch]`, `[esi+18h]`
  and then repeats the two displaced instructions. The `call` rel32 is
  relative and the patch range has no LE fixup records, so no relocation
  edit is needed (the launcher checks this before patching).
- Only DOSBox Staging 0.82.2 with `aspect = stretch` was used. The GOG
  bundle's DOSBox (0.74 family) presumably needs `aspect=false`, a 16:9
  `fullresolution` and `output=opengl`, but that is **untested**: say so or
  leave it out.
- Status: I found no explicit in-race check of E2 in `RUNTIME_NOTES.md`.
  The evidence is the README's 1080p gameplay video recorded with the
  Enhanced preset (widescreen defaults to on). The owner should confirm the
  video shows Hor+ (cars not stretched) before posting.

### S3 — Screamer (1995): the frame-rate rows

The wiki currently says `60 fps = true`, `120 fps = false`, "70 FPS cap".
Static analysis says the race loop waits for 2 ticks of the HMI SOS timer
at 100 Hz (`STARTH.EXE` file offset 02A446: `83 FA 02 72 F5`, `cmp edx,2; jb`),
which is a 50 FPS ceiling. Physics then indexes a 50-entry table by
`int(1/dt)`, with `dt` clamped to ≥ 0.02 s. The repo's runtime tests showed
that lowering either limit breaks the game: the game runs 1.4–2× too fast,
or the track crashes on load.

**Before posting, verify with an FPS counter in a race** (DOSBox Staging
shows FPS in the title bar), because this contradicts the wiki's 60/70 FPS
claim. If confirmed:

```wikitext
|60 fps                     = false
|60 fps notes               = Races run at a fixed 50 FPS (two ticks of a 100 Hz timer); physics uses a 50-entry per-frame-rate table, so the cap cannot be raised without breaking the game.<ref>{{Refcheck|user=OWNER|date=2026-11-XX|comment=Measured in DOSBox Staging; analysis in https://github.com/akitaonrails/rea-screamer-trilogy/blob/master/docs/04-screamer-engine-and-patches.md}}</ref>
|120 fps                    = false
|120 fps notes              =
```

Screamer 2 is already "50 FPS cap" on PCGW, which agrees with our analysis.
Screamer Rally says "60 FPS cap". In our analysis Rally's spin-wait is
already 1 tick, while the `dt ≥ 0.02 s` physics floor remains. What that
means for its real frame rate is **not established**, so do not edit the
Rally row.

### S4 — Screamer 2 / Screamer Rally: `MAXCACHE` exit in Glide mode

Pages: *Screamer 2*, *Screamer Rally*. Add under Issues fixed:

```wikitext
===Game exits with "MAXCACHE" in 3dfx mode===
{{ii}} With emulated Voodoo memory above 4 MB, the 3dfx executable ({{file|S2_3DFX.EXE}} / {{file|STRT3FX.EXE}}) uploads more than 1,024 textures before its texture cache wraps, prints <code>MAXCACHE</code> and quits.
{{Fixbox|description=Emulate a 4 MB Voodoo 1|ref=<ref>{{Refcheck|user=OWNER|date=2026-11-XX|comment=DOSBox Staging 0.82.2; voodoo_memsize = 12 reproduces the exit, 4 avoids it. Analysis: https://github.com/akitaonrails/rea-screamer-trilogy/blob/master/docs/RUNTIME_NOTES.md}}</ref>|fix=
# In the DOSBox Staging configuration, set <code>voodoo_memsize = 4</code> in the <code>[voodoo]</code> section (2 MB frame buffer + 2 MB texture memory, as on a real Voodoo 1).
}}
```

Note: this applies to DOSBox builds with built-in Voodoo emulation (Staging
0.81+). It does not apply to the GOG/Steam bundles, which use nGlide.

### S5 — Screamer 2 (GOG): Glide in plain DOSBox

Page: *Screamer 2*, Other information → API, as a note on the Glide row:

```wikitext
{{ii}} The GOG/Steam {{file|glide2x.ovl}} (31,425 bytes) is a stub for the bundled nGlide setup and does not work with DOSBox's own Voodoo emulation. Copying the 348,791-byte DOS {{file|GLIDE2X.OVL}} from a [[Screamer Rally]] install next to {{file|S2_3DFX.EXE}} lets the 3dfx renderer run in DOSBox Staging.<ref>{{Refcheck|user=OWNER|date=2026-11-XX}}</ref>
```

Hashes measured on this machine (now also in `edition.rs`): stub `fe6f33d3add608eee61f956b82eaa9c9027ee79441dde29b3fbc687f0605f37b`;
Screamer Rally ISO `GLIDE2X.OVL` `6981a7bd77445be7b360ca367af1e527927db396546db15b69803b16a67676a9`.
Evidence: the headless run entered "3dfx Voodoo 640x480 16-bit" with
`S2_3DFX.EXE` (`RUNTIME_NOTES.md` §5). An in-race session before posting is
advisable. Whether to suggest copying a file between two games is the
owner's call: both are owned copies, and nothing is downloaded.

---

## After verification

### S2 — Hor+ for Screamer 2 / Screamer Rally

These use the same 31-byte routine and original bytes as S1, plus a 5-byte
`call`:

| Executable (SHA-256 prefix) | routine offset | call offset | call bytes |
|---|---|---|---|
| S2 `S2_3DFX.EXE` (`e6c24c84…`) | 08A41E | 0899B2 | `E8 67 0A 00 00` |
| S2 `START65H.EXE` (`05f57686…`) | 09334E | 0928F2 | `E8 57 0A 00 00` |
| S2 `STARTH.EXE` (`91a49f26…`) | 09270E | 091CB2 | `E8 57 0A 00 00` |
| SR `STRT3FX.EXE` (`9eac6e12…`) | 087FEE | 087582 | `E8 67 0A 00 00` |
| SR `START65H.EXE` (`0785e385…`) | 08564E | 084BF2 | `E8 57 0A 00 00` |
| SR `STARTH.EXE` (`18d32c27…`) | 08A4FE | 089AA2 | `E8 57 0A 00 00` |

Needed: an in-race visual check in each game. One reason for caution:
`docs/04-…` §3.2 describes Screamer 2/Rally's camera matrix as
**floating-point**, while the patch applies the same integer `sar 2` routine
as Screamer 1. The patched call site does look identical to Screamer 1's
(`lea esi,[edx+48h]` … `sub eax,eax; mov [esi+24h],eax`), so it may be an
integer copy, but if the matrix there is float the result would be wrong.
Do not publish until it is seen working.

### S6 — F7, 3dfx texture upload at the 2 MB boundary

The bug was reproduced with the original exe: switching track previews
exits with `grTexDownloadMipMap: mipmap cannot span 2 Mbyte boundary`. Fix:

| Executable | offset | original → new |
|---|---|---|
| `S2_3DFX.EXE` | 0D3457 | `8E` → `8C` (`jle` → `jl`) |
| | 0D3658 | `7D 10` → `90 90` |
| | 0D3826 | `7D 10` → `90 90` |
| `STRT3FX.EXE` | 0D0FF7 | `8E` → `8C` |
| | 0D11F8 | `7D 10` → `90 90` |
| | 0D13C6 | `7D 10` → `90 90` |

Needed: repeat the same track-preview sequence with the patched exe and
play a race. The second and third bytes are the 2nd byte of a `0F 8E`/`jge`;
the offsets point exactly at the changed bytes.

### S7 — E5 deadzone (10% centre deadzone, `cmp eax,66h`)

Change `66` → `00` at: S1 `STARTH` 02F73E, S1 `STARTL` 02F6DE, S2 `S2_3DFX`
03FE04, S2 `START65H` 0473B3, S2 `STARTH` 0449DB, SR `STRT3FX` 03F644,
SR `START65H` 03D2B4, SR `STARTH` 03BE84.

`docs/04-…` §5.1 gives 02F73C/03FE02/03F642 because it points at the
start of the `83 F8 66` instruction, two bytes earlier. Needed: a test
with DOSBox's own joystick emulation. The launcher's injection sets
`record.enabled = 2`, which skips this code path entirely, so E5 has never
had an observable effect in our runs.

### S8 — E6 draw distance

- S2/SR: five `cmp eax, 14257880h` (`3D 80 78 25 14` → `3D 00 E2 95 50`) per exe.
- S1: float 40000 → 48000 plus two 52000 → 62400 gates. `STARTH`: 0F7529
  `00 40 1C 47` → `00 80 3B 47`; 02B1BB and 02B1D2 `20 CB 00 00` →
  `C0 F3 00 00`.
- `S2_3DFX`: 081D0F, 081D73, 081DF9, 081E5D, 089652.
- `STRT3FX`: 07F8DF, 07F943, 07F9C9, 07FA2D, 087222.

The other renderers' offsets can be computed the same way from `fixes.rs`.
Needed: a visual before/after in a race. Only a headless Rally race
segment ran.

### S9 — DOSBox compatibility fixes

These are known community pain points: the Screamer 1 SVGA crash (PCGW's
"VGA vs SVGA", track 5) and SETUP crashing at high cycles.

- F3 `STARTH` (S1) 09665F `83 E0 07` → `90 90 90`: WinGranularity mask. It
  is a plausible cause of SVGA bank-switch faults, **but no link to the
  track-5 crash has been shown**.
- F2 `STARTH` (S1) 03A0CA `53 51 52 56 57 55` → `B8 00 00 00 01 C3`: DPMI
  probe returns 16 MB.
- F5 S2 `START65H` 0EC02F `7C` → `EB`: force VESA 0x111 (`docs/03-…` said
  0EB02F before the fix below).
- F6 `SETUP.EXE` S1 03384B/0338F2, S2 038553/038606, SR `ST.EXE`
  03B2A3/03B356: `F7 F9` → `90 90`. This NOPs `idiv ecx`, so the benchmark
  result is meaningless; check SETUP's resulting choices.

Needed for each: a before/after run on the real game under the stated
DOSBox settings. For F3, a run of track 5 in SVGA with and without it.

## Repository inconsistencies (fixed 2026-11-02)

The SDK's drafting pass found these; all are fixed in this repository:

- The Glide OVL hashes disagreed three ways. `edition.rs` and
  `docs/RUNTIME_NOTES.md` §4 now carry the measured ones (stub
  `fe6f33d3…`, DOS driver `6981a7bd…`). Behaviour never depended on them:
  the launcher tells the files apart by size.
- `docs/03-…` §4.2 gave F5's file offset as `0x0EB02F`; it is `0x0EC02F`
  (range `0x0EC025..`), and the two other `START65H.EXE` offsets in the
  same section had the same 0x1000 error.
- `docs/04-…` §2 named a different function for F2 than the one `fixes.rs`
  patches (VA `0x02F8CA`, file `0x03A0CA`); it now describes the shipped fix.
- `docs/04-…` §5.1 pointed at the start of the `cmp eax,66h` instructions;
  it now also gives the offset of the `66` byte E5 changes.
- The 30/50 FPS choice suggested a frame-rate unlock that does not exist
  (E1 is empty): the window now has one frame-generation switch.
