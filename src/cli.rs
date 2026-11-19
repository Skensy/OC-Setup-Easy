use crate::{
    diagnostics, graph, plist_tools, system,
    workspace::{self, Workspace},
};
use anyhow::{Context, Result, bail};
use plist::Value as P;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

#[derive(Debug, PartialEq)]
pub enum Action {
    Gui,
    Help,
    Version,
    Check(PathBuf),
    Sort(PathBuf, bool),
    Anonymize(PathBuf, PathBuf),
}
pub fn parse(args: &[OsString]) -> Result<Action> {
    let args: Vec<_> = args
        .iter()
        .filter(|s| !s.to_string_lossy().starts_with("-psn_"))
        .collect();
    let path = |s: &&OsString| !s.is_empty() && !s.to_string_lossy().starts_with("--");
    Ok(match args.as_slice() {
        [] => Action::Gui,
        [s] if *s == "--help" => Action::Help,
        [s] if *s == "--version" => Action::Version,
        [s, p] if *s == "--check" && path(p) => Action::Check(p.into()),
        [s, p] if *s == "--sort-kexts" && path(p) => Action::Sort(p.into(), false),
        [s, p, f] if *s == "--sort-kexts" && *f == "--dry-run" && path(p) => {
            Action::Sort(p.into(), true)
        }
        [s, a, b] if *s == "--anonymize" && path(a) && path(b) => {
            Action::Anonymize(a.into(), b.into())
        }
        _ => bail!("Argomenti non validi: --help"),
    })
}
static MESSAGES: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    let data = include_str!("../ui/messages.js")
        .trim()
        .strip_prefix("const english = ")
        .unwrap()
        .strip_suffix(';')
        .unwrap();
    serde_json::from_str(data).expect("invalid translation catalog")
});
unsafe extern "C" {
    fn oc_english() -> bool;
}
pub fn translate(s: &str) -> String {
    translate_in(s, unsafe { oc_english() })
}
fn translate_in(s: &str, en: bool) -> String {
    if en {
        if let Some(v) = MESSAGES.get(s) {
            return v.clone();
        }
    } else if let Some((k, _)) = MESSAGES.iter().find(|(_, v)| *v == s) {
        return k.clone();
    }
    if let Some((a, b)) = s.split_once(": ") {
        return format!("{}: {}", translate_in(a, en), translate_in(b, en));
    }
    s.into()
}
fn check(path: &Path) -> Result<u8> {
    let path = path.canonicalize()?;
    let bytes = fs::read(&path)?;
    let xml = if bytes.starts_with(b"bplist") {
        workspace::xml(&bytes)?
    } else {
        String::from_utf8(bytes)?
    };
    let validation = workspace::validate(&xml)?;
    println!("{}", validation["log"].as_str().unwrap_or_default());
    if P::from_reader_xml(xml.as_bytes()).is_err() {
        return Ok(1);
    }
    let inv = Workspace::inventory_at(path.parent().context("Destinazione non valida")?)?;
    let mut failed = validation["pass"] != true;
    for issue in diagnostics::lint(&xml, &inv, &system::overview())? {
        let warning = matches!(issue["kind"].as_str(), Some("model" | "pattern"));
        failed |= !warning;
        println!(
            "{} · {} · {} · {}",
            translate(if warning { "Avviso" } else { "Errore" }),
            translate(issue["label"].as_str().unwrap_or_default()),
            issue["path"],
            issue["details"]
        );
    }
    Ok(u8::from(failed))
}
fn sort(path: &Path, dry: bool) -> Result<u8> {
    let mut w = Workspace::open(path)?;
    let mut p = P::from_reader_xml(w.disk_xml()?.as_bytes())?;
    let list = p
        .as_dictionary_mut()
        .and_then(|d| d.get_mut("Kernel"))
        .and_then(P::as_dictionary_mut)
        .and_then(|d| d.get_mut("Add"))
        .and_then(P::as_array_mut)
        .context("Kernel → Add assente o non valido")?;
    let paths: Vec<_> = list
        .iter()
        .map(|p| {
            diagnostics::field(p, "BundlePath")
                .and_then(P::as_string)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .context("BundlePath assente o non valido")
        })
        .collect::<Result<_>>()?;
    let inv = w.inventory()?;
    let result = graph::order(
        &paths,
        inv["Kexts"].as_array().context("Invalid inventory")?,
    );
    for d in result["missing"].as_array().into_iter().flatten() {
        println!("{} · {d}", translate("Dipendenza assente"));
    }
    if result["cycles"].as_array().is_some_and(|a| !a.is_empty()) {
        println!(
            "{} · {}",
            translate("Dipendenza circolare"),
            result["cycles"]
        );
        return Ok(1);
    }
    let indices: Vec<usize> = serde_json::from_value(result["order"].clone())?;
    println!("{}", translate("Ordine proposto"));
    for (i, n) in indices.iter().enumerate() {
        println!("{}  {}", i + 1, paths[*n]);
    }
    if indices.iter().copied().eq(0..paths.len()) {
        println!("{}", translate("Nessuna modifica"));
        return Ok(0);
    }
    if !dry {
        *list = indices.iter().map(|n| list[*n].clone()).collect();
        let mut out = Vec::new();
        p.to_writer_xml(&mut out)?;
        let backup = w.save(std::str::from_utf8(&out)?)?;
        println!("{}{}", translate("Copia precedente in "), backup.display());
    }
    Ok(0)
}
pub fn run(action: Action) -> Result<u8> {
    match action {
        Action::Help => println!(
            "{}: oc-setup-easy\n  --check <config.plist>  {}\n  --sort-kexts <EFI/OC> [--dry-run]  {}\n  --anonymize <input.plist> <output.plist>  {}\n  --help\n  --version",
            translate("Uso"),
            translate("Verifica con ocvalidate e diagnostica locale"),
            translate("Ordina i kext per dipendenze"),
            translate("Esporta una copia anonimizzata")
        ),
        Action::Version => println!("OC Setup Easy {}", env!("CARGO_PKG_VERSION")),
        Action::Check(path) => return check(&path),
        Action::Sort(path, dry) => return sort(&path, dry),
        Action::Anonymize(a, b) => {
            plist_tools::export(&workspace::xml(&fs::read(a)?)?, &b)?;
            println!("{}", b.display());
        }
        Action::Gui => unreachable!(),
    }
    Ok(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &[&str]) -> Result<Action> {
        parse(&s.iter().map(OsString::from).collect::<Vec<_>>())
    }
    #[test]
    fn finder_and_strict_cli_arguments() -> Result<()> {
        assert_eq!(args(&[])?, Action::Gui);
        assert_eq!(args(&["-psn_0_123"])?, Action::Gui);
        assert_eq!(args(&["--check", "a b"])?, Action::Check("a b".into()));
        assert_eq!(
            args(&["--sort-kexts", "EFI", "--dry-run"])?,
            Action::Sort("EFI".into(), true)
        );
        assert_eq!(args(&["--version"])?, Action::Version);
        assert_eq!(args(&["--help"])?, Action::Help);
        assert_eq!(
            args(&["--anonymize", "a", "b"])?,
            Action::Anonymize("a".into(), "b".into())
        );
        for bad in [
            &["--dry-run"][..],
            &["--anonymize", "a"],
            &["--check", "--version"],
            &["--sort-kexts", "EFI", "force"],
        ] {
            assert!(args(bad).is_err());
        }
        Ok(())
    }
    #[test]
    fn dry_run_preserves_binary_config_and_unknown_fields() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("config.plist");
        let original=P::from_reader_xml(b"<dict><key>Kernel</key><dict><key>Add</key><array><dict><key>BundlePath</key><string>C.kext</string><key>Custom</key><integer>7</integer></dict><dict><key>BundlePath</key><string>P.kext</string></dict></array></dict></dict>".as_slice())?;
        original.to_file_binary(&path)?;
        let bytes = fs::read(&path)?;
        for (name, dependencies) in [
            (
                "C",
                "<key>OSBundleLibraries</key><dict><key>P</key><string>1</string></dict>",
            ),
            ("P", ""),
        ] {
            let contents = tmp.path().join(format!("Kexts/{name}.kext/Contents"));
            fs::create_dir_all(&contents)?;
            fs::write(
                contents.join("Info.plist"),
                format!(
                    "<dict><key>CFBundleIdentifier</key><string>{name}</string>{dependencies}</dict>"
                ),
            )?;
        }
        assert_eq!(sort(tmp.path(), true)?, 0);
        assert_eq!(fs::read(&path)?, bytes);
        Ok(())
    }
    #[test]
    fn cli_and_webview_share_both_translations() {
        assert_eq!(
            translate_in("Argomenti non validi: --help", true),
            "Invalid arguments: --help"
        );
        assert_eq!(
            translate_in("Invalid arguments: --help", false),
            "Argomenti non validi: --help"
        );
    }
}
