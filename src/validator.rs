use crate::{plist_tools, system};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

pub fn cache() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("HOME").context("Home non disponibile")?)
            .join("Library/Application Support/OC Setup Easy/Cache"),
    )
}
fn digest(path: &Path) -> Result<String> {
    let out = system::output(
        "/usr/bin/shasum",
        &["-a", "256", path.to_str().context("Percorso non valido")?],
    )?;
    Ok(std::str::from_utf8(&out)?
        .split_whitespace()
        .next()
        .context("Missing release SHA-256")?
        .into())
}
pub fn install(archive: &Path, version: &str) -> Result<()> {
    let dest = cache()?.join("ocvalidate").join(version);
    fs::create_dir_all(&dest)?;
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
    let mut extract = |name: &str| -> Result<NamedTempFile> {
        let entry = zip.by_name(name)?;
        ensure!(
            entry.size() <= 16 * 1024 * 1024,
            "Archivio ZIP troppo grande"
        );
        let mut file = NamedTempFile::new_in(&dest)?;
        std::io::copy(&mut entry.take(16 * 1024 * 1024 + 1), &mut file)?;
        Ok(file)
    };
    let loader = extract("X64/EFI/OC/OpenCore.efi")?;
    let bin = extract("Utilities/ocvalidate/ocvalidate")?;
    let mut hashes = NamedTempFile::new_in(&dest)?;
    writeln!(hashes, "{} {}", digest(loader.path())?, digest(bin.path())?)?;
    bin.as_file()
        .set_permissions(fs::Permissions::from_mode(0o700))?;
    bin.persist(dest.join("ocvalidate"))?;
    hashes.persist(dest.join("SHA256"))?;
    Ok(())
}
fn matched(root: &Path, cache: &Path) -> Option<(PathBuf, String)> {
    let candidates = fs::read_dir(cache).ok()?;
    let loader = root.join("OpenCore.efi");
    if loader.canonicalize().ok()? != loader {
        return None;
    }
    let hash = digest(&loader).ok()?;
    for dir in candidates.flatten() {
        let path = dir.path();
        let bin = path.join("ocvalidate");
        if let Ok(meta) = fs::read_to_string(path.join("SHA256"))
            && let Some((efi, tool)) = meta.trim().split_once(' ')
            && efi == hash
            && bin.canonicalize().ok().as_deref() == Some(&bin)
            && digest(&bin).ok().as_deref() == Some(tool)
        {
            return Some((bin, dir.file_name().to_string_lossy().into_owned()));
        }
    }
    None
}
pub fn select(root: &Path) -> Result<(PathBuf, String)> {
    if let Some(pair) = matched(root, &cache()?.join("ocvalidate")) {
        return Ok(pair);
    }
    Ok((plist_tools::resource("ocvalidate")?, "1.0.8".into()))
}

#[test]
fn only_matching_loader_and_intact_tool_select_cache() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    let dir = root.join("cache/1.2.3");
    fs::create_dir_all(&dir)?;
    let loader = root.join("OpenCore.efi");
    let bin = dir.join("ocvalidate");
    fs::write(&bin, "tool")?;
    fs::write(&loader, "loader")?;
    fs::write(
        dir.join("SHA256"),
        format!("{} {}", digest(&loader)?, digest(&bin)?),
    )?;
    assert_eq!(
        matched(&root, &root.join("cache")),
        Some((bin.clone(), "1.2.3".into()))
    );
    fs::write(bin, "different")?;
    assert!(matched(&root, &root.join("cache")).is_none());
    Ok(())
}
