use crate::{system, workspace::Workspace};
use anyhow::{Context, Result};
use plist::Value as P;
use std::{fs, io::Cursor, path::Path, process::Command};

struct Attached(String);
impl Drop for Attached {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/hdiutil")
            .args(["detach", &self.0])
            .output();
    }
}

#[test]
fn fat32_efi_mount_save_backup_conflict_and_unmount() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let image = temp.path().join("test.cdr");
    let path = image.to_str().unwrap();
    system::output("/usr/bin/python3", &["tests/macos/efi_fixture.py", path])?;
    let attach = P::from_reader(Cursor::new(system::output(
        "/usr/bin/hdiutil",
        &["attach", "-readwrite", "-nomount", "-plist", path],
    )?))?;
    let entities = attach.as_dictionary().unwrap()["system-entities"]
        .as_array()
        .unwrap();
    let device = |hint: &str| {
        entities
            .iter()
            .filter_map(P::as_dictionary)
            .find(|d| d.get("content-hint").and_then(P::as_string) == Some(hint))
            .and_then(|d| d["dev-entry"].as_string())
            .and_then(|p| p.strip_prefix("/dev/"))
            .context("missing fixture device")
    };
    let parent = device("GUID_partition_scheme")?;
    let _guard = Attached(parent.into());
    let id = device("EFI")?;
    let listed = system::disks()?;
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id && r["disk"] == parent)
        .context("EFI not detected")?;
    let uuid = row["uuid"].as_str().context("Missing EFI UUID")?;
    let mounted = system::mount(id, uuid, true)?;
    let mount = mounted
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uuid"] == uuid && r["id"] == id)
        .and_then(|r| r["mount"].as_str())
        .filter(|s| s.starts_with("/Volumes/"))
        .context("Missing mount point")?;
    let oc = Path::new(mount).join("EFI/OC");
    fs::create_dir_all(&oc)?;
    let file = oc.join("config.plist");
    let initial = "<plist><dict/></plist>";
    fs::write(&file, initial)?;
    let mut workspace = Workspace::open(&oc)?;
    let draft = "<plist><dict><key>Boot</key><true/></dict></plist>";
    let backup = workspace.save_in(draft, temp.path())?;
    assert_eq!(fs::read_to_string(backup)?, initial);
    assert_eq!(fs::read_to_string(&file)?, draft);
    fs::write(&file, initial)?;
    assert!(workspace.save_in(draft, temp.path()).is_err());
    assert_eq!(fs::read_to_string(file)?, initial);
    let after = system::mount(id, uuid, false)?;
    assert!(
        after
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == id && r["mount"] == "")
    );
    Ok(())
}
