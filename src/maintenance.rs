use crate::{system, workspace::Workspace};
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};
use tempfile::NamedTempFile;

pub fn logs(work: Option<&Workspace>) -> Result<Value> {
    let mut dirs = BTreeSet::new();
    for disk in system::disks()?.as_array().into_iter().flatten() {
        if let Some(p) = disk["mount"].as_str().filter(|s| !s.is_empty()) {
            dirs.insert(PathBuf::from(p));
        }
    }
    if let Some(w) = work
        && let Some(parent) = w
            .root
            .parent()
            .filter(|p| p.ends_with("EFI"))
            .and_then(Path::parent)
    {
        dirs.insert(parent.to_owned());
    }
    let mut folders: Vec<_> = dirs.into_iter().map(|d| (d, false)).collect();
    folders.push((PathBuf::from("/Library/Logs/DiagnosticReports"), true));
    read_logs(folders)
}
fn read_logs(folders: Vec<(PathBuf, bool)>) -> Result<Value> {
    let mut list = Vec::new();
    for (dir, panic) in folders {
        if !dir.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        for e in fs::read_dir(&dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_lowercase();
            if e.file_type()?.is_file()
                && (if panic {
                    name.ends_with(".panic")
                } else {
                    name.starts_with("opencore-") && name.ends_with(".txt")
                })
            {
                files.push((
                    e.metadata()?.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    e.path(),
                ));
            }
        }
        files.sort_by(|a, b| b.cmp(a));
        for (_, p) in files.into_iter().take(8) {
            if p.canonicalize()? != dir.canonicalize()?.join(p.file_name().unwrap()) {
                continue;
            }
            let mut f = File::open(&p)?;
            let len = f.metadata()?.len();
            let limit = 128 * 1024;
            let mut buf = Vec::new();
            (&mut f).take(limit).read_to_end(&mut buf)?;
            if len > limit {
                if len > limit * 2 {
                    buf.extend_from_slice(b"\n[...]\n");
                    f.seek(SeekFrom::End(-(limit as i64)))?;
                }
                f.take(limit).read_to_end(&mut buf)?;
            }
            let text = String::from_utf8_lossy(&buf);
            let mut trace = false;
            let highlights: Vec<_> = text
                .lines()
                .filter(|l| {
                    let s = l.to_lowercase();
                    if s.contains("extensions in backtrace") {
                        trace = true;
                        return true;
                    }
                    if l.trim().is_empty() {
                        trace = false
                    }
                    [
                        "error",
                        "failed",
                        "panic(",
                        "panicstring",
                        "fatal",
                        "assert",
                        "backtrace",
                        "ocb:",
                        "ocabc:",
                    ]
                    .iter()
                    .any(|t| s.contains(t))
                        || trace && (s.contains("com.") || s.contains("org.") || s.contains("as."))
                })
                .take(100)
                .collect();
            list.push(json!({"path":p,"text":text,"highlights":highlights}));
        }
    }
    Ok(json!(list))
}

