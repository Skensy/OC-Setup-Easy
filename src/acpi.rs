use crate::workspace::{Workspace, rename_exclusive};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

pub fn header(path: &Path) -> Option<Value> {
    let mut file = fs::File::open(path).ok()?;
    let mut bytes = [0; 36];
    file.read_exact(&mut bytes).ok()?;
    let length = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if u64::from(length) != file.metadata().ok()?.len() {
        return None;
    }
    let text = |s: &[u8]| {
        String::from_utf8_lossy(s)
            .trim_matches([' ', '\0'])
            .to_owned()
    };
    Some(
        json!({"signature":text(&bytes[..4]),"length":length,"revision":bytes[8],"oem":text(&bytes[16..24])}),
    )
}
pub fn compiler() -> Option<PathBuf> {
    let search = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&search)
        .filter(|p| p.is_absolute())
        .map(|p| p.join("iasl"))
        .chain(
            [
                "/opt/homebrew/bin/iasl",
                "/usr/local/bin/iasl",
                "/opt/local/bin/iasl",
                "/Applications/MaciASL.app/Contents/MacOS/iasl-stable",
            ]
            .map(PathBuf::from),
        )
        .find(|p| {
            p.metadata().is_ok_and(|m| {
                m.is_file()
                    && std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o111 != 0
            })
        })
}
pub fn compile(w: &Workspace, source: &Path) -> Result<Value> {
    let tool = compiler().context("Compilatore iasl non disponibile")?;
    w.assert_unchanged()?;
    let source = source.canonicalize()?;
    ensure!(
        source.is_file()
            && source
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("dsl")),
        "Seleziona un file DSL"
    );
    let dir = w.root.join("ACPI");
    if !dir.exists() {
        fs::create_dir(&dir)?
    }
    ensure!(dir.canonicalize()? == dir, "Destinazione non valida");
    let dest = dir
        .join(source.file_name().context("Percorso non valido")?)
        .with_extension("aml");
    ensure!(!dest.exists(), "Tabella AML già presente");
    let tmp = tempfile::tempdir_in(&dir)?;
    let result = Command::new(tool)
        .current_dir(tmp.path())
        .arg("-p")
        .arg("compiled")
        .arg(&source)
        .output()?;
    ensure!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let aml = tmp.path().join("compiled.aml");
    let header = header(&aml).context("Header ACPI non valido")?;
    let entry = json!({"path":dest.file_name().and_then(|v|v.to_str()).context("Percorso non valido")?,
        "bytes":header["length"], "header":header});
    w.assert_unchanged()?;
    ensure!(dir.canonicalize()? == dir, "Destinazione non valida");
    rename_exclusive(&aml, &dest)?;
    Ok(entry)
}

#[test]
fn aml_header_rejects_truncation() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut b = [0; 36];
    b[..4].copy_from_slice(b"SSDT");
    b[4] = 36;
    b[8] = 2;
    b[16..24].copy_from_slice(b"SSDTUSB ");
    fs::write(tmp.path(), b).unwrap();
    assert_eq!(header(tmp.path()).unwrap()["oem"], "SSDTUSB");
    fs::write(tmp.path(), &b[..35]).unwrap();
    assert!(header(tmp.path()).is_none());
}

#[test]
fn import_with_installed_iasl_preserves_existing_aml() -> Result<()> {
    if compiler().is_none() {
        return Ok(());
    }
    let tmp = tempfile::tempdir()?;
    fs::write(tmp.path().join("config.plist"), "<plist><dict/></plist>")?;
    let dsl = tmp.path().join("SSDT-TEST.dsl");
    fs::write(
        &dsl,
        r#"DefinitionBlock ("", "SSDT", 2, "OCQA", "FIXTURE", 1) { Name (TST0, One) }"#,
    )?;
    let w = Workspace::open(tmp.path())?;
    let item = compile(&w, &dsl)?;
    assert_eq!(item["path"], "SSDT-TEST.aml");
    assert_eq!(item["header"]["signature"], "SSDT");
    assert_eq!(item["header"]["oem"], "FIXTURE");
    assert_eq!(item["header"]["revision"], 2);
    assert!(compile(&w, &dsl).is_err());
    Ok(())
}
