use crate::workspace::rename_exclusive;
use crate::{
    diagnostics::string,
    maintenance, system,
    workspace::{Workspace, backups},
};
use anyhow::{Context, Result, bail, ensure};
use plist::Value as P;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
};

const STANDARD: &[&str] = &[
    "Lilu",
    "VirtualSMC",
    "WhateverGreen",
    "AppleALC",
    "RestrictEvents",
    "IntelMausi",
    "NVMeFix",
];
const AMD: &str = "AMDRyzenCPUPowerManagement";
type Tree = BTreeMap<PathBuf, Option<Vec<u8>>>;

pub struct Candidate {
    path: PathBuf,
    id: String,
    url: String,
    digest: String,
    version: String,
    before: Tree,
}
fn version(s: &str) -> Option<[u32; 4]> {
    let (base, fix) = s
        .trim_start_matches('v')
        .split_once('f')
        .unwrap_or((s.trim_start_matches('v'), "0"));
    let mut parts = base.split('.');
    let v = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next().unwrap_or("0").parse().ok()?,
        fix.parse().ok()?,
    ];
    parts.next().is_none().then_some(v)
}
fn snapshot(root: &Path) -> Result<Tree> {
    fn scan(base: &Path, path: &Path, tree: &mut Tree) -> Result<()> {
        for e in fs::read_dir(path)? {
            let e = e?;
            let p = e.path();
            let ty = e.file_type()?;
            ensure!(
                ty.is_file() || ty.is_dir(),
                "Bundle con collegamenti o file speciali"
            );
            tree.insert(
                p.strip_prefix(base)?.into(),
                if ty.is_file() {
                    Some(fs::read(&p)?)
                } else {
                    None
                },
            );
            if ty.is_dir() {
                scan(base, &p, tree)?;
            }
        }
        Ok(())
    }
    ensure!(root.canonicalize()? == root, "Destinazione non valida");
    let mut tree = Tree::new();
    scan(root, root, &mut tree)?;
    Ok(tree)
}
pub fn check(w: &mut Workspace) -> Result<Value> {
    w.kext_updates.clear();
    w.check()?;
    let inventory = w.inventory()?;
    let mut rows = Vec::new();
    for item in inventory["Kexts"].as_array().into_iter().flatten() {
        let rel = item["path"].as_str().unwrap_or_default();
        let Some(name) = rel.strip_suffix(".kext") else {
            continue;
        };
        if !(STANDARD.contains(&name) || name == AMD) {
            continue;
        }
        let repo = if name == AMD {
            "trulyspinach/SMCAMDProcessor".into()
        } else {
            format!("acidanthera/{name}")
        };
        let installed = item["shortVersion"].as_str().unwrap_or("");
        let mut row = json!({"name":name,"installed":installed});
        let candidate = (|| -> Result<Option<Candidate>> {
            let release = maintenance::release(&repo)?;
            ensure!(
                release["draft"] == false && release["prerelease"] == false,
                "Release non stabile"
            );
            let tag = release["tag_name"]
                .as_str()
                .context("Missing release version")?;
            let latest = version(tag).context("Unexpected release version")?;
            row["latest"] = json!(tag);
            if latest <= version(installed).context("Versione installata non valida")? {
                return Ok(None);
            }
            let filename = if name == AMD {
                format!("{name}.kext.zip")
            } else {
                format!("{name}-{tag}-RELEASE.zip")
            };
            let asset = release["assets"]
                .as_array()
                .and_then(|a| a.iter().find(|a| a["name"] == filename))
                .context("Release archive missing")?;
            let url = format!("https://github.com/{repo}/releases/download/{tag}/{filename}");
            ensure!(
                asset["browser_download_url"] == url,
                "Origine archivio non valida"
            );
            let published_digest = asset["digest"]
                .as_str()
                .map(|s| {
                    s.strip_prefix("sha256:")
                        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                        .context("Missing release SHA-256")
                })
                .transpose()?;
            // GitHub did not publish digests for these historical release assets.
            let digest = published_digest
                .or(match (name, tag) {
                    ("IntelMausi", "1.0.8") => {
                        Some("cc02ea7e972ead536a51e5f6725e79f121026dbf36a766e97044ef2483fd29bc")
                    }
                    ("AMDRyzenCPUPowerManagement", "0.7.2f1") => {
                        Some("844cd318a3f137d46a10e204e1367dbc9593716120289f5261dc2d48110869b4")
                    }
                    _ => None,
                })
                .context("Missing release SHA-256")?
                .into();
            let path = w.root.join("Kexts").join(rel);
            let candidate = Candidate {
                id: item["id"]
                    .as_str()
                    .context("CFBundleIdentifier assente")?
                    .into(),
                before: snapshot(&path)?,
                path,
                url,
                digest,
                version: tag.into(),
            };
            if name == AMD
                && tag
                    .split_once('f')
                    .is_some_and(|(base, _)| base == installed)
            {
                let temp = tempfile::tempdir()?;
                let next = prepare(&candidate, &temp.path().canonicalize()?.join("bundle"))?;
                let binary = Path::new("Contents/MacOS/AMDRyzenCPUPowerManagement");
                if next.get(binary) == candidate.before.get(binary) {
                    return Ok(None);
                }
            }
            Ok(Some(candidate))
        })();
        match candidate {
            Ok(Some(c)) => {
                row["index"] = json!(w.kext_updates.len());
                w.kext_updates.push(c);
            }
            Ok(None) => (),
            Err(e) => row["error"] = json!(e.to_string()),
        }
        rows.push(row);
    }
    Ok(json!(rows))
}