struct Change {
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
}
pub struct Plan {
    changes: Vec<Change>,
}
fn x64(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"MZ") {
        return false;
    }
    let Some(offset) = bytes.get(60..64) else {
        return false;
    };
    let n = u32::from_le_bytes(offset.try_into().unwrap()) as usize;
    bytes.get(n..n + 6) == Some(b"PE\0\0\x64\x86")
}
pub(crate) fn efi(w: &Workspace) -> Result<&Path> {
    w.check()?;
    w.root
        .parent()
        .filter(|p| {
            p.file_name().is_some_and(|s| s.eq_ignore_ascii_case("EFI"))
                && w.root
                    .file_name()
                    .is_some_and(|s| s.eq_ignore_ascii_case("OC"))
        })
        .context("Open the full EFI/OC directory to update binaries")
}
fn plan_archive(w: &Workspace, archive: &Path) -> Result<Plan> {
    let root = efi(w)?;
    let zip = archive.to_str().context("Invalid archive path")?;
    let names = String::from_utf8(system::output("/usr/bin/unzip", &["-Z1", zip])?)?;
    let mut paths = vec![
        "BOOT/BOOTx64.efi".to_string(),
        "OC/OpenCore.efi".to_string(),
    ];
    for f in w.inventory()?["Drivers"].as_array().into_iter().flatten() {
        if let Some(path) = f["path"].as_str() {
            paths.push(format!("OC/Drivers/{path}"));
        }
    }
    let mut changes = Vec::new();
    for rel in paths {
        let dest = root.join(&rel);
        let entry = format!("X64/EFI/{rel}");
        let Some(name) = names.lines().find(|s| s.eq_ignore_ascii_case(&entry)) else {
            continue;
        };
        ensure!(
            dest.is_file() && dest.canonicalize()? == dest,
            "Missing or redirected binary: {rel}"
        );
        let before = fs::read(&dest)?;
        let after = system::output("/usr/bin/unzip", &["-p", zip, name])?;
        ensure!(
            x64(&before) && x64(&after),
            "Unsupported EFI architecture: {rel}"
        );
        if rel.eq_ignore_ascii_case("BOOT/BOOTx64.efi") {
            let marker: Vec<_> = "OpenCore.efi"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            ensure!(
                before.windows(marker.len()).any(|s| s == marker),
                "Unrecognized fallback bootloader"
            );
        }
        if before != after {
            changes.push(Change {
                path: dest,
                before,
                after,
            });
        }
    }
    Ok(Plan { changes })
}
const CURL: &[&str] = &[
    "--fail",
    "--silent",
    "--show-error",
    "--location",
    "--proto",
    "=https",
    "--proto-redir",
    "=https",
    "--max-time",
    "120",
];
fn accept_etag(value: &str) -> bool {
    let quoted = value.strip_prefix("W/").unwrap_or(value);
    quoted.len() >= 2
        && quoted.starts_with('"')
        && quoted.ends_with('"')
        && quoted[1..quoted.len() - 1]
            .bytes()
            .all(|c| c == 0x21 || (0x23..=0x7e).contains(&c))
}
fn release_response(
    code: &str,
    headers: &str,
    body: &[u8],
    cached: Option<&Value>,
) -> Result<Value> {
    if code == "304" {
        return cached.cloned().context("Cache GitHub non disponibile");
    }
    ensure!(code == "200", "Risposta GitHub non valida");
    let release: Value = serde_json::from_slice(body)?;
    ensure!(
        release["assets"].is_array() && release["tag_name"].is_string(),
        "Risposta GitHub non valida"
    );
    let final_block = headers.trim_end().rsplit("\r\n\r\n").next().unwrap_or("");
    let etag = final_block
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("etag"))
        .map(|(_, v)| v.trim())
        .filter(|v| accept_etag(v));
    Ok(json!({"etag":etag,"release":release}))
}
pub(crate) fn release(repo: &str) -> Result<Value> {
    let dir = crate::validator::cache()?.join("GitHub");
    fs::create_dir_all(&dir)?;
    let cache = dir.join(format!("{}.json", repo.replace('/', "--")));
    let cached = fs::read(&cache)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(|r| r["release"]["assets"].is_array() && r["release"]["tag_name"].is_string());
    let body = NamedTempFile::new()?;
    let headers = NamedTempFile::new()?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut args = CURL.to_vec();
    let conditional;
    if let Some(etag) = cached
        .as_ref()
        .and_then(|r| r["etag"].as_str())
        .filter(|v| accept_etag(v))
    {
        conditional = format!("If-None-Match: {etag}");
        args.extend(["-H", &conditional]);
    }
    args.extend([
        "--max-filesize",
        "1048576",
        "-D",
        headers.path().to_str().context("Percorso non valido")?,
        "-o",
        body.path().to_str().context("Percorso non valido")?,
        "-w",
        "%{http_code}",
        &url,
    ]);
    let code = String::from_utf8(system::output("/usr/bin/curl", &args)?)?;
    let response = release_response(
        &code,
        &fs::read_to_string(headers.path())?,
        &fs::read(body.path())?,
        cached.as_ref(),
    )?;
    if code == "200" {
        let mut tmp = NamedTempFile::new_in(dir)?;
        serde_json::to_writer(&mut tmp, &response)?;
        tmp.persist(cache)?;
    }
    Ok(response["release"].clone())
}

