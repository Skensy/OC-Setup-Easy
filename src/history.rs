use serde_json::{Value, json};
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone)]
enum State {
    Xml(Arc<str>),
    Field(Value),
}
struct Edit {
    before: State,
    after: State,
}

pub struct History {
    current: Option<Arc<str>>,
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
}
impl History {
    pub fn new(source: String) -> Self {
        Self {
            current: Some(source.into()),
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }
    fn push(&mut self, before: State, after: State) -> Value {
        if self.undo.len() == 50 {
            self.undo.pop_front();
        }
        self.undo.push_back(Edit { before, after });
        self.redo.clear();
        self.status()
    }
    pub fn record(&mut self, source: &str) -> Value {
        if self.current.as_deref() == Some(source) {
            return self.status();
        }
        let next: Arc<str> = source.into();
        let prev = self
            .current
            .replace(next.clone())
            .expect("snapshot required before structural edit");
        self.push(State::Xml(prev), State::Xml(next))
    }
    pub fn splice(&mut self, offset: usize, old: &str, next: &str) -> anyhow::Result<Value> {
        use anyhow::{Context, ensure};
        let source = self.current.as_deref().context("Snapshot mancante")?;
        let mut units = 0;
        let start = source
            .char_indices()
            .find_map(|(byte, ch)| {
                let pos = units;
                units += ch.len_utf16();
                (pos == offset).then_some(byte)
            })
            .or_else(|| (units == offset).then_some(source.len()))
            .context("Posizione XML non valida")?;
        ensure!(source[start..].starts_with(old), "Posizione XML non valida");
        let mut text = source.to_string();
        text.replace_range(start..start + old.len(), next);
        Ok(self.record(&text))
    }
    pub fn has_snapshot(&self) -> bool {
        self.current.is_some()
    }
    pub fn snapshot(&mut self, source: &str) {
        if self.current.is_none() {
            self.current = Some(source.into());
        }
    }
    pub fn patch(&mut self, path: &Value, old: &Value, new: &Value) -> Value {
        // Retain exact source at a full-document boundary, including comments and formatting.
        let before = self
            .current
            .take()
            .map(State::Xml)
            .unwrap_or_else(|| State::Field(json!({"path":path,"node":old})));
        self.push(before, State::Field(json!({"path":path,"node":new})))
    }
    fn status(&self) -> Value {
        json!({"undo":!self.undo.is_empty(),"redo":!self.redo.is_empty()})
    }
    pub fn step(&mut self, redo: bool) -> Value {
        let state = if redo {
            self.redo.pop().map(|edit| {
                let state = edit.after.clone();
                self.undo.push_back(edit);
                state
            })
        } else {
            self.undo.pop_back().map(|edit| {
                let state = edit.before.clone();
                self.redo.push(edit);
                state
            })
        };
        let mut result = self.status();
        match state.or_else(|| self.current.clone().map(State::Xml)) {
            Some(State::Xml(source)) => {
                result["fields"] = json!(
                    plist::Value::from_reader_xml(source.as_bytes())
                        .ok()
                        .is_some_and(|v| v.as_dictionary().is_some())
                );
                result["xml"] = json!(source.as_ref());
                self.current = Some(source);
            }
            Some(State::Field(patch)) => {
                result["patch"] = patch;
                self.current = None;
            }
            None => (),
        }
        result
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

#[test]
fn field_patches_cross_xml_boundaries() {
    let mut h = History::new("<original/>".into());
    let path = json!(["Misc", "n"]);
    let node = |n| json!(["integer", n]);
    h.patch(&path, &node("1"), &node("2"));
    h.patch(&path, &node("2"), &node("3"));
    assert_eq!(h.step(false)["patch"]["node"], node("2"));
    assert_eq!(h.step(false)["xml"], "<original/>");
    assert_eq!(h.step(true)["patch"]["node"], node("2"));
    h.snapshot("materialized");
    h.record("<edited/>");
    assert_eq!(h.step(false)["xml"], "materialized");
    assert_eq!(h.step(false)["xml"], "<original/>");
}
#[test]
fn xml_edit_uses_utf16_offsets_and_checks_old_text() -> anyhow::Result<()> {
    let mut h = History::new("a🍎b".into());
    assert!(h.splice(2, "", "!").is_err());
    assert!(h.splice(3, "c", "z").is_err());
    h.splice(3, "b", "end")?;
    assert_eq!(h.step(false)["xml"], "a🍎b");
    assert_eq!(h.step(true)["xml"], "a🍎end");
    Ok(())
}
