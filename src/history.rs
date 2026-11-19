use plist::Value;
use serde_json::{Value as Json, json};
use std::collections::VecDeque;

// Keep the exact source alongside the tree so undo also restores formatting,
// comments and incomplete XML while typing.
struct Snapshot {
    source: String,
    tree: Option<Value>,
}
impl Snapshot {
    fn new(source: String) -> Self {
        let tree = Value::from_reader_xml(source.as_bytes()).ok();
        Self { source, tree }
    }
}
pub struct History {
    current: Snapshot,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
}
impl History {
    pub fn new(source: String) -> Self {
        Self {
            current: Snapshot::new(source),
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }
    pub fn record(&mut self, source: &str) -> Json {
        if source != self.current.source {
            if self.undo.len() == 50 {
                self.undo.pop_front();
            }
            self.undo.push_back(std::mem::replace(
                &mut self.current,
                Snapshot::new(source.into()),
            ));
            self.redo.clear();
        }
        self.status()
    }
    fn status(&self) -> Json {
        json!({"undo":!self.undo.is_empty(),"redo":!self.redo.is_empty()})
    }
    pub fn step(&mut self, redo: bool) -> Json {
        if redo {
            if let Some(s) = self.redo.pop() {
                self.undo.push_back(std::mem::replace(&mut self.current, s));
            }
        } else if let Some(s) = self.undo.pop_back() {
            self.redo.push(std::mem::replace(&mut self.current, s));
        }
        let mut r = self.status();
        r["xml"] = json!(self.current.source);
        r["fields"] = json!(
            self.current
                .tree
                .as_ref()
                .is_some_and(|p| p.as_dictionary().is_some())
        );
        r
    }
}
#[test]
fn bounded_history_branches_and_restores_exact_source() {
    let xml = "<plist><dict/></plist>";
    let mut h = History::new(xml.into());
    h.record("<");
    assert_eq!(h.step(false)["xml"], xml);
    assert_eq!(h.step(true)["fields"], false);
    h.step(false);
    assert_eq!(h.record("new")["redo"], false);
    for n in 0..60 {
        h.record(&n.to_string());
    }
    for _ in 0..50 {
        h.step(false);
    }
    assert_eq!(h.step(false)["xml"], "9");
}
