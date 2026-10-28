# NusaSetup.exe / Nusa 1.0.0 — REA analysis report

Date: 2026-10-27
Analyst tooling: REA 6.5.0 (CLI), Ghidra 12.1 provider, innoextract 1.12-dev
(Inno 6.7 support), custom Inno 6.7.0 table parser (`parse_inno.py`)
Method: static only. Neither the installer nor the payload was executed.

## 1. Artifacts

| Item | Value |
|---|---|
| File | `E:\NusaSetup\NusaSetup.exe` |
| Size | 2,809,698 bytes |
| SHA-256 | `63170651afbabd0aefce1a011b01a8fb28c887790e3463da5952e2236c533c99` |
| Format | PE32, machine x86 (0x014C), GUI subsystem, 11 sections |
| Signature | **Not signed** (Authenticode absent) |
| Product | Nusa, version 1.0.0.0 |
| Company / author | Rama |
| FileDescription | "Nusa Setup" |
| Copyright | © 2026 Rama |
| Builder | Inno Setup 6.7.0 (Unicode setup data), Delphi 36.0 compiler |

Resources (33): 15 `RT_ICON` + `RT_GROUP_ICON` (16→256 px), 13 `RT_STRING` tables,
`RT_VERSION`, `RT_MANIFEST`, `DVCLAL`, `PACKAGEINFO` (Delphi VCL markers), plus
`RCDATA (10, 11111, 1033)` = the Inno loader offset table.

Embedded manifest: `requestedExecutionLevel level="asInvoker"`, `dpiAware=true`,
supportedOS Win7 → Win11, fixed DLL loading from `%SystemRoot%\system32`
(`comctl32`, `mpr`, `netapi32`, `netutils`, `textshaping`, `version`, `winhttp`).

## 2. Installer container

| Region | File offset | Stored | Inflated | Content |
|---|---|---|---|---|
| File-data chunk for `nusa.exe` | `0xF1200` | 461,502 | 909,312 | `zlb\x1a` + LZMA2 (32 MiB dict) + Inno exe filter |
| Primary setup block | `0x161D33` | 35,841 | 194,445 | all setup tables |
| Secondary setup block | `0x16A941` | 77 | 89 | 1 file-location entry |
| Embedded Setup engine | `0x16A99B` | 1,324,474 | 4,489,728 | MZ image |

Loader offset table (revision 2) at `0xF02C0` in RCDATA `(10, 11111, 1033)`:
`TotalSize=0x2ADF62` (= file size), `OffsetEXE=0x16A99B`,
`UncompressedSizeEXE=0x448200`, `Offset0/header=0x161CBE`, `Offset1/data=0xF1200`.

- Setup-data ID `Inno Setup Setup Data (6.7.0)` at `0x161CBE`.
- `TSetupEncryptionHeader` at `0x161CFE`: `EncryptionUse=0`, CRC valid, KDF salt /
  password test / base nonce all zero → **no password, no setup-data encryption**.
- The primary, secondary and embedded-engine block headers all pass their CRC-32
  checks. The primary block inflates
  `35,800 → 194,445` bytes (LZMA1, props `0x5D`: lc=3, lp=0, pb=2, 2 MiB dict), and the
  table walk consumes exactly 194,445 bytes (0 trailing bytes) — the parse is
  self-validating.
- The loader's `CRCExe` field (`0x8ADD113F`) does not equal a plain zlib CRC-32 of the
  stored block, its payload, or the inflated engine image (which hashes to `0x7FFBF87F`).
  It is presumably Inno's internal checksum over the extracted `Setup.e32`; it is not
  needed to decode the data tables and was not investigated further (**unknown**).
- The single file-location entry records `OriginalSize=909,312`,
  `ChunkCompressedSize=461,498` and SHA-256
  `2dfa517fe20fb944cd263597e9f731b8245fc2b5253d9f1a241221636e8d1bdb`, exactly the
  extracted `nusa.exe` (bit-identical).
- Its five flag bits decode as `VersionInfoValid=0, TimeStampInUTC=0,
  CallInstructionOptimized=1, ChunkEncrypted=0, ChunkCompressed=1`
  (`TSetupFileLocationEntry.Flags`, issrc 6.7.0). The chunk therefore uses the header's
  compression method and Inno's x86 "call instruction" exe filter.
