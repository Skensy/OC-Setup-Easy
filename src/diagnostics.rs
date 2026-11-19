use anyhow::Result;
use plist::Value as P;
use serde_json::{Value, json};
use std::collections::HashMap;
pub fn field<'a>(p: &'a P, k: &str) -> Option<&'a P> {
    p.as_dictionary()?.get(k)
}
pub fn string<'a>(p: &'a P, k: &str) -> &'a str {
    field(p, k).and_then(P::as_string).unwrap_or("")
}
fn version(v: &str) -> [u32; 3] {
    let mut out = [0; 3];
    for (n, s) in out.iter_mut().zip(v.split('.')) {
        *n = s.parse().unwrap_or(0);
    }
    out
}
fn active(p: &P, host: &Value) -> bool {
    let kernel = host["kernel"].as_str().unwrap_or("");
    field(p, "Enabled").and_then(P::as_boolean) == Some(true)
        && (string(p, "MinKernel").is_empty() || version(kernel) >= version(string(p, "MinKernel")))
        && (string(p, "MaxKernel").is_empty() || version(kernel) <= version(string(p, "MaxKernel")))
        && (matches!(string(p, "Arch"), "" | "Any")
            || Some(string(p, "Arch")) == host["arch"].as_str())
}
pub fn lint(text: &str, inventory: &Value, host: &Value) -> Result<Vec<Value>> {
    let root = P::from_reader_xml(text.as_bytes())?;
    let mut findings = Vec::new();
    let mut issue = |kind: &str, path: Value, details: Value| {
        let label = match kind {
            "cores" => "Core fisici diversi dal byte Replace",
            "pattern" => "Formato patch non riconosciuto",
            "conflict" => "Conflitto",
            "duplicate" => "Bundle duplicato",
            "order" => "Caricato prima della dipendenza",
            "model" => "Modello configurato e modello del Mac",
            "usbModel" => "Modello SMBIOS diverso nella mappa USB",
            _ => kind,
        };
        findings.push(json!({"kind":kind,"label":label,"path":path,"details":details}))
    };
    let array = |k| {
        field(&root, "Kernel")
            .and_then(|p| field(p, k))
            .and_then(P::as_array)
            .into_iter()
            .flatten()
            .enumerate()
    };
    if host["cpu"]
        .as_str()
        .unwrap_or("")
        .to_uppercase()
        .contains("AMD")
        && let Some(cores) = host["cores"]
            .as_str()
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|n| *n > 0)
    {
        for (i, p) in array("Patch") {
            if field(p, "Enabled").and_then(P::as_boolean) != Some(true) {
                continue;
            }
            let find = field(p, "Find").and_then(P::as_data).unwrap_or(&[]);
            if !(string(p, "Comment")
                .to_lowercase()
                .contains("cpuid_cores_per_package")
                || string(p, "Base") == "_cpuid_set_info"
                    && matches!(find, [0xc1, 0xe8, 0x1a, 0, 0] | [0xc1, 0xe8, 0x1a, 0, 0, 0]))
            {
                continue;
            }
            let replace = field(p, "Replace").and_then(P::as_data).unwrap_or(&[]);
            let path = json!(["Kernel", "Patch", i, "Replace"]);
            match replace {
                [0xb8 | 0xba, count, 0, 0, 0] | [0xb8 | 0xba, count, 0, 0, 0, 0 | 0x90]
                    if *count != cores =>
                {
                    issue("cores", path, json!([count, cores]))
                }
                [0xb8 | 0xba, _, 0, 0, 0] | [0xb8 | 0xba, _, 0, 0, 0, 0 | 0x90] => (),
                _ => issue("pattern", path, json!([])),
            }
        }
    }
    let files = inventory["Kexts"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let enabled: Vec<_> = array("Add")
        .filter(|(_, p)| active(p, host))
        .map(|(i, p)| {
            let path = string(p, "BundlePath");
            let file = files.iter().find(|f| {
                f["path"]
                    .as_str()
                    .is_some_and(|s| crate::graph::normalized(s) == crate::graph::normalized(path))
            });
            (i, path, file)
        })
        .collect();
    let mut ids = HashMap::new();
    for (i, path, file) in &enabled {
        let target = json!(["Kernel", "Add", i, "BundlePath"]);
        let name = path.rsplit('/').next().unwrap_or(path);
        let id = file.and_then(|f| f["id"].as_str()).unwrap_or(name);
        if let Some(prev) = ids.insert(id, *path) {
            issue("duplicate", target.clone(), json!([prev, path]));
        }
        if let Some(other) = match name {
            "SMCBatteryManager.kext" => Some("ACPIBatteryManager.kext"),
            "AirportItlwm.kext" => Some("itlwm.kext"),
            _ => None,
        } && enabled
            .iter()
            .any(|(_, p, _)| p.rsplit('/').next() == Some(other))
        {
            issue("conflict", target.clone(), json!([path, other]));
        }
        for dep in file
            .and_then(|f| f["dependencies"].as_array())
            .into_iter()
            .flatten()
        {
            if let Some((j, provider, _)) = enabled
                .iter()
                .find(|(_, _, f)| f.is_some_and(|f| &f["id"] == dep))
                && j > i
            {
                issue("order", target.clone(), json!([path, provider]));
            }
        }
    }
    let mut drivers = Vec::new();
    for (i, p) in field(&root, "UEFI")
        .and_then(|v| field(v, "Drivers"))
        .and_then(P::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if field(p, "Enabled").and_then(P::as_boolean) == Some(true) || p.as_string().is_some() {
            drivers.push((i, p.as_string().unwrap_or_else(|| string(p, "Path"))));
        }
    }
    for (a, b) in [
        ("HfsPlus.efi", "OpenHfsPlus.efi"),
        ("VBoxHfs.efi", "HfsPlus.efi"),
        ("VBoxHfs.efi", "OpenHfsPlus.efi"),
    ] {
        if let Some((i, _)) = drivers
            .iter()
            .find(|(_, p)| p.rsplit('/').next() == Some(a))
            && drivers.iter().any(|(_, p)| p.rsplit('/').next() == Some(b))
        {
            issue("conflict", json!(["UEFI", "Drivers", i]), json!([a, b]));
        }
    }
    let model = field(&root, "PlatformInfo")
        .and_then(|p| field(p, "Generic"))
        .map(|p| string(p, "SystemProductName"))
        .unwrap_or("");
    if !model.is_empty() {
        for file in files {
            for pair in file["usbModels"].as_array().into_iter().flatten() {
                if pair[1] != model {
                    issue(
                        "usbModel",
                        json!(["PlatformInfo", "Generic", "SystemProductName"]),
                        json!([file["path"], pair[0], pair[1], model]),
                    );
                }
            }
        }
    }
    if !model.is_empty()
        && host["model"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s != model)
    {
        issue(
            "model",
            json!(["PlatformInfo", "Generic", "SystemProductName"]),
            json!([model, host["model"]]),
        );
    }
    Ok(findings)
}
#[test]
fn amd_count_and_conflicts() {
    let xml = "<plist><dict><key>Kernel</key><dict><key>Patch</key><array><dict><key>Enabled</key><true/><key>Comment</key><string>cpuid_cores_per_package</string><key>Replace</key><data>ugQAAAA=</data></dict></array><key>Add</key><array><dict><key>Enabled</key><true/><key>BundlePath</key><string>itlwm.kext</string></dict><dict><key>Enabled</key><true/><key>BundlePath</key><string>AirportItlwm.kext</string></dict></array></dict></dict></plist>";
    let mut host = json!({"cpu":"AMD Ryzen","cores":"6","kernel":"25.0.0"});
    let findings = lint(xml, &Value::Null, &host).unwrap();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["details"], json!([4, 6]));
    host["cpu"] = json!("Intel");
    assert_eq!(lint(xml, &Value::Null, &host).unwrap().len(), 1);
}

