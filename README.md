<div align="center">
  <img src="assets/icon.svg" width="100" alt="OC Setup Easy app icon">
  <h1>OC Setup Easy</h1>
  <p>Mount your EFI, compare installed kexts with <code>Kernel → Add</code>, and edit OpenCore on your Hackintosh.</p>
  <p><b>Rust</b> · Native macOS WebKit · Italian interface · Works offline</p>
  <p><a href="https://github.com/Skensy/OC-Setup-Easy/releases/latest"><b>Download for macOS (x86-64)</b></a></p>
</div>

![OC Setup Easy dashboard showing sample system and boot information](assets/panoramica.png)

## Install

Download the app ZIP from [GitHub Releases](https://github.com/Skensy/OC-Setup-Easy/releases/latest), unzip it, and drag **OC Setup Easy.app** into **Applications**. The published build is ad-hoc signed and has no Apple notarization. It targets macOS 11 or later and has been tested on an x86-64 AMD Hackintosh running macOS 26.7.1.

## Edit your first configuration

The interface is in Italian. The button names below match what you’ll see in the app.

1. Open **Volumi EFI** (EFI volumes). Identify your disk by its model, connection, and device identifier. Click **Monta** to mount it, then **Apri EFI** to open it. To work on a copy, use **Apri cartella EFI** and select the volume, the EFI folder, or the OC folder containing `config.plist`.
2. Open **Verifica e backup** (validation and backup) and create a ZIP before making changes. When OC sits inside EFI, the archive includes the entire EFI folder. Opening a standalone OC folder backs up that folder.
3. Open **Kext e componenti** (kexts and components). **Copia un kext** copies a bundle to disk. **Registra** adds a disabled entry to the draft plist. Review its plug-ins, enable the entries you need, and use the arrows to set their load order.
4. Open **Editor plist** to edit fields by section. Switch to **XML** to add or rename keys or edit whole arrays. **Salva plist** or **⌘S** validates and saves your draft. Close the configuration before unmounting its volume.

## What you can inspect

| View | Data and controls |
| --- | --- |
| Dashboard | CPU, core and thread counts, physical memory, macOS version and build, Darwin kernel, architecture, SMBIOS model, and the OpenCore version reported by NVRAM when available. |
| EFI volumes | Disk model, connection, partition size, mount point, and mount controls. macOS may ask for administrator authorization. |
| Plist editor | Standard and custom sections, dictionaries, arrays, and scalar values. XML and binary plists retain their original format when saved. |
| Kexts and components | On-disk bundles and plug-ins, versions, bundle IDs, declared dependencies, and load order among detected bundles. Compare and register ACPI tables, UEFI drivers, and tools. Removing a plist entry leaves its file on disk. |
| Validation and backups | Full ocvalidate output for the current draft, a copy of the previous plist on every save, and ZIP backups on demand. |
| Hardware and system | Boot arguments, loaded kexts, power profiles, and the reports available through system_profiler on your Mac, including graphics, audio, PCI, USB, networking, and NVMe. |

## Backups and recovery

Before saving, the app checks the on-disk plist against the version you opened. If another editor changed it, the save stops. Otherwise the app writes the new plist to a temporary file on the same volume, backs up the original, and replaces it. Importing a kext also checks for an existing bundle with the same name.

Use **Verifica e backup → Mostra backup** to open `~/Library/Application Support/OC Setup Easy/Backups`. To restore a plist, close the configuration, copy the backup into `EFI/OC` as `config.plist`, and reopen the folder. To recover individual EFI files, extract the ZIP to a separate folder first.

The bundled validator checks the **OpenCore 1.0.8** schema. Other versions can have different rules. Review missing files in the components view and test bootability on your hardware. [Official OpenCore 1.0.8 release](https://github.com/acidanthera/OpenCorePkg/releases/tag/1.0.8)

## Build from source

Install the Xcode Command Line Tools and stable Rust with Cargo on macOS. Run the following from the repository root.

```sh
./scripts/build-app.sh
open "dist/OC Setup Easy.app"
```

The script builds for the host Mac’s architecture. On the first build, it downloads the Rust dependencies and the official OpenCore 1.0.8 archive. It verifies the archive’s SHA-256 digest, extracts ocvalidate, and ad-hoc signs the app bundle.

### Source and tests

The Rust backend lives in `src/`. The AppKit window and WKWebView bridge are in `native/window.m`. The HTML, CSS, JavaScript, font, and graphics are in `ui/`.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

The tests cover partition identifiers, opening a nested EFI folder, backups, conflicting edits, binary plists, importing kexts with plug-ins, and duplicate bundles.

OpenCore and ocvalidate are maintained by Acidanthera. The app includes the licenses for ocvalidate and the Barlow Semi Condensed typeface. [OpenCorePkg license](licenses/OpenCorePkg.txt) · [Barlow license](ui/FONT-LICENSE.txt)