fn unpack(file: File, dest: &Path, name: &str) -> Result<()> {
    let mut archive = zip::ZipArchive::new(file).context("Archivio ZIP non valido")?;
    ensure!(archive.len() <= 20000, "Archivio ZIP troppo grande");
    let info = format!("{name}/Contents/Info.plist");
    let mut roots = archive.file_names().filter(|p| {
        !p.starts_with("__MACOSX/")
            && (*p == info
                || p.strip_suffix(&info)
                    .is_some_and(|prefix| prefix.ends_with('/')))
    });
    let root = roots
        .next()
        .context("Bundle assente nella release")?
        .strip_suffix("/Contents/Info.plist")
        .unwrap()
        .to_owned()
        + "/";
    ensure!(roots.next().is_none(), "Bundle ambiguo nella release");
    drop(roots);
    fs::create_dir(dest)?;
    let mut remaining = 128 * 1024 * 1024u64;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).context("Archivio ZIP non valido")?;
        let Some(rel) = f.name().strip_prefix(&root).filter(|r| !r.is_empty()) else {
            continue;
        };
        ensure!(
            !rel.contains(['\\', ':', '\0'])
                && rel.split('/').all(|s| s != ".." && s != ".")
                && Path::new(rel)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "Percorso ZIP non valido"
        );
        let path = dest.join(rel);
        let mode = f.unix_mode().unwrap_or(0);
        ensure!(
            matches!(mode & 0o170000, 0 | 0o100000 | 0o040000),
            "Bundle con collegamenti o file speciali"
        );
        if f.is_dir() {
            fs::create_dir_all(path)?;
            continue;
        }
        ensure!(f.size() <= remaining, "Archivio ZIP troppo grande");
        fs::create_dir_all(path.parent().context("Destinazione non valida")?)?;
        let mut out = File::create_new(&path)?;
        let len = f.size();
        ensure!(
            std::io::copy(&mut (&mut f).take(len + 1), &mut out)? == len,
            "Archivio ZIP non valido"
        );
        remaining -= len;
        out.flush()?;
        out.sync_all()?;
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(if mode & 0o111 != 0 || rel_is_executable(&path) {
                0o755
            } else {
                0o644
            }),
        )?;
    }
    Ok(())
}
fn rel_is_executable(path: &Path) -> bool {
    path.parent().is_some_and(|p| p.ends_with("Contents/MacOS"))
}
fn prepare(c: &Candidate, dest: &Path) -> Result<Tree> {
    let zip = maintenance::download(&c.url, &c.digest)?;
    unpack(
        zip.reopen()?,
        dest,
        c.path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Nome bundle assente")?,
    )?;
    let info = P::from_file(dest.join("Contents/Info.plist"))?;
    let v = version(string(&info, "CFBundleShortVersionString"))
        .context("Versione installata non valida")?;
    ensure!(
        string(&info, "CFBundleIdentifier") == c.id
            && (Some(v) == version(&c.version)
                || (c.path.ends_with("AMDRyzenCPUPowerManagement.kext")
                    && c.version
                        .split_once('f')
                        .is_some_and(|(base, _)| Some(v) == version(base)))),
        "Identità o versione del bundle diversa dalla release"
    );
    let exe = string(&info, "CFBundleExecutable");
    ensure!(
        !exe.is_empty()
            && Path::new(exe).file_name() == Some(exe.as_ref())
            && dest.join("Contents/MacOS").join(exe).is_file(),
        "Bundle incompleto"
    );
    snapshot(dest)
}