#[test]
fn disabled_and_out_of_range_kexts_do_not_conflict() {
    let host = json!({"kernel":"25.1.0","arch":"x86_64"});
    for (extra, expected) in [
        ("", true),
        ("<key>MaxKernel</key><string>24.99.99</string>", false),
        ("<key>MinKernel</key><string>26.0.0</string>", false),
        ("<key>Arch</key><string>i386</string>", false),
    ] {
        let node = P::from_reader_xml(
            format!("<plist><dict><key>Enabled</key><true/>{extra}</dict></plist>").as_bytes(),
        )
        .unwrap();
        assert_eq!(active(&node, &host), expected);
    }
    assert!(!active(&P::Dictionary(Default::default()), &host));
}

const APPLE: &str = "7C436110-AB2A-4BBB-A880-FE41995C9F82";
const OC: &str = "4D1FDA02-38C7-4A6A-9CC6-4BCCA8B30102";

fn compare_nvram(config: &P, nvram: &P) -> Vec<Value> {
    fn display(value: &P, binary: bool) -> Option<String> {
        if let Some(bytes) = value.as_data() {
            if binary {
                return Some(bytes.iter().map(|b| format!("{b:02X}")).collect());
            }
            return Some(String::from_utf8_lossy(bytes).trim_end_matches('\0').into());
        }
        value.as_string().map(str::to_owned)
    }
    let dict = field(config, "NVRAM")
        .and_then(|p| field(p, "Add"))
        .and_then(P::as_dictionary);
    [(APPLE,"boot-args"),(APPLE,"csr-active-config"),(OC,"opencore-version")].iter().map(|&(guid,key)| {
        let config = dict.and_then(|d|d.iter().find(|(k,_)|k.eq_ignore_ascii_case(guid)))
            .and_then(|(_,p)|field(p,key)).and_then(|p|display(p,key=="csr-active-config"));
        let scoped = format!("{guid}:{key}");
        let live = nvram.as_dictionary().and_then(|d|d.iter().find(|(k,_)|k.eq_ignore_ascii_case(&scoped)).map(|(_,v)|v)
            .or_else(||(guid==APPLE).then(||d.get(key)).flatten())).and_then(|v|display(v,key=="csr-active-config"));
        json!({"key":key,"guid":guid,"different":config.is_some() && live.is_some() && config!=live,"config":config,"live":live})
    }).collect()
}
pub fn live(w: &crate::workspace::Workspace) -> Result<Value> {
    let config = P::from_reader_xml(w.disk_xml()?.as_bytes())?;
    let nvram =
        P::from_reader_xml(crate::system::output("/usr/sbin/nvram", &["-x", "-p"])?.as_slice())?;
    Ok(json!(compare_nvram(&config, &nvram)))
}

#[test]
fn nvram_scopes_and_binary_values() {
    let config=P::from_reader_xml(format!("<plist><dict><key>NVRAM</key><dict><key>Add</key><dict><key>{APPLE}</key><dict><key>boot-args</key><string>-v</string><key>csr-active-config</key><data>AwgAAA==</data></dict></dict></dict></dict></plist>").as_bytes()).unwrap();
    let live=P::from_reader_xml(b"<plist><dict><key>boot-args</key><data>LXY=</data><key>csr-active-config</key><data>AAAAAA==</data><key>opencore-version</key><string>foreign</string></dict></plist>".as_slice()).unwrap();
    let rows = compare_nvram(&config, &live);
    assert_eq!(rows[0]["different"], false);
    assert_eq!(rows[1]["different"], true);
    assert_eq!(rows[1]["config"], "03080000");
    assert!(rows[2]["live"].is_null());
}