#[test]
fn etag_cache_uses_final_response_and_handles_304() -> Result<()> {
    let headers = "HTTP/1.1 200 Connection established\r\nETag: \"wrong\"\r\n\r\nHTTP/2 200\r\netag: W/\"abc\"\r\n";
    let r = release_response("200", headers, br#"{"assets":[],"tag_name":"test"}"#, None)?;
    assert_eq!(r["etag"], "W/\"abc\"");
    assert_eq!(release_response("304", "", b"", Some(&r))?, r);
    assert!(release_response("304", "", b"", None).is_err());
    assert!(!accept_etag("\"a\r\nb\""));
    Ok(())
}
pub fn check_update(w: &mut Workspace) -> Result<Value> {
    w.update = None;
    w.sample = None;
    efi(w)?;
    let release = release("acidanthera/OpenCorePkg")?;
    let tag = release["tag_name"]
        .as_str()
        .context("Missing release version")?;
    ensure!(
        tag.split('.').count() == 3 && tag.split('.').all(|s| s.parse::<u16>().is_ok()),
        "Unexpected release version"
    );
    let name = format!("OpenCore-{tag}-RELEASE.zip");
    let asset = release["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"] == name))
        .context("Release archive missing")?;
    let digest = asset["digest"]
        .as_str()
        .and_then(|s| s.strip_prefix("sha256:"))
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .context("Missing release SHA-256")?;
    let url = format!("https://github.com/acidanthera/OpenCorePkg/releases/download/{tag}/{name}");
    let archive = download(&url, digest)?;
    crate::validator::install(archive.path(), tag)?;
    let sample = system::output(
        "/usr/bin/unzip",
        &[
            "-p",
            archive.path().to_str().context("Invalid archive path")?,
            "Docs/Sample.plist",
        ],
    )?;
    w.sample = Some(plist::Value::from_reader(std::io::Cursor::new(sample))?);
    let plan = plan_archive(w, archive.path())?;
    let root = efi(w)?;
    let summary = json!({"version":tag,"digest":digest,"paths":plan.changes.iter().map(|c|c.path.strip_prefix(root).unwrap()).collect::<Vec<_>>()});
    w.update = Some(plan);
    Ok(summary)
}
pub(crate) fn download(url: &str, digest: &str) -> Result<NamedTempFile> {
    let archive = NamedTempFile::new()?;
    let file = archive.path().to_str().context("Invalid archive path")?;
    let mut args = CURL.to_vec();
    args.extend(["--max-filesize", "134217728", "--output", file, url]);
    system::output("/usr/bin/curl", &args)?;
    let actual = String::from_utf8(system::output("/usr/bin/shasum", &["-a", "256", file])?)?;
    ensure!(
        actual.split_whitespace().next() == Some(digest),
        "SHA-256 mismatch"
    );
    Ok(archive)
}
pub(crate) fn replace(path: &Path, expected: &[u8], data: &[u8]) -> Result<()> {
    ensure!(
        path.canonicalize()? == path && fs::read(path)? == expected,
        "Binary changed since preview: {}",
        path.display()
    );
    let mut tmp = NamedTempFile::new_in(path.parent().context("Invalid destination")?)?;
    tmp.write_all(data)?;
    tmp.as_file()
        .set_permissions(path.metadata()?.permissions())?;
    tmp.as_file().sync_all()?;
    ensure!(
        path.canonicalize()? == path && fs::read(path)? == expected,
        "Binary changed during update"
    );
    tmp.persist(path)?;
    Ok(())
}
pub(crate) fn write_lock(dir: &Path) -> Result<File> {
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(".save.lock"))?;
    lock.try_lock_exclusive()
        .context("Another write is in progress")?;
    Ok(lock)
}
pub fn apply_update(w: &mut Workspace) -> Result<Value> {
    efi(w)?;
    let plan = w
        .update
        .take()
        .context("Check the release again before applying")?;
    ensure!(!plan.changes.is_empty(), "No binary changes");
    let _lock = write_lock(&crate::workspace::backups()?)?;
    for c in &plan.changes {
        ensure!(
            c.path.canonicalize()? == c.path && fs::read(&c.path)? == c.before,
            "Binary changed since preview"
        );
    }
    let backup = w.backup()?;
    system::output(
        "/usr/bin/unzip",
        &["-tqq", backup.to_str().context("Invalid backup path")?],
    )?;
    write_changes(w, &plan.changes, &backup)?;
    Ok(json!(backup))
}
fn write_changes(w: &Workspace, changes: &[Change], backup: &Path) -> Result<()> {
    for (i, c) in changes.iter().enumerate() {
        if let Err(e) = w
            .check()
            .and_then(|_| replace(&c.path, &c.before, &c.after))
        {
            let mut failures = Vec::new();
            for done in changes[..i].iter().rev() {
                if let Err(err) = w
                    .check()
                    .and_then(|_| replace(&done.path, &done.after, &done.before))
                {
                    failures.push(err.to_string());
                }
            }
            bail!(
                "{e}. Backup: {}. Rollback errors: {failures:?}",
                backup.display()
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_replace_refuses_external_edits() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let p = dir.path().canonicalize()?.join("OpenCore.efi");
        fs::write(&p, b"old")?;
        assert!(replace(&p, b"other", b"new").is_err());
        assert_eq!(fs::read(&p)?, b"old");
        replace(&p, b"old", b"new")?;
        assert_eq!(fs::read(&p)?, b"new");
        Ok(())
    }
    #[test]
    fn interrupted_batch_restores_earlier_binaries() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        fs::write(root.join("config.plist"), "<plist><dict/></plist>")?;
        let w = Workspace::open(&root)?;
        let changes: Vec<_> = ["first", "second"]
            .into_iter()
            .map(|name| Change {
                path: root.join(name),
                before: b"old".to_vec(),
                after: b"new".to_vec(),
            })
            .collect();
        fs::write(&changes[0].path, b"old")?;
        fs::write(&changes[1].path, b"external")?;
        assert!(write_changes(&w, &changes, &root.join("backup.zip")).is_err());
        assert_eq!(fs::read(&changes[0].path)?, b"old");
        assert_eq!(fs::read(&changes[1].path)?, b"external");
        Ok(())
    }
    #[test]
    fn panic_trace_and_boot_log_are_selected() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let p = dir.path();
        fs::write(
            p.join("a.panic"),
            "Kernel Extensions in backtrace:\n com.example.driver(1.0)\n\nmore info",
        )?;
        fs::write(p.join("opencore-1.txt"), "OK\nOCB: failed to load")?;
        fs::write(p.join("unrelated.txt"), "unrelated")?;
        let logs = read_logs(vec![(p.into(), true), (p.into(), false)])?;
        assert_eq!(logs.as_array().unwrap().len(), 2);
        assert_eq!(logs[0]["highlights"][1], " com.example.driver(1.0)");
        assert_eq!(logs[1]["highlights"][0], "OCB: failed to load");
        Ok(())
    }
    #[test]
    fn pe_architecture_bounds() {
        let mut p = vec![0; 100];
        assert!(!x64(&p));
        p[..2].copy_from_slice(b"MZ");
        p[60] = 80;
        p[80..86].copy_from_slice(b"PE\0\0\x64\x86");
        assert!(x64(&p));
        p[60] = 250;
        assert!(!x64(&p));
    }
}
