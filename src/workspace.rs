use crate::system::output;
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use plist::Value as P;
use serde_json::{Value, json};
use std::{
    ffi::CString,
    fs::{self, File},
    io::{Cursor, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt},
    },
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::{Builder, NamedTempFile};

const CONFIG: &str = "config.plist";
pub struct Workspace {
    pub root: PathBuf,
    baseline: Vec<u8>,
    device: u64,
    inode: u64,
}
fn xml(bytes: &[u8]) -> Result<String> {
    let parsed = P::from_reader(Cursor::new(bytes)).context("Plist non leggibile")?;
    ensure!(
        parsed.as_dictionary().is_some(),
        "La radice del plist deve essere un dizionario"
    );
    if !bytes.starts_with(b"bplist")
        && let Ok(text) = std::str::from_utf8(bytes)
        && text.trim_start().starts_with('<')
    {
        return Ok(text.into());
    }
    let mut out = Vec::new();
    parsed.to_writer_xml(&mut out)?;
    Ok(String::from_utf8(out)?)
}
pub fn backups() -> Result<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("HOME").context("Home non disponibile")?)
        .join("Library/Application Support/OC Setup Easy/Backups");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    Ok(dir)
}
impl Workspace {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize().context("Cartella non disponibile")?;
        let root = [path.clone(), path.join("OC"), path.join("EFI/OC")]
            .into_iter()
            .find(|p| p.join(CONFIG).is_file())
            .context("config.plist assente. Seleziona il volume, EFI o OC.")?;
        let root = root.canonicalize()?;
        ensure!(
            root.starts_with(&path),
            "La cartella OC punta fuori dalla selezione"
        );
        let config = root.join(CONFIG);
        ensure!(
            config.canonicalize()? == config,
            "config.plist è un collegamento simbolico"
        );
        let baseline = fs::read(config)?;
        xml(&baseline)?;
        let meta = root.metadata()?;
        Ok(Self {
            root,
            baseline,
            device: meta.dev(),
            inode: meta.ino(),
        })
    }
    fn check(&self) -> Result<()> {
        let m = self
            .root
            .metadata()
            .context("La cartella è stata scollegata")?;
        ensure!(
            (m.dev(), m.ino()) == (self.device, self.inode),
            "Il volume o la cartella è cambiato. Riaprilo."
        );
        Ok(())
    }
    pub fn view(&self) -> Result<Value> {
        Ok(json!({"root":self.root,"xml":xml(&self.baseline)?,"inventory":self.inventory()?}))
    }
    pub fn save(&mut self, text: &str) -> Result<PathBuf> {
        self.save_in(text, &backups()?)
    }
    fn save_in(&mut self, text: &str, backup_dir: &Path) -> Result<PathBuf> {
        self.check()?;
        let parsed = P::from_reader_xml(text.as_bytes()).context("XML plist non valido")?;
        ensure!(parsed.as_dictionary().is_some(), "Radice plist non valida");
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(backup_dir.join(".save.lock"))?;
        lock.try_lock_exclusive()
            .context("Un altro salvataggio è in corso")?;
        let target = self.root.join(CONFIG);
        let check_file = || -> Result<()> {
            ensure!(
                target.canonicalize()? == target && fs::read(&target)? == self.baseline,
                "Il file su disco è cambiato. Riapri la cartella per leggere la nuova versione."
            );
            Ok(())
        };
        check_file()?;
        let mut next = Vec::new();
        if self.baseline.starts_with(b"bplist") {
            parsed.to_writer_binary(&mut next)?;
        } else {
            next.extend_from_slice(text.as_bytes());
        }
        let mut temp = NamedTempFile::new_in(&self.root).context("Cartella OC non scrivibile")?;
        temp.write_all(&next)?;
        temp.as_file()
            .set_permissions(target.metadata()?.permissions())?;
        temp.as_file().sync_all()?;
        let mut bak = Builder::new()
            .prefix("config-")
            .suffix(".plist")
            .tempfile_in(backup_dir)?;
        bak.write_all(&self.baseline)?;
        bak.as_file().sync_all()?;
        let (_, backup) = bak.keep()?;
        check_file()?;
        temp.persist(target)?;
        self.baseline = next;
        Ok(backup)
    }
    pub fn inventory(&self) -> Result<Value> {
        self.check()?;
        let mut result = json!({});
        for (folder, extensions) in [
            ("Kexts", &["kext"][..]),
            ("ACPI", &["aml", "bin"][..]),
            ("Drivers", &["efi"][..]),
            ("Tools", &["efi"][..]),
        ] {
            let base = self.root.join(folder);
            let mut items = Vec::new();
            if base.exists() {
                ensure!(
                    base.canonicalize()? == base,
                    "Collegamento simbolico in {folder}"
                );
                Self::scan(&base, &base, extensions, &mut items)?;
            }
            items.sort_by_key(|r| r["path"].as_str().unwrap_or("").to_lowercase());
            result[folder] = json!(items);
        }
        Ok(result)
    }
    fn scan(base: &Path, dir: &Path, extensions: &[&str], out: &mut Vec<Value>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let rel = path.strip_prefix(base)?;
            let ty = entry.file_type()?;
            if ty.is_symlink() || entry.file_name().as_bytes().starts_with(b".") {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ty.is_dir() && ext == "kext" && extensions.contains(&"kext") {
                let info = path.join("Contents/Info.plist");
                let meta = info
                    .canonicalize()
                    .ok()
                    .filter(|p| p.starts_with(&path))
                    .and_then(|p| P::from_file(p).ok());
                let get = |k: &str| {
                    meta.as_ref()
                        .and_then(P::as_dictionary)
                        .and_then(|d| d.get(k))
                        .and_then(P::as_string)
                };
                let executable = get("CFBundleExecutable")
                    .map(|e| format!("Contents/MacOS/{e}"))
                    .unwrap_or_default();
                let valid = get("CFBundleIdentifier").is_some()
                    && (executable.is_empty()
                        || path
                            .join(&executable)
                            .canonicalize()
                            .is_ok_and(|p| p.starts_with(&path) && p.is_file()));
                out.push(json!({"path":rel,"id":get("CFBundleIdentifier"),"version":get("CFBundleVersion"),"executable":executable,"valid":valid,
                    "dependencies":meta.as_ref().and_then(P::as_dictionary).and_then(|d|d.get("OSBundleLibraries")).and_then(P::as_dictionary).map(|d|d.keys().collect::<Vec<_>>())}));
                let plugins = path.join("Contents/PlugIns");
                if plugins.is_dir() && plugins.canonicalize()? == plugins {
                    Self::scan(base, &plugins, extensions, out)?;
                }
            } else if ty.is_file() && extensions.contains(&ext.as_str()) {
                out.push(json!({"path":rel,"bytes":entry.metadata()?.len()}));
            } else if ty.is_dir() && !extensions.contains(&"kext") {
                Self::scan(base, &path, extensions, out)?;
            }
        }
        Ok(())
    }
    pub fn import(&self, source: &Path) -> Result<Value> {
        self.check()?;
        let source = source.canonicalize()?;
        ensure!(
            source
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("kext")),
            "Seleziona un bundle .kext"
        );
        let info = P::from_file(source.join("Contents/Info.plist"))?;
        ensure!(
            info.as_dictionary()
                .and_then(|d| d.get("CFBundleIdentifier"))
                .and_then(P::as_string)
                .is_some(),
            "CFBundleIdentifier assente"
        );
        let dir = self.root.join("Kexts");
        if !dir.exists() {
            fs::create_dir(&dir)?;
        }
        ensure!(
            dir.canonicalize()? == dir && !dir.starts_with(&source),
            "Destinazione non valida"
        );
        let dest = dir.join(source.file_name().context("Nome bundle assente")?);
        ensure!(!dest.exists(), "Esiste già un kext con questo nome");
        let staging = tempfile::tempdir_in(&dir)?;
        let staged = staging.path().join("bundle.kext");
        output(
            "/usr/bin/ditto",
            &[
                source.to_str().context("Percorso non UTF-8")?,
                staged.to_str().context("Percorso non UTF-8")?,
            ],
        )?;
        let mut copied = Vec::new();
        Self::scan(staging.path(), staging.path(), &["kext"], &mut copied)?;
        ensure!(
            !copied.is_empty() && copied.iter().all(|k| k["valid"] == true),
            "Metadati o eseguibile mancanti nel bundle copiato"
        );
        // macOS renamex_np with RENAME_EXCL publishes the copy without replacing a concurrent import.
        unsafe extern "C" {
            fn renamex_np(a: *const i8, b: *const i8, flags: u32) -> i32;
        }
        let a = CString::new(staged.as_os_str().as_bytes())?;
        let b = CString::new(dest.as_os_str().as_bytes())?;
        if unsafe { renamex_np(a.as_ptr(), b.as_ptr(), 4) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        self.inventory()
    }
    pub fn backup(&self) -> Result<PathBuf> {
        self.check()?;
        let dir = backups()?;
        let source = self
            .root
            .parent()
            .filter(|p| p.file_name().is_some_and(|s| s.eq_ignore_ascii_case("EFI")))
            .unwrap_or(&self.root);
        ensure!(
            !dir.starts_with(source),
            "La destinazione del backup si trova nella sorgente"
        );
        let zip = Builder::new()
            .prefix("OpenCore-")
            .suffix(".zip")
            .tempfile_in(&dir)?;
        output(
            "/usr/bin/ditto",
            &[
                "-c",
                "-k",
                "--keepParent",
                source.to_str().context("Percorso non UTF-8")?,
                zip.path().to_str().context("Percorso non UTF-8")?,
            ],
        )?;
        zip.as_file().sync_all()?;
        Ok(zip.keep()?.1)
    }
}
pub fn validate(text: &str) -> Result<Value> {
    let mut file = NamedTempFile::new()?;
    file.write_all(text.as_bytes())?;
    let bin = std::env::current_exe()?
        .parent()
        .context("Bundle non disponibile")?
        .join("../Resources/ocvalidate");
    if !bin.is_file() {
        bail!("ocvalidate assente nel bundle");
    }
    let res = Command::new(bin).arg(file.path()).output()?;
    Ok(
        json!({"pass":res.status.success(),"log":format!("{}{}",String::from_utf8_lossy(&res.stdout),String::from_utf8_lossy(&res.stderr))}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGINAL: &str = "<plist version=\"1.0\"><dict><key>Kernel</key><dict/></dict></plist>";
    #[test]
    fn preserve_and_detect_conflict() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let oc = tmp.path().join("EFI/OC");
        fs::create_dir_all(&oc)?;
        let target = oc.join(CONFIG);
        fs::write(&target, ORIGINAL)?;
        let mut w = Workspace::open(tmp.path())?;
        assert!(w.save_in("bad", tmp.path()).is_err());
        assert_eq!(
            fs::read_to_string(w.save_in(&ORIGINAL.replace("Kernel", "Misc"), tmp.path())?)?,
            ORIGINAL
        );
        fs::write(&target, "external")?;
        assert!(w.save_in(ORIGINAL, tmp.path()).is_err());
        assert_eq!(fs::read_to_string(target)?, "external");
        Ok(())
    }
    #[test]
    fn binary_and_nested_kext() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path();
        let cfg = root.join(CONFIG);
        P::from_reader_xml(ORIGINAL.as_bytes())?.to_file_binary(&cfg)?;
        let mut w = Workspace::open(root)?;
        let text = xml(&w.baseline)?;
        w.save_in(&text, root)?;
        assert!(fs::read(cfg)?.starts_with(b"bplist"));
        let src = tempfile::tempdir()?;
        let bundle = src.path().join("Test.kext");
        for p in [bundle.clone(), bundle.join("Contents/PlugIns/Plugin.kext")] {
            fs::create_dir_all(p.join("Contents"))?;
            fs::write(
                p.join("Contents/Info.plist"),
                ORIGINAL
                    .replace("Kernel", "CFBundleIdentifier")
                    .replace("<dict/>", "<string>example.test</string>"),
            )?;
        }
        let inv = w.import(&bundle)?;
        assert_eq!(inv["Kexts"].as_array().unwrap().len(), 2);
        assert!(w.import(&bundle).is_err());
        Ok(())
    }
}