- Chunk chain: magic `zlb\x1a` at `0xF1200`, LZMA2 property `0x1A` (32 MiB dictionary)
  at `0xF1204`, stream at `0xF1205` (461,497 bytes). The chunk ends exactly at the
  setup-data ID (`0x161CBE`), so it occupies 461,502 bytes on disk; the location entry's
  `ChunkCompressedSize=461,498` counts the property byte plus stream and excludes the
  4-byte magic. Plain LZMA2 yields 909,312 bytes
  with SHA-256 `59a1515282c66ec182cfa61399f56d9118ea08442c87e2e5f46d370f67c71b10` —
  the *filtered* image. It differs from the real PE at 24,465 bytes, and all of those
  bytes lie inside `E8`/`E9` (CALL/JMP) displacement fields across 10,877 sites.
  Undoing the filter (three low address bytes rewritten relative to the instruction
  position, high byte toggled when bit 23 is set, instructions crossing a 64 KiB
  boundary skipped) reproduces `2dfa517f…8d1bdb`. Verified by
  `re-analysis\verify_exefilter.py`.

`innoextract --list` independently reports one file: `app\nusa.exe` (888 KiB).

## 3. Recovered setup tables

The `[Setup]`, `[Tasks]`, `[Icons]`, `[Registry]`, `[Run]`, `[Files]` and compiled
`[Code]` tables were decompressed and parsed directly from the primary block
(`0x161D33` → 194,445 bytes). All values below are **observed** unless marked
otherwise.

Entry counts: language 1, custom messages 15, permissions 0, types 0, components 0,
tasks 2, directories 0, ISSig keys 0, files 2, file locations 1, icons 2, INI 0,
registry 1, install-delete 0, uninstall-delete 0, run 1, uninstall-run 0.

### 3.1 `[Setup]`

| Directive | Value |
|---|---|
| `AppName` | `Nusa` |
| `AppVersion` | `1.0.0.0` |
| `AppVerName` | `Nusa 1.0.0.0` |
| `AppId` | `{A03D4881-45E3-4B16-B844-936040C558E5}` |
| `AppPublisher` | `Rama` |
| `AppPublisherURL` / `AppSupportURL` / `AppUpdatesURL` | `https://rama2555.github.io/nusa/` |
| `AppCopyright` | `Copyright (C) 2026 Rama` |
| `DefaultDirName` | `{autopf}\Nusa` |
| `DefaultGroupName` | `Nusa` |
| `UninstallFilesDir` | `{app}` |
| `UninstallDisplayName` | `Nusa` |
| `UninstallDisplayIcon` | `{app}\nusa.exe` |
| `AppMutex` | `NusaNotch.SingleInstance` |
| `CreateUninstallRegKey` / `Uninstallable` | `yes` / `yes` |
| `ArchitecturesAllowed` | `x64compatible` |
| `ArchitecturesInstallIn64BitMode` | `x64compatible` |
| `MinVersion` | NT 10.0.17763 (WinVersion unset) |
| `OnlyBelowVersion` | none |
| `PrivilegesRequired` | `lowest` (`prLowest`, stored value 3) |
| `PrivilegesRequiredOverridesAllowed` | none (empty set) |
| `Compression` | `lzma2` (`cmLZMA2`, stored value 4; file chunks only) |
| `DisableDirPage` | `yes` |
| `DisableProgramGroupPage` | `yes` |
| `DisableWelcomePage` | `yes` |
| `DisableStartupPrompt` | `yes` |
| `CloseApplications` / `RestartApplications` | `yes` / `yes` |
| `CloseApplicationsFilter` | `*.exe,*.dll,*.chm` |
| `WizardResizePercent` | 120 × 120, modern style |
| `SetupLogging` | not enabled by default |
| `LicenseFile` / `InfoBeforeFile` / `InfoAfterFile` | empty |
| `ChangesEnvironment` / `ChangesAssociations` | `no` / `no` |
| `UsePrevious*` | app dir, group, setup type, tasks, user info all `yes` |

Language: only English (`en`, LCID 1033), Segoe UI dialog font.
Custom messages: the 12 stock `Default.isl` entries plus three script-defined ones:

| Message | Value |
|---|---|
| `StartupTask` | `Start Nusa when Windows starts` |
| `DesktopTask` | `Create a desktop shortcut` |
| `RemoveData` | `Also remove your Nusa data (to-do list, water log, shelf, settings)?` |

### 3.2 Install scope and paths (inference)

`PrivilegesRequired=lowest` with no `/ALLUSERS` or `/CURRENTUSER` override means Setup
never requests elevation (`asInvoker` manifest, no `runas`). Under Inno's "auto"
constant mapping, `{autopf}` resolves to `{userpf}` for a non-administrative install,
i.e. the effective default directory is:

```
%LOCALAPPDATA%\Programs\Nusa
```

The uninstall entry is therefore per-user (HKCU). `DisableDirPage=yes` hides the
directory page, so the user cannot change this. (The exact `{autopf}` → `{userpf}`
resolution is documented Inno behaviour, not a stored string; hence inference.)

### 3.3 `[Tasks]`

| Name | Description | Default | Notes |
|---|---|---|---|
| `startup` | "Start Nusa when Windows starts" | **checked** | gates the HKCU Run value |
| `desktop` | "Create a desktop shortcut" | **unchecked** (`unchecked` flag) | gates the desktop icon |

No components, no task groups, no `[Components]` at all.

### 3.4 `[Icons]`

| IconName | Target | Condition | Flags |
|---|---|---|---|
| `{autoprograms}\Nusa` | `{app}\nusa.exe` | none | none, index 0 |
| `{autodesktop}\Nusa` | `{app}\nusa.exe` | task `desktop` | none, index 0 |

The Start-menu shortcut is unconditional; the desktop shortcut is opt-in.

### 3.5 `[Registry]`

| Root | Key | Value name | Type | Data | Condition | Flags |
|---|---|---|---|---|---|---|
| HKCU | `Software\Microsoft\Windows\CurrentVersion\Run` | `Nusa` | `REG_SZ` | `"{app}\nusa.exe"` (quotes included) | task `startup` | `uninsdeletevalue` |

This is the same autostart mechanism the payload itself manipulates
(`HKCU\...\Run`), so the installer pre-seeds the Run value when the user leaves the
"start when Windows starts" box ticked.

### 3.6 `[Run]`

| Program | Parameters | Description | Wait | Flags |
|---|---|---|---|---|
| `{app}\nusa.exe` | — | `{cm:LaunchProgram,Nusa}` → "Launch Nusa" | no wait | `postinstall`, `skipifsilent`, `runasoriginaluser` |

So after a normal (non-silent) install, Setup offers/launches Nusa on the Finished
page; silent installs do not launch it. No `[UninstallRun]` entries.

### 3.7 `[Files]` and file data

| # | Source | Destination | Type | Flags |
|---|---|---|---|---|
| 0 | (empty) | (empty) | `ftUninstExe` | `overwritesameversion`; LocationEntry −1, PermissionsEntry −1 |
| 1 | (empty) | `{app}\nusa.exe` | `ftUserFile` | `ignoreversion`; embedded (ExternalSize 0) |

Entry 0 is the uninstaller placeholder that Setup materialises as `unins000.exe` in
`{app}` at install time (its data location is −1). Entry 1 is the only real payload;
its data is embedded (no external/`dontcopy` flag), and the stored SHA-256 matches the
extracted binary.

The source-name string is empty in the compiled header; only the destination is
recorded. The payload is stored as one LZMA2 chunk at `0xF1200` (4-byte `zlb\x1a` magic,
property byte `0x1A`, then a 461,497-byte stream) with Inno's x86 exe filter applied
before compression; §2 gives the full decode chain.

### 3.8 Compiled `[Code]`

`CompiledCodeText` is a 946-byte IFPS (Pascal Script) blob, format version 23. The
symbol table references exactly:

- event handler `CurUninstallStepChanged`
- identifiers `UninstallSilent`, `MsgBox`, `CustomMessage`, `DelTree`, `ExpandConstant`
- enum/type metadata `TUninstallStep`, `TMsgBoxType`, `TWizardForm`, `TUninstallProgressForm`
- UTF-16 string constants `RemoveData` and `{userappdata}\Nusa`

Reconstructed logic (**inference**; the operands were not disassembled byte-by-byte,
but the symbol/constant set and call order are unambiguous):

