use crate::diagnostics::{field, string};
use anyhow::{Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use plist::Value as P;
use serde_json::{Value, json};

pub fn convert(kind: &str, input: &str) -> Result<Value> {
    let raw = match kind {
        "Int" => input
            .trim()
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("Intero a 32 bit non valido"))?
            .to_le_bytes()
            .to_vec(),
        "String" => input.as_bytes().to_vec(),
        "Hex" => {
            let text: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
            ensure!(
                text.len().is_multiple_of(2) && text.bytes().all(|c| c.is_ascii_hexdigit()),
                "Esadecimale non valido"
            );
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                .collect()
        }
        "Data" => STANDARD
            .decode(input.split_whitespace().collect::<String>())
            .map_err(|_| anyhow::anyhow!("Base64 non valido"))?,
        _ => bail!("Formato sconosciuto"),
    };
    let int = <[u8; 4]>::try_from(raw.as_slice())
        .ok()
        .map(u32::from_le_bytes);
    Ok(
        json!({"Int":int,"String":std::str::from_utf8(&raw).ok(),"Hex":raw.iter().map(|b|format!("{b:02X}")).collect::<String>(),"Data":STANDARD.encode(&raw)}),
    )
}

fn word(node: &P, key: &str) -> Option<u32> {
    field(node, key)?
        .as_data()?
        .get(..4)?
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
}
pub fn pci(tree: &P) -> Vec<Value> {
    fn visit(n: &P, parent: Option<&str>, rows: &mut Vec<Value>) {
        if let Some(a) = n.as_array() {
            for n in a {
                visit(n, parent, rows);
            }
            return;
        }
        let name = string(n, "IORegistryEntryName");
        let compatible = field(n, "compatible").and_then(P::as_data).unwrap_or(&[]);
        let root = compatible
            .split(|b| *b == 0)
            .any(|id| matches!(id, b"PNP0A03" | b"PNP0A08"));
        let path = if root {
            let uid = match field(n, "_UID") {
                None => Some(0),
                Some(v) => v
                    .as_unsigned_integer()
                    .or_else(|| v.as_string()?.parse().ok())
                    .or_else(|| word(n, "_UID").map(u64::from)),
            };
            uid.map(|id| format!("PciRoot(0x{id:X})"))
        } else if let (Some(parent), Some(vendor), Some(device), Some(reg)) = (
            parent,
            word(n, "vendor-id"),
            word(n, "device-id"),
            word(n, "reg"),
        ) {
            let path = format!(
                "{parent}/Pci(0x{:X},0x{:X})",
                (reg >> 11) & 31,
                (reg >> 8) & 7
            );
            rows.push(json!({"name":name,"vendor":format!("{vendor:04X}"),"device":format!("{device:04X}"),"path":path}));
            Some(path)
        } else {
            None
        };
        if let Some(children) = field(n, "IORegistryEntryChildren") {
            visit(children, path.as_deref(), rows);
        }
    }
    let mut rows = Vec::new();
    visit(tree, None, &mut rows);
    rows
}

#[test]
fn conversion_preserves_bytes_and_checks_input() -> Result<()> {
    let v = convert("Int", "11")?;
    assert_eq!(v["Hex"], "0B000000");
    assert_eq!(v["Data"], "CwAAAA==");
    assert_eq!(convert("Data", "CwAAAA==")?, v);
    assert_eq!(convert("Hex", "c3 a8")?, convert("String", "è")?);
    for (k, v) in [
        ("Int", "-1"),
        ("Int", "4294967296"),
        ("Hex", "A"),
        ("Hex", "ZZ"),
        ("Data", "!"),
    ] {
        assert!(convert(k, v).is_err());
    }
    Ok(())
}
#[test]
fn pci_preserves_root_uid_bridge_device_and_function() {
    let tree=P::from_reader_xml(b"<dict><key>compatible</key><data>UE5QMEEwOAA=</data><key>_UID</key><string>1</string><key>IORegistryEntryChildren</key><array><dict><key>vendor-id</key><data>AhAAAA==</data><key>device-id</key><data>AAAAAA==</data><key>reg</key><data>ABgAAA==</data><key>IORegistryEntryChildren</key><array><dict><key>IORegistryEntryName</key><string>HDEF</string><key>vendor-id</key><data>AhAAAA==</data><key>device-id</key><data>AAAAAA==</data><key>reg</key><data>AAEAAA==</data></dict></array></dict></array></dict>".as_slice()).unwrap();
    let rows = pci(&tree);
    assert_eq!(rows[1]["path"], "PciRoot(0x1)/Pci(0x3,0x0)/Pci(0x0,0x1)");
    assert_eq!(rows[1]["vendor"], "1002");
}
