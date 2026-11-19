use anyhow::{Context, Result, bail, ensure};
use plist::Value as P;
use serde_json::{Value, json};
use std::{io::Cursor, process::Command};

pub fn output(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let result = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("Avvio di {program}"))?;
    if !result.status.success() {
        bail!(
            "{} {}",
            String::from_utf8_lossy(&result.stderr).trim(),
            String::from_utf8_lossy(&result.stdout).trim()
        );
    }
    Ok(result.stdout)
}
fn read(program: &str, args: &[&str]) -> String {
    output(program, args)
        .ok()
        .map(|x| String::from_utf8_lossy(&x).trim().to_owned())
        .unwrap_or_default()
}
fn profiler_types() -> Result<Vec<String>> {
    Ok(
        String::from_utf8(output("/usr/sbin/system_profiler", &["-listDataTypes"])?)?
            .lines()
            .map(str::trim)
            .filter(|name| {
                name.starts_with("SP")
                    && name.ends_with("DataType")
                    && name.bytes().all(|b| b.is_ascii_alphanumeric())
            })
            .map(String::from)
            .collect(),
    )
}
pub fn overview() -> Value {
    let sys = |key: &str| read("/usr/sbin/sysctl", &["-n", key]);
    let oc = read(
        "/usr/sbin/nvram",
        &["4D1FDA02-38C7-4A6A-9CC6-4BCCA8B30102:opencore-version"],
    );
    json!({"os":read("/usr/bin/sw_vers",&["-productVersion"]), "build":read("/usr/bin/sw_vers",&["-buildVersion"]),
        "kernel":read("/usr/bin/uname",&["-r"]), "arch":read("/usr/bin/uname",&["-m"]),
        "cpu":sys("machdep.cpu.brand_string"),"ram":sys("hw.memsize").parse::<u64>().ok().map(|v|v/(1<<30)),
        "cores":sys("hw.physicalcpu"),"threads":sys("hw.logicalcpu"),"model":sys("hw.model"),
        "boot":sys("kern.bootargs"),"oc":oc.split_once('\t').map(|v|v.1).unwrap_or("")})
}
fn report(category: &str) -> Result<Value> {
    let data = match category {
        "graphics" | "pci" | "usb" | "storage" | "audio" | "network" | "memory" => {
            let name = match category {
                "graphics" => "SPDisplaysDataType",
                "pci" => "SPPCIDataType",
                "usb" => "SPUSBDataType",
                "storage" => "SPNVMeDataType",
                "audio" => "SPAudioDataType",
                "network" => "SPNetworkDataType",
                _ => "SPMemoryDataType",
            };
            output("/usr/sbin/system_profiler", &[name, "-timeout", "20"])?
        }
        "pciDevices" => {
            return Ok(json!(crate::devices::pci(&P::from_reader(Cursor::new(
                output("/usr/sbin/ioreg", &["-a", "-l", "-p", "IODeviceTree"])?
            ))?)));
        }
        "categories" => return Ok(json!(profiler_types()?)),
        "ioreg" => output("/usr/sbin/ioreg", &["-p", "IOService", "-w", "0"])?,
        "loaded" => output("/usr/bin/kmutil", &["showloaded", "--list-only"])?,
        "power" => output("/usr/bin/pmset", &["-g", "custom"])?,
        name if name.starts_with("SP")
            && name.ends_with("DataType")
            && name.len() < 100
            && name.bytes().all(|b| b.is_ascii_alphanumeric()) =>
        {
            output("/usr/sbin/system_profiler", &[name, "-timeout", "30"])?
        }
        _ => bail!("Categoria sconosciuta"),
    };
    Ok(json!(String::from_utf8_lossy(&data).trim()))
}

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
static REPORTS: LazyLock<Mutex<HashMap<String, Value>>> = LazyLock::new(Default::default);

pub fn cached_report(category: &str, refresh: bool) -> Result<Value> {
    let mut cache = REPORTS.lock().unwrap();
    if let Some(state) = cache.get(category)
        && (!refresh || state["state"] == "loading")
    {
        return Ok(state.clone());
    }
    let key = category.to_owned();
    std::thread::Builder::new()
        .name("hardware-report".into())
        .spawn(move || {
            let state = match report(&key) {
                Ok(value) => json!({"state":"ready","value":value}),
                Err(e) => json!({"state":"error","detail":e.to_string()}),
            };
            REPORTS.lock().unwrap().insert(key, state);
        })?;
    let loading = json!({"state":"loading"});
    cache.insert(category.into(), loading.clone());
    Ok(loading)
}

