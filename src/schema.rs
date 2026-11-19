use plist::Value as P;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn display(p: &P) -> String {
    if let P::String(s) = p {
        return s.clone();
    }
    let mut data = Vec::new();
    if p.to_writer_xml(&mut data).is_err() {
        return format!("{p:?}");
    }
    let xml = String::from_utf8_lossy(&data);
    xml.split_once("<plist version=\"1.0\">")
        .and_then(|(_, s)| s.split_once("</plist>"))
        .map(|(v, _)| v.trim().to_owned())
        .unwrap_or_default()
}

pub fn compare(sample: &P, draft: &P) -> Value {
    fn visit(templates: &[&P], draft: &P, path: &str, out: &mut Vec<Value>) {
        match draft {
            P::Dictionary(d) => {
                let dicts: Vec<_> = templates.iter().filter_map(|p| p.as_dictionary()).collect();
                let keys: BTreeSet<_> = dicts
                    .iter()
                    .flat_map(|d| d.keys())
                    .chain(d.keys())
                    .collect();
                for k in keys {
                    let next: Vec<_> = dicts.iter().filter_map(|d| d.get(k)).collect();
                    let path = format!("{path}/{}", k.replace('~', "~0").replace('/', "~1"));
                    match (next.first(), d.get(k)) {
                        (Some(p), None) => {
                            out.push(json!({"kind":"missing","path":path,"default":display(p)}))
                        }
                        (None, Some(_)) => out.push(json!({"kind":"extra","path":path})),
                        (_, Some(p)) => visit(&next, p, &path, out),
                        _ => (),
                    }
                }
            }
            P::Array(rows) => {
                let examples: Vec<_> = templates
                    .iter()
                    .filter_map(|p| p.as_array())
                    .flatten()
                    .collect();
                for (i, row) in rows.iter().enumerate() {
                    visit(&examples, row, &format!("{path}/{i}"), out);
                }
            }
            _ => {
                if templates.iter().any(|p| p.as_dictionary().is_some()) {
                    visit(templates, &P::Dictionary(Default::default()), path, out);
                }
            }
        }
    }
    let mut rows = Vec::new();
    visit(&[sample], draft, "", &mut rows);
    json!(rows)
}
#[test]
fn compares_keys_and_array_templates_without_comparing_configured_values() {
    let sample=P::from_reader_xml(b"<dict><key>A</key><array><dict><key>Enabled</key><true/></dict></array><key>B</key><integer>11</integer></dict>".as_slice()).unwrap();
    let draft=P::from_reader_xml(b"<dict><key>A</key><array><dict><key>Enabled</key><false/></dict><dict><key>X/Y</key><true/></dict></array></dict>".as_slice()).unwrap();
    let r = compare(&sample, &draft);
    assert_eq!(r.as_array().unwrap().len(), 3);
    assert_eq!(r[0]["path"], "/A/1/Enabled");
    assert!(r[0]["default"].as_str().unwrap().contains("true"));
    assert_eq!(r[1]["path"], "/A/1/X~1Y");
    assert_eq!(r[2]["path"], "/B");
}