```pascal
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and not UninstallSilent then
    if MsgBox(CustomMessage('RemoveData'), mbConfirmation, MB_YESNO) = IDYES then
      DelTree(ExpandConstant('{userappdata}\Nusa'), True, True, True);
end;
```

Behavioural consequence: on an interactive uninstall the user is asked whether to also
remove "Nusa data"; on Yes the script deletes `{userappdata}\Nusa`
(`%APPDATA%\Nusa`). It is skipped in silent uninstall and on the normal uninstall step.

### 3.9 Uninstall "remove data" behaviour (corrected)

The payload builds **two** state locations, not one:

- `%APPDATA%\Nusa` — constructed in `appdata_nusa()` (`0x14003F86D`), which reads
  the `APPDATA` environment variable and appends `Nusa` (a 4-byte slice taken from
  the adjacent `NusaShelf` literal). It is referenced from 8 call sites
  (`0x140017C2F`, `0x14001ADDB`, `0x140029FFE`, `0x140034F41`, `0x1400354CB`,
  `0x14003FE8E`, `0x1400454CB`, `0x140046691`) and holds `settings.txt`,
  `todo.txt`, `water.txt` and `shelf.txt` (metadata). The todo write path at
  `0x14003FE6A` confirms this: it calls `appdata_nusa()`, creates the directory,
  joins `todo.txt` (`0x1400400B4`) and saves through `0x140016839`.
- `%LOCALAPPDATA%\Nusa\Shelf` — constructed in `localappdata_nusashelf()`
  (`0x14001750B`), which reads `LOCALAPPDATA`, appends `Nusa` (4 bytes) and then
  `Shelf` (5 bytes). Used by the shelf/file-drop feature (`0x140017D13`…).

The uninstall script's `DelTree(ExpandConstant('{userappdata}\Nusa'), …)` therefore
**does** target the app's primary state directory (`%APPDATA%\Nusa`), so on an
interactive uninstall the settings/todo/water/shelf-metadata files are removed.
It does **not** remove `%LOCALAPPDATA%\Nusa\Shelf`, where dropped shelf files live,
so those survive uninstall even when the user answers "Yes" to "Also remove your
Nusa data". That is the real (low-severity) defect: the prompt is only
partially effective. Silent and non-interactive uninstalls skip the prompt
entirely and leave all data in place, as before.

The earlier claim in this report that the payload never references `%APPDATA%\Nusa`
was wrong and has been corrected here and in §4/§5.

## 4. Payload — `app\nusa.exe`

| Item | Value |
|---|---|
| Size | 909,312 bytes |
| SHA-256 | `2dfa517fe20fb944cd263597e9f731b8245fc2b5253d9f1a241221636e8d1bdb` |
| Format | PE32+ x86-64, GUI subsystem, 7 sections (`.text .rdata .data .pdata .fptable .rsrc .reloc`) |
| Hardening | ASLR + DEP + high-entropy VA; no CFG; no embedded manifest; **not signed** |
| Language | Rust (rustc commit `48a229c…`), crate `windows-0.58.0` |
| Build provenance | user `rama2` (`C:\Users\rama2\.cargo\…`), PDB `nusa.pdb` |
| Embedded version | `Nusa 1.0.0` |

Self-description found in `.rdata`:

> "A living notch for Windows. Music, lyrics, focus timer, water reminder, to-do and file shelf.
> Lyrics provided by LRCLIB (lrclib.net)."

### What it does

- **Custom-drawn "notch" overlay**: Direct2D + DirectComposition + DirectWrite + D3D11,
  `SetWindowsHookExW` / `GetAsyncKeyState` for keyboard (e.g. Caps Lock indicator),
  `RegisterPowerSettingNotification` for battery/charger events,
  `Shell_NotifyIconW` for the tray icon.
- **Now-playing + lyrics**: WinRT `Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager`,
  then HTTP GET `https://lrclib.net/api/search?track_name=…&artist_name=…`
  with User-Agent `Nusa-Notch/1.0 (Windows)`.
- **Focus timer / Pomodoro**: 25-minute focus, 5-minute short break, 15-minute long break.
- **Water reminder**: bottle goal (1.5/2.0/2.5/3.0 L), refill/snooze state.
- **To-do list** and **file shelf** (drag & drop in/out, `.url` Internet Shortcut creation,
  `RegisterDragDrop` / `DoDragDrop` / `RegisterClipboardFormatW`).
