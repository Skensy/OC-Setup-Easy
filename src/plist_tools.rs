use crate::diagnostics::{field, string};
use anyhow::{Context, Result, ensure};
use plist::Value as P;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;

fn differences(a: Option<&P>, b: Option<&P>, path: &str, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Some(P::Dictionary(x)), Some(P::Dictionary(y))) => {
            for key in x.keys().chain(y.keys()).collect::<BTreeSet<_>>() {
                differences(x.get(key), y.get(key), &format!("{path}/{key}"), out);
            }
        }
        (Some(P::Array(x)), Some(P::Array(y))) => {
            for i in 0..x.len().max(y.len()) {
                differences(x.get(i), y.get(i), &format!("{path}[{i}]"), out);
            }
        }
        _ => out.push(format!(
            "{} {path}",
            if a.is_none() {
                "+"
            } else if b.is_none() {
                "−"
            } else {
                "~"
            }
        )),
    }
}
pub fn diff(disk: &str, draft: &str) -> Result<Value> {
    let mut paths = Vec::new();
    differences(
        Some(&P::from_reader_xml(disk.as_bytes())?),
        Some(&P::from_reader_xml(draft.as_bytes())?),
        "",
        &mut paths,
    );
    let mut a = NamedTempFile::new()?;
    a.write_all(disk.as_bytes())?;
    let mut b = NamedTempFile::new()?;
    b.write_all(draft.as_bytes())?;
    let res = Command::new("/usr/bin/diff")
        .args(["-u", "-L", "config.plist", "-L", "draft.plist"])
        .arg(a.path())
        .arg(b.path())
        .output()?;
    ensure!(
        matches!(res.status.code(), Some(0 | 1)),
        "XML comparison failed"
    );
    Ok(json!({"paths":paths,"xml":String::from_utf8(res.stdout)?}))
}
fn redact(p: &mut P) {
    if let Some(d) = p.as_dictionary_mut() {
        for (k, v) in d.iter_mut() {
            match k.as_str() {
                "SystemSerialNumber" | "BoardSerialNumber" | "MLB" => *v = P::String(String::new()),
                "SystemUUID" | "SystemUuid" if v.as_data().is_some() => *v = P::Data(vec![0; 16]),
                "SystemUUID" | "SystemUuid" => {
                    *v = P::String("00000000-0000-0000-0000-000000000000".into())
                }
                "ROM" => *v = P::Data(vec![0; 6]),
                _ => redact(v),
            }
        }
    }
}
pub fn export(draft: &str, path: &Path) -> Result<()> {
    let mut tree = P::from_reader_xml(draft.as_bytes())?;
    if let Some(platform) = tree
        .as_dictionary_mut()
        .and_then(|p| p.get_mut("PlatformInfo"))
    {
        redact(platform);
    }
    let mut file = NamedTempFile::new_in(path.parent().context("Invalid destination")?)?;
    tree.to_writer_xml(&mut file)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path)
        .context("Choose a new filename for the exported copy")?;
    Ok(())
}
pub fn resource(name: &str) -> Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .context("App bundle unavailable")?
        .join("../Resources")
        .join(name))
}
fn macserial(args: &[&str]) -> Result<String> {
    let res = Command::new(resource("macserial")?).args(args).output()?;
    ensure!(
        res.status.success(),
        "macserial could not process the selected model"
    );
    Ok(String::from_utf8(res.stdout)?)
}
pub fn smbios(text: &str, generate: bool) -> Result<Value> {
    let p = P::from_reader_xml(text.as_bytes())?;
    let generic = field(&p, "PlatformInfo")
        .and_then(|p| field(p, "Generic"))
        .context("PlatformInfo → Generic is missing")?;
    let model = string(generic, "SystemProductName");
    let list = macserial(&["-l"])?;
    let block = list
        .split("\n\n")
        .find(|block| {
            block.lines().any(|l| {
                l.trim()
                    .strip_prefix("Model:")
                    .is_some_and(|s| s.trim() == model)
            })
        })
        .context("Unknown SMBIOS model")?;
    if generate {
        let pair = macserial(&["-g", "-m", model, "-n", "1"])?;
        let (serial, mlb) = pair
            .trim()
            .split_once('|')
            .context("macserial returned an invalid pair")?;
        let uuid = crate::system::output("/usr/bin/uuidgen", &[])?;
        return Ok(
            json!({"SystemSerialNumber":serial.trim(),"MLB":mlb.trim(),"SystemUUID":String::from_utf8(uuid)?.trim()}),
        );
    }
    let serial = string(generic, "SystemSerialNumber");
    let mlb = string(generic, "MLB");
    let codes = |key: &str| {
        block
            .lines()
            .find_map(|l| l.trim().strip_prefix(key))
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>()
    };
    let base = codes("Base Serial:")[0];
    let characters = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
    };
    let serial_ok = matches!(serial.len(), 11 | 12)
        && serial.len() == base.len()
        && characters(serial)
        && codes("Model codes:")
            .iter()
            .any(|s| !s.is_empty() && serial.ends_with(s))
        && macserial(&["-i", serial])?.contains("Valid: Possibly");
    let range = if base.len() == 11 { 9..12 } else { 11..15 };
    let board_ok = mlb.len() == if base.len() == 11 { 13 } else { 17 }
        && characters(mlb)
        && mlb
            .get(range)
            .is_some_and(|s| codes("Board codes:").contains(&s))
        && (mlb.len() == 13 || macserial(&["--verify", mlb])?.contains("Valid MLB checksum."));
    Ok(json!({"serial":serial_ok,"board":board_ok}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diff_tracks_types_and_array_indices() -> Result<()> {
        let a = "<plist><dict><key>A</key><array><string>1</string></array></dict></plist>";
        let d = diff(
            a,
            &a.replace("<string>1</string>", "<integer>1</integer><true/>"),
        )?;
        assert_eq!(d["paths"], json!(["~ /A[0]", "+ /A[1]"]));
        assert!(d["xml"].as_str().unwrap().contains("+<plist>"));
        Ok(())
    }
    #[test]
    fn export_masks_copy_and_refuses_overwrite() -> Result<()> {
        let text = "<plist><dict><key>PlatformInfo</key><dict><key>Generic</key><dict><key>ROM</key><data>AQIDBAUG</data><key>MLB</key><string>SECRET</string><key>SystemUUID</key><string>UUID</string><key>SystemSerialNumber</key><string>SERIAL</string></dict></dict></dict></plist>";
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("copy.plist");
        export(text, &path)?;
        let p = P::from_file(&path)?;
        let g = field(field(&p, "PlatformInfo").unwrap(), "Generic").unwrap();
        assert_eq!(field(g, "ROM").unwrap().as_data(), Some(&[0; 6][..]));
        assert_eq!(string(g, "MLB"), "");
        assert_eq!(string(g, "SystemSerialNumber"), "");
        assert!(string(g, "SystemUUID").starts_with("0000"));
        assert!(export(text, &path).is_err());
        Ok(())
    }
}
