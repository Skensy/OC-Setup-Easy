use crate::{
    diagnostics::{field, string},
    workspace::Workspace,
};
use anyhow::{Context, Result, ensure};
use plist::Value as P;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

pub fn models(info: &P, bundle: &Path) -> Vec<(String, String)> {
    let known = bundle
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| {
            ["USBMap.kext", "UTBMap.kext"]
                .iter()
                .any(|k| s.eq_ignore_ascii_case(k))
        });
    field(info, "IOKitPersonalities")
        .and_then(P::as_dictionary)
        .into_iter()
        .flatten()
        .filter_map(|(key, p)| {
            let model = field(p, "model")?.as_string()?;
            let usb = ["IOClass", "IOProviderClass"].iter().any(|k| {
                let v = string(p, k).to_ascii_uppercase();
                ["USB", "XHCI", "EHCI"].iter().any(|s| v.contains(s))
            });
            (known || usb).then(|| (key.clone(), model.into()))
        })
        .collect()
}
pub struct Fix {
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
}
impl Fix {
    pub fn preview(w: &Workspace, bundle: &str, xml: &str) -> Result<(Self, Value)> {
        w.assert_unchanged()?;
        let p = P::from_reader_xml(xml.as_bytes())?;
        let model = field(&p, "PlatformInfo")
            .and_then(|p| field(p, "Generic"))
            .map(|p| string(p, "SystemProductName"))
            .filter(|s| !s.is_empty())
            .context("Modello SMBIOS assente")?;
        let rel = Path::new(bundle);
        ensure!(
            rel.components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
                && rel
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("kext")),
            "Destinazione non valida"
        );
        let path = w.root.join("Kexts").join(rel).join("Contents/Info.plist");
        ensure!(path.canonicalize()? == path, "Destinazione non valida");
        let before = fs::read(&path)?;
        let mut info = P::from_reader(Cursor::new(&before))?;
        let changes: Vec<_> = models(&info, rel)
            .into_iter()
            .filter(|(_, old)| old != model)
            .collect();
        ensure!(!changes.is_empty(), "Nessuna modifica");
        let personalities = info
            .as_dictionary_mut()
            .unwrap()
            .get_mut("IOKitPersonalities")
            .unwrap()
            .as_dictionary_mut()
            .unwrap();
        for (key, _) in &changes {
            personalities
                .get_mut(key)
                .unwrap()
                .as_dictionary_mut()
                .unwrap()
                .insert("model".into(), P::String(model.into()));
        }
        let mut after = Vec::new();
        if before.starts_with(b"bplist") {
            info.to_writer_binary(&mut after)?;
        } else {
            info.to_writer_xml(&mut after)?;
        }
        Ok((
            Self {
                path,
                before,
                after,
            },
            json!({"model":model,"changes":changes}),
        ))
    }
    pub fn apply(self, w: &Workspace, dir: &Path) -> Result<PathBuf> {
        w.assert_unchanged()?;
        let _lock = crate::maintenance::write_lock(dir)?;
        let mut backup = tempfile::Builder::new()
            .prefix("USB-Info-")
            .suffix(".plist")
            .tempfile_in(dir)?;
        backup.write_all(&self.before)?;
        backup.as_file().sync_all()?;
        let backup = backup.keep()?.1;
        crate::maintenance::replace(&self.path, &self.before, &self.after)?;
        Ok(backup)
    }
}
#[test]
fn fix_only_modelled_usb_personalities_and_detect_stale_preview() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let xml = "<dict><key>PlatformInfo</key><dict><key>Generic</key><dict><key>SystemProductName</key><string>New</string></dict></dict></dict>";
    fs::write(tmp.path().join("config.plist"), xml)?;
    let contents = tmp.path().join("Kexts/Custom.kext/Contents");
    fs::create_dir_all(&contents)?;
    let path = contents.join("Info.plist");
    let data=b"<dict><key>IOKitPersonalities</key><dict><key>A</key><dict><key>IOProviderClass</key><string>AppleUSBHostController</string><key>model</key><string>Old</string></dict><key>B</key><dict><key>model</key><string>Other</string></dict></dict></dict>";
    fs::write(&path, data)?;
    let w = Workspace::open(tmp.path())?;
    let plan = || Fix::preview(&w, "Custom.kext", xml);
    assert!(Fix::preview(&w, "../Custom.kext", xml).is_err());
    let (fix, summary) = plan()?;
    assert_eq!(summary["changes"], json!([["A", "Old"]]));
    let stale = plan()?.0;
    assert_eq!(fs::read(fix.apply(&w, tmp.path())?)?, data);
    assert!(stale.apply(&w, tmp.path()).is_err());
    assert!(fs::read_to_string(path)?.contains("Other"));
    Ok(())
}