- **Bluetooth device info** (`Windows.Devices.Bluetooth.BluetoothDevice`),
  **notification listener** (`Windows.UI.Notifications.Management.UserNotificationListener`),
  **network status** (`Windows.Networking.Connectivity.NetworkInformation`).
- **Single instance**: mutex `Local\NusaNotch.SingleInstance`.
- **Autostart**: writes/deletes `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
  (`RegSetKeyValueW` / `RegDeleteKeyValueW` / `RegGetValueW`) — user-toggleable.
  The Run-key string at `0x140094D0C` is code-referenced from `0x140017AFE`,
  `0x140017B4F`, `0x140019B1A`, `0x14001D82D`.
- **Local state** in two roots (see §3.9):
  - `%APPDATA%\Nusa` — `settings.txt`, `todo.txt`, `water.txt`, `shelf.txt`
    (metadata; constructed by `appdata_nusa()` at `0x14003F86D`);
  - `%LOCALAPPDATA%\Nusa\Shelf` — dropped shelf files
    (constructed by `localappdata_nusashelf()` at `0x14001750B`).
  Strings at `0x933C0`–`0x948C0`: `APPDATA`, `LOCALAPPDATA`, `NusaShelf`,
  the four file names; the `Nusa` component is a 4-byte slice of the
  `NusaShelf` literal in both constructors.

### Network / privacy

- Only one URL exists in the binary: `https://lrclib.net/api/search?` (lyrics lookup).
- No update endpoint, no telemetry endpoint, no analytics host.
- No `ws2_32`/`WinHTTP`/`WinINet` imports; HTTP goes through WinRT (`Windows.Web.Http`),
  which also means standard system proxy/TLS behaviour.
- Requests disclose the current track title and artist name to lrclib.net while a song plays.

## 5. Risk assessment

- **Clean of overt malicious behaviour on static evidence**: no persistence beyond an
  explicit HKCU `Run` toggle (and the same value the installer can pre-create), no
  service/driver, no scheduled task, no shell/`cmd` execution path, no process injection
  APIs, no credential access, no wallet/browser targeting, no packed/encrypted payload,
  no second-stage downloader.
- The installer is likewise minimal: one file, two shortcuts, one Run value, one run
  entry, one uninstall-time prompt. No `[Dirs]`, no `[INI]`, no `[InstallDelete]`,
  no `[UninstallDelete]`, no services, no firewall rules.
- **Unsigned publisher** ("Rama"), custom-drawn UI, and a single-BLOB Inno installer
  mean SmartScreen/reputation warnings are expected. Verify the source before running.
- Functional defect (low severity): the uninstall "remove my data" prompt removes
  `%APPDATA%\Nusa` (settings/todo/water/shelf metadata) but not
  `%LOCALAPPDATA%\Nusa\Shelf` (dropped shelf files), so part of the user data
  survives even when the user answers "Yes" (§3.9). Silent uninstalls remove
  nothing.
- The only external data flow is track metadata → lrclib.net.
- Unknowns: runtime behaviour was not executed/observed; the exact textual source of the
  `[Code]` handler is inferred (§3.8).

## 7. Renamed deliverables (Neno)

A byte-preserving rebrand to **Neno** was produced from copies of the original
artifacts (originals left untouched):

- `E:\NusaSetup\renamed\NenoSetup.exe` (SHA-256 `53CD44D323629212629CF3F081B83C779ABE4A65C3247F55A76347749596F5A0`, 3,319,711 bytes) - packages the current reconstruction build (GUI subsystem, no console)
- Embedded payload `neno.exe` (2,459,648 bytes, SHA-256 `204FB5B8A9AAE2F6F02E76C5F2BC6F8A56B485149ED94C40EDBB3E561AB47FFC`)
- `E:\NusaSetup\renamed\app\neno.exe`

