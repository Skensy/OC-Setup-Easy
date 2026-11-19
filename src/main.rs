mod diagnostics;
mod graph;
mod maintenance;
mod plist_tools;
mod system;
mod workspace;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    ffi::{CStr, CString, c_char},
    path::Path,
    sync::Mutex,
};
use workspace::Workspace;

unsafe extern "C" {
    fn run_window();
}
static SESSION: Mutex<Option<Workspace>> = Mutex::new(None);

fn dispatch(req: &Value) -> Result<Value> {
    let field = |name: &str| {
        req[name]
            .as_str()
            .with_context(|| format!("Parametro mancante: {name}"))
    };
    let mut session = SESSION
        .lock()
        .map_err(|_| anyhow::anyhow!("Sessione interrotta. Riapri l’app."))?;
    match field("op")? {
        "logs" => maintenance::logs(session.as_ref()),
        "system" => Ok(system::overview()),
        "disks" => system::disks(),
        "report" => system::report(field("category")?),
        "mount" | "unmount" => {
            let id = field("id")?;
            if field("op")? == "unmount" {
                let disks = system::disks()?;
                if let Some(point) = disks
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|p| p["id"] == id)
                    .and_then(|p| p["mount"].as_str())
                    .filter(|s| !s.is_empty())
                {
                    ensure!(
                        !session.as_ref().is_some_and(|s| s.root.starts_with(point)),
                        "Chiudi la configurazione di questo volume prima di smontarlo."
                    );
                }
            }
            system::mount(id, field("uuid")?, field("op")? == "mount")
        }
        "open" => {
            let w = Workspace::open(Path::new(field("path")?))?;
            let data = w.view()?;
            *session = Some(w);
            Ok(data)
        }
        "finish" => {
            let id = field("id")?;
            let uuid = field("uuid")?;
            let w = session
                .as_ref()
                .context("Open the mounted configuration first")?;
            w.assert_unchanged()?;
            let list = system::disks()?;
            let disk = list
                .as_array()
                .and_then(|a| a.iter().find(|d| d["id"] == id && d["uuid"] == uuid))
                .context("EFI partition unavailable")?;
            ensure!(
                disk["mount"]
                    .as_str()
                    .is_some_and(|m| !m.is_empty() && w.root.starts_with(m)),
                "The selected EFI does not own this configuration"
            );
            let after = system::mount(id, uuid, false)?;
            *session = None;
            let mut warning = None;
            if disk["ejectable"] == true
                && let Some(parent) = disk["disk"].as_str()
                && after.as_array().is_some_and(|a| {
                    a.iter().any(|d| {
                        d["id"] == id
                            && d["uuid"] == uuid
                            && d["disk"] == parent
                            && d["ejectable"] == true
                    })
                })
                && let Err(e) = system::output("/usr/sbin/diskutil", &["eject", parent])
            {
                warning = Some(e.to_string());
            }
            Ok(json!({"disks":system::disks().unwrap_or(after),"warning":warning}))
        }
        "close" => {
            *session = None;
            Ok(Value::Null)
        }
        op => {
            let w = session
                .as_mut()
                .context("Apri prima una cartella OpenCore.")?;
            match op {
                "checkUpdate" => maintenance::check_update(w),
                "applyUpdate" => maintenance::apply_update(w),
                "diff" => plist_tools::diff(&w.disk_xml()?, field("xml")?),
                "export" => {
                    plist_tools::export(field("xml")?, Path::new(field("path")?))?;
                    Ok(json!(field("path")?))
                }
                "smbios" | "generate" => plist_tools::smbios(field("xml")?, op == "generate"),
                "save" => Ok(json!(w.save(field("xml")?)?)),
                "validate" => {
                    let text = field("xml")?;
                    let mut result = workspace::validate(text)?;
                    result["issues"] =
                        json!(if plist::Value::from_reader_xml(text.as_bytes()).is_ok() {
                            diagnostics::lint(text, &w.inventory()?, &system::overview())?
                        } else {
                            Vec::new()
                        });
                    Ok(result)
                }
                "order" => {
                    let paths: Vec<String> = serde_json::from_value(req["paths"].clone())?;
                    let inv = w.inventory()?;
                    Ok(graph::order(
                        &paths,
                        inv["Kexts"].as_array().context("Invalid inventory")?,
                    ))
                }
                "inventory" => w.inventory(),
                "import" => w.import(Path::new(field("path")?)),
                "backup" => Ok(json!(w.backup()?)),
                "finder" | "backups" => {
                    let path = if op == "backups" {
                        workspace::backups()?
                    } else {
                        w.root.clone()
                    };
                    system::output(
                        "/usr/bin/open",
                        &[path.to_str().context("Percorso non valido")?],
                    )?;
                    Ok(Value::Null)
                }
                _ => bail!("Comando sconosciuto"),
            }
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn oc_request(input: *const c_char) -> *mut c_char {
    let response = std::panic::catch_unwind(|| -> Result<Value> {
        let req = serde_json::from_slice(unsafe { CStr::from_ptr(input) }.to_bytes())?;
        dispatch(&req)
    });
    let envelope = match response {
        Ok(Ok(value)) => json!({"ok":value}),
        Ok(Err(error)) => json!({"error":format!("{error:#}")}),
        Err(_) => json!({"error":"Errore interno. Riapri la configurazione."}),
    };
    CString::new(envelope.to_string()).unwrap().into_raw()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn oc_free(ptr: *mut c_char) {
    drop(unsafe { CString::from_raw(ptr) });
}

fn main() {
    unsafe { run_window() }
}
