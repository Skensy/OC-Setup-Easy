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
        "close" => {
            *session = None;
            Ok(Value::Null)
        }
        op => {
            let w = session
                .as_mut()
                .context("Apri prima una cartella OpenCore.")?;
            match op {
                "save" => Ok(json!(w.save(field("xml")?)?)),
                "validate" => workspace::validate(field("xml")?),
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
