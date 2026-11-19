use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};

pub fn normalized(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

/// Indices refer to plist entries, including multiple entries for one bundle.
pub fn order(paths: &[String], files: &[Value]) -> Value {
    let by_path: HashMap<_, _> = files
        .iter()
        .filter_map(|f| Some((normalized(f["path"].as_str()?), f)))
        .collect();
    let keys: Vec<_> = paths.iter().map(|p| normalized(p)).collect();
    let mut providers: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, path) in keys.iter().enumerate() {
        if let Some(id) = by_path.get(path.as_str()).and_then(|f| f["id"].as_str()) {
            providers.entry(id).or_default().push(i);
        }
    }
    let mut edges = vec![BTreeSet::new(); paths.len()];
    let mut missing = Vec::new();
    for (i, path) in keys.iter().enumerate() {
        let Some(file) = by_path.get(path.as_str()) else {
            continue;
        };
        if let Some((parent, _)) = path.rsplit_once("/contents/plugins/") {
            edges[i].extend(
                keys.iter()
                    .enumerate()
                    .filter(|(_, p)| p.as_str() == parent)
                    .map(|(i, _)| i),
            );
        }
        for dep in file["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if let Some(indices) = providers.get(dep) {
                edges[i].extend(indices);
            } else if !dep.starts_with("com.apple.kpi.") && !dep.starts_with("com.apple.iokit.") {
                missing.push(json!([paths[i], dep]));
            }
        }
    }
    let mut result = Vec::new();
    let mut used = vec![false; paths.len()];
    while let Some(i) = (0..paths.len()).find(|&i| !used[i] && edges[i].iter().all(|&p| used[p])) {
        used[i] = true;
        result.push(i);
    }
    let mut cycles = Vec::new();
    for i in 0..paths.len() {
        if used[i] {
            continue;
        }
        let mut stack: Vec<_> = edges[i].iter().copied().collect();
        let mut seen = BTreeSet::new();
        while let Some(p) = stack.pop() {
            if p == i {
                cycles.push(&paths[i]);
                break;
            }
            if !used[p] && seen.insert(p) {
                stack.extend(&edges[p]);
            }
        }
    }
    if !cycles.is_empty() {
        result = (0..paths.len()).collect();
    }
    json!({"order":result,"cycles":cycles,"missing":missing})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_order_plugins_and_missing() {
        let p = ["A.kext/Contents/PlugIns/B.kext", "Other", "A.kext", "Lilu"].map(String::from);
        let files = vec![
            json!({"path":p[0].to_uppercase(),"id":"b"}),
            json!({"path":"A.kext","id":"a","dependencies":["lilu","missing","com.apple.kpi.bsd","com.apple.iokit.X"]}),
            json!({"path":"Lilu","id":"lilu"}),
        ];
        let result = order(&p, &files);
        assert_eq!(result["order"], json!([1, 3, 2, 0]));
        assert_eq!(result["missing"], json!([["A.kext", "missing"]]));
    }
    #[test]
    fn cycle_excludes_blocked_consumers_and_keeps_input_order() {
        let p = ["a", "b", "c"].map(String::from);
        let files: Vec<_> = ["b", "a", "a"]
            .iter()
            .enumerate()
            .map(|(i, d)| json!({"path":p[i],"id":p[i],"dependencies":[d]}))
            .collect();
        let result = order(&p, &files);
        assert_eq!(result["cycles"], json!(["a", "b"]));
        assert_eq!(result["order"], json!([0, 1, 2]));
    }
}