Substitutions are same-length, so no offset shifts occur within uncompressed
data: `Nusa`→`Neno`, `nusa`→`neno` in all casings, in both ASCII and UTF-16
(including `NusaShelf`→`NenoShelf`, `NusaNotch`→`NenoNotch`,
`Nusa-Notch`→`Neno-Notch`, `{app}\nusa.exe`→`{app}\neno.exe`). Observed
identifiers (`Nusa`, `nusa.exe`, `{A03D4881-…}`, the original hashes) remain
documented above as evidence. Scope applied: name + exe only — `AppId` GUID and
publisher URL (`https://rama2555.github.io/nusa/`) intentionally unchanged;
publisher name "Rama" and icon unchanged; no data migration.

The rename rebuild (see `re-analysis\build_neno.py`) kept the embedded engine
block byte-identical, recomputed the LZMA1 primary/secondary blocks and the
LZMA2 payload chunk, and recomputed `TableCRC = zlib.crc32(first 60 bytes of
the loader table)`. Verification: `re-analysis\verify_neno.py` (primary/secondary
inflate exactly, loader fields consistent, payload decodes bit-identically to
`neno.exe`, PE version strings updated).

A reconstruction scaffold (Cargo/Rust skeleton) is kept at
`E:\NusaSetup\reconstruction\` (modules: notch, media, lyrics, focus, water,
todo, shelf, bluetooth, notifications, network, autostart, state, settings),
documenting the recovered architecture and data formats for future feature work.

## 6. Evidence and reproducibility

### REA evidence ledger

| Evidence ID | Operation | Subject |
|---|---|---|
| `ev_839a0cb0c73c58a64cc171c95dc0ee301562ec0874a3e320e3cdf2612ad350f5` | `inspect_pe_resources` | NusaSetup.exe |
| `ev_8d9148e21055b2b0d16988c30f9e83bd62511aa340c9d7ed1141b3df250d4a19` | `inspect_artifact` | NusaSetup.exe |
| `ev_0691ba350fa4125742d3a83c846e25a46a507ade07d41147f99393633c8cfbe2` | `binary_overview` (Ghidra) | NusaSetup.exe — 2,523 procedures / 1,998 strings |
| `ev_d51f0a24068639049b07a5e40e3d33c2b7364a90b713754ce10deea675398e30` | `binary_overview` (Ghidra) | nusa.exe — 1,712 procedures / 832 strings |
| `ev_7473d4cb5352e82e77f94e54150a1f400d7366dbf094ec9b5592ed4862eef66e` | `search_strings` (Ghidra) | NusaSetup.exe |
| `ev_47d30e8464bad9fe154b492ff397e67233d953a839a95fe7bc77d4c974ed6ca0` | `inspect_pe_resources` | nusa.exe |

### Reproducing the setup-table results

The table values above are direct byte-level parses, not REA tool output. They can be
reproduced with `E:\NusaSetup\re-analysis\parse_inno.py` (read-only; no execution of the
installer):

1. Locate the **last** `Inno Setup Setup Data (6.7.0)` occurrence (`0x161CBE`); the
   earlier `0xB530C` hit is an embedded constant, not the data header.
2. Skip the 64-byte ID and the 53-byte encryption header (`0x161CFE`, CRC-validated,
   `EncryptionUse=0`), then parse the block at `0x161D33`
   (`u32 CRC | u64 stored | u8 compressed`, then 4096-byte chunks each prefixed with a
   CRC-32; payload begins with 5-byte LZMA1 properties).
3. Inflate to 194,445 bytes and walk `TSetupHeader` → languages → messages → tasks →
   files → icons → INI → registry → deletes → runs → wizard images, then the secondary
   block at `0x16A941` for file locations.
4. Decode the payload chunk at `0xF1200`: skip `zlb\x1a`, read the LZMA2 property byte
   (`0x1A` → 32 MiB dictionary), inflate the 461,497-byte stream, then reverse the exe
   filter (`verify_exefilter.py`). The result is 909,312 bytes and matches the
   file-location entry's SHA-256; without the filter step it hashes to
   `59a15152…c71b10`.

Supporting artifacts kept in `re-analysis\`: `parse_inno.py`, `parse_out.txt`,
`compiled_code.bin` (the 946-byte IFPS blob), `verify_exefilter.py`,
`check_diff_pos.py`, `analyze.json`, `analyze-nusa.json`,
`strings.json`, `nusa-res.json`, `nusa_ascii.txt`, `nusa_utf16.txt`, `nusa-app-snap`
(Ghidra snapshot), and the payload at `E:\NusaSetup\extracted\app\nusa.exe`.