#[test]
fn background_reports_cache_failures_and_retry() {
    let key = "unknown-test";
    assert_eq!(cached_report(key, false).unwrap()["state"], "loading");
    for _ in 0..100 {
        if cached_report(key, false).unwrap()["state"] == "error" {
            assert_eq!(cached_report(key, true).unwrap()["state"], "loading");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("report worker did not finish");
}

pub fn disks() -> Result<Value> {
    let list = P::from_reader(Cursor::new(output(
        "/usr/sbin/diskutil",
        &["list", "-plist"],
    )?))?;
    let disks = list
        .as_dictionary()
        .and_then(|d| d.get("AllDisksAndPartitions"))
        .and_then(P::as_array)
        .context("Elenco dischi non leggibile")?;
    let mut rows = Vec::new();
    for disk in disks.iter().filter_map(P::as_dictionary) {
        for p in disk
            .get("Partitions")
            .and_then(P::as_array)
            .into_iter()
            .flatten()
            .filter_map(P::as_dictionary)
        {
            let val = |k: &str| p.get(k).and_then(P::as_string).unwrap_or("");
            if val("Content") == "EFI" {
                let parent = disk
                    .get("DeviceIdentifier")
                    .and_then(P::as_string)
                    .unwrap_or("");
                if !disk_id(val("DeviceIdentifier")) || !whole_disk(parent) {
                    continue;
                }
                if !val("DeviceIdentifier")
                    .strip_prefix(parent)
                    .is_some_and(|s| s.starts_with('s'))
                {
                    continue;
                }
                let info = output("/usr/sbin/diskutil", &["info", "-plist", parent])
                    .ok()
                    .and_then(|b| P::from_reader(Cursor::new(b)).ok());
                let meta = |k: &str| {
                    info.as_ref()
                        .and_then(P::as_dictionary)
                        .and_then(|d| d.get(k))
                        .and_then(P::as_string)
                };
                let ejectable = info.as_ref().is_some_and(can_eject);
                rows.push(json!({"ejectable":ejectable,"model":meta("MediaName"),"bus":meta("BusProtocol"),"uuid":val("DiskUUID"),"id":val("DeviceIdentifier"),"name":val("VolumeName"),"mount":val("MountPoint"),
                "size":p.get("Size").and_then(P::as_unsigned_integer),"disk":disk.get("DeviceIdentifier").and_then(P::as_string)}));
            }
        }
    }
    Ok(json!(rows))
}
fn can_eject(info: &P) -> bool {
    let get = |k| {
        info.as_dictionary()
            .and_then(|d| d.get(k))
            .and_then(P::as_boolean)
    };
    get("Internal") == Some(false)
        && (get("Ejectable") == Some(true) || get("RemovableMediaOrExternalDevice") == Some(true))
}
#[test]
fn only_external_media_can_eject() {
    for (internal, expect) in [("true", false), ("false", true)] {
        let p=P::from_reader_xml(format!("<plist><dict><key>Internal</key><{internal}/><key>Ejectable</key><true/></dict></plist>").as_bytes()).unwrap();
        assert_eq!(can_eject(&p), expect);
    }
    assert!(!can_eject(&P::Dictionary(Default::default())));
}
pub fn whole_disk(id: &str) -> bool {
    id.strip_prefix("disk")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}
pub fn disk_id(id: &str) -> bool {
    id.strip_prefix("disk")
        .and_then(|s| s.split_once('s'))
        .is_some_and(|(a, b)| {
            [a, b]
                .iter()
                .all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        })
}
pub fn mount(id: &str, uuid: &str, mount: bool) -> Result<Value> {
    ensure!(
        disk_id(id)
            && !uuid.is_empty()
            && disks()?
                .as_array()
                .is_some_and(|rows| rows.iter().any(|r| r["id"] == id && r["uuid"] == uuid)),
        "Partizione EFI non disponibile"
    );
    let op = if mount { "mount" } else { "unmount" };
    if output("/usr/sbin/diskutil", &[op, id]).is_err() {
        let elevated = output(
            "/usr/bin/osascript",
            &[
                "-e",
                &format!(
                    "do shell script \"/usr/sbin/diskutil {op} {id}\" with administrator privileges"
                ),
            ],
        );
        if let Err(error) = elevated {
            if error.to_string().contains("(-128)") {
                bail!("Autorizzazione annullata")
            }
            return Err(error);
        }
    }
    disks()
}
#[test]
fn partition_identifiers() {
    assert!(disk_id("disk12s3"));
    assert!(whole_disk("disk12"));
    assert!(!whole_disk("disk12s3"));
    assert!(!whole_disk("disk"));
    for s in ["disk0", "disk0s", "disk0s1;", "diskas1", "disk0s1s2"] {
        assert!(!disk_id(s));
    }
}