// Keep both directories on the destination volume. Retain the staging directory
// if an external writer prevents restoring the previous bundle.
fn exchange(dest: &Path, other: &Path) -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    unsafe extern "C" {
        fn renamex_np(a: *const i8, b: *const i8, flags: u32) -> i32;
    }
    let a = CString::new(dest.as_os_str().as_bytes())?;
    let b = CString::new(other.as_os_str().as_bytes())?;
    if unsafe { renamex_np(a.as_ptr(), b.as_ptr(), 2) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    // FAT32 reports DirectoryNotEmpty for RENAME_SWAP on populated bundles.
    if !matches!(
        error.kind(),
        std::io::ErrorKind::Unsupported
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::DirectoryNotEmpty
            | std::io::ErrorKind::AlreadyExists
    ) {
        return Err(error.into());
    }
    exchange_by_rename(dest, other)
}
fn exchange_by_rename(dest: &Path, other: &Path) -> Result<()> {
    let old = other.with_extension("previous");
    rename_exclusive(dest, &old)?;
    if let Err(e) = rename_exclusive(other, dest) {
        rename_exclusive(&old, dest)?;
        return Err(e);
    }
    rename_exclusive(&old, other)
}
fn verify_backup(w: &Workspace, plans: &[&Candidate], backup: &Path) -> Result<()> {
    system::output(
        "/usr/bin/unzip",
        &["-tqq", backup.to_str().context("Invalid backup path")?],
    )?;
    let base = maintenance::efi(w)?
        .parent()
        .context("Destinazione non valida")?;
    let mut zip = zip::ZipArchive::new(File::open(backup)?).context("Archivio ZIP non valido")?;
    let mut verify = |path: &Path, bytes: &[u8]| -> Result<()> {
        let key = path
            .strip_prefix(base)?
            .to_str()
            .context("Percorso non UTF-8")?;
        let mut entry = zip.by_name(key).context("Backup ZIP incompleto")?;
        ensure!(entry.size() == bytes.len() as u64, "Backup ZIP incompleto");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        ensure!(buf == bytes, "Backup ZIP incompleto");
        Ok(())
    };
    let config = w.root.join("config.plist");
    verify(&config, &fs::read(&config)?)?;
    for c in plans {
        for (file, content) in &c.before {
            if let Some(bytes) = content {
                verify(&c.path.join(file), bytes)?;
            }
        }
    }
    Ok(())
}
pub fn apply(w: &mut Workspace, selected: Vec<usize>) -> Result<Value> {
    maintenance::efi(w)?;
    w.assert_unchanged()?;
    let ids: BTreeSet<_> = selected.iter().copied().collect();
    ensure!(
        !ids.is_empty()
            && ids.len() == selected.len()
            && ids.iter().all(|i| *i < w.kext_updates.len()),
        "Selezione aggiornamenti non valida"
    );
    let plans: Vec<_> = ids.iter().map(|i| &w.kext_updates[*i]).collect();
    let downloads = tempfile::tempdir()?;
    let mut expected = Vec::new();
    for (i, c) in plans.iter().enumerate() {
        expected.push(prepare(
            c,
            &downloads.path().canonicalize()?.join(i.to_string()),
        )?);
    }
    let _lock = maintenance::write_lock(&backups()?)?;
    for c in &plans {
        ensure!(
            snapshot(&c.path)? == c.before,
            "Bundle cambiato dopo l’anteprima"
        );
    }
    let backup = w.backup()?;
    verify_backup(w, &plans, &backup)?;
    let dir = w.root.join("Kexts");
    ensure!(dir.canonicalize()? == dir, "Destinazione non valida");
    let stage = tempfile::tempdir_in(dir)?;
    for (i, tree) in expected.iter().enumerate() {
        let name = i.to_string();
        let dst = stage.path().join(&name);
        let src = downloads.path().join(&name);
        system::output(
            "/usr/bin/ditto",
            &[
                src.to_str().context("Percorso non UTF-8")?,
                dst.to_str().context("Percorso non UTF-8")?,
            ],
        )?;
        ensure!(snapshot(&dst)? == *tree, "Copia del bundle incompleta");
    }
    for (i, c) in plans.iter().enumerate() {
        let result = (|| -> Result<()> {
            w.assert_unchanged()?;
            ensure!(
                snapshot(&c.path)? == c.before,
                "Bundle cambiato dopo l’anteprima"
            );
            exchange(&c.path, &stage.path().join(i.to_string()))
        })();
        if let Err(e) = result {
            let mut errors = Vec::new();
            for j in (0..i).rev() {
                let p = &plans[j].path;
                let old = stage.path().join(j.to_string());
                let rollback = (|| -> Result<()> {
                    ensure!(
                        snapshot(p)? == expected[j] && snapshot(&old)? == plans[j].before,
                        "Bundle cambiato dopo l’anteprima"
                    );
                    exchange(p, &old)
                })();
                if let Err(err) = rollback {
                    errors.push(err.to_string());
                }
            }
            let clean = errors.is_empty()
                && plans
                    .iter()
                    .take(i + 1)
                    .all(|c| snapshot(&c.path).is_ok_and(|tree| tree == c.before));
            if clean {
                return Err(e.context(format!("ZIP: {}", backup.display())));
            }
            let retained = stage.keep();
            bail!(
                "Aggiornamento interrotto: {e}: ZIP: {}: {}: {}",
                backup.display(),
                retained.display(),
                errors.join(", ")
            );
        }
    }
    w.kext_updates.clear();
    Ok(json!(backup))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_versions_include_amd_fixes() {
        assert!(version("0.7.2f1") > version("0.7.2"));
        assert!(version("1.10.0") > version("1.9.9"));
        assert!(version("1.2.3-rc1").is_none());
    }
    #[test]
    fn fallback_exchange_restores_destination_when_stage_is_missing() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let live = tmp.path().join("live");
        let staged = tmp.path().join("next");
        fs::create_dir(&live)?;
        fs::write(live.join("original"), b"original")?;
        assert!(exchange_by_rename(&live, &staged).is_err());
        assert_eq!(fs::read(live.join("original"))?, b"original");
        fs::create_dir(&staged)?;
        fs::write(staged.join("new"), b"new")?;
        exchange_by_rename(&live, &staged)?;
        assert_eq!(fs::read(live.join("new"))?, b"new");
        assert_eq!(fs::read(staged.join("original"))?, b"original");
        Ok(())
    }
    #[test]
    fn readable_zip_with_wrong_backup_bytes_is_rejected() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let oc = tmp.path().join("EFI/OC");
        fs::create_dir_all(&oc)?;
        fs::write(oc.join("config.plist"), "<plist><dict/></plist>")?;
        let w = Workspace::open(&oc)?;
        let path = tmp.path().join("backup.zip");
        for (bytes, valid) in [
            (b"old".as_slice(), false),
            (b"<plist><dict/></plist>", true),
        ] {
            let mut writer = zip::ZipWriter::new(File::create(&path)?);
            writer.start_file(
                "EFI/OC/config.plist",
                zip::write::SimpleFileOptions::default(),
            )?;
            writer.write_all(bytes)?;
            writer.finish()?;
            assert_eq!(verify_backup(&w, &[], &path).is_ok(), valid);
        }
        Ok(())
    }
    #[test]
    fn extraction_confines_bundle_and_swap_preserves_previous_tree() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let dir = tmp.path().canonicalize()?;
        let zip = dir.join("sample.zip");
        for (entry, valid) in [("Contents/Info.plist", true), ("../escape", false)] {
            let mut writer = zip::ZipWriter::new(File::create(&zip)?);
            for name in [
                "Kexts/Foo.kext/Contents/Info.plist",
                &format!("Kexts/Foo.kext/{entry}"),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
            {
                writer.start_file(name, zip::write::SimpleFileOptions::default())?;
                writer.write_all(b"new")?;
            }
            writer.finish()?;
            let bundle = dir.join(if valid { "good" } else { "bad" });
            assert_eq!(
                unpack(File::open(&zip)?, &bundle, "Foo.kext").is_ok(),
                valid
            );
        }
        assert!(!dir.join("escape").exists());
        let live = dir.join("live");
        fs::create_dir(&live)?;
        fs::write(live.join("old"), b"before")?;
        exchange(&live, &dir.join("good"))?;
        assert_eq!(fs::read(live.join("Contents/Info.plist"))?, b"new");
        assert_eq!(fs::read(dir.join("good/old"))?, b"before");
        Ok(())
    }
}
