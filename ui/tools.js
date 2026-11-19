"use strict";

const pathKey = (value) => value.replace(/\\/g, "/").toLowerCase();
function kernelSelect(key, current, change) {
  const select = document.createElement("select");
  select.setAttribute("aria-label", key);
  const names = [
    "Big Sur",
    "Monterey",
    "Ventura",
    "Sonoma",
    "Sequoia",
    "Tahoe",
  ];
  const options = [
    ["", "—"],
    ...names.map((name, i) => [20 + i + ".0.0", name]),
  ];
  if (current && !options.some(([v]) => v === current))
    options.push([current, current]);
  for (const [v, name] of options)
    select.add(new Option(key + " · " + name, v, false, v === current));
  select.onchange = () => change(select.value);
  return select;
}
// This dialog also works with the version of WebKit shipped with macOS 11.
function preview(title, text, apply) {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "overlay";
    const box = document.createElement("section");
    box.className = "card modal";
    box.setAttribute("role", "dialog");
    box.setAttribute("aria-modal", "true");
    box.setAttribute("aria-labelledby", "preview-title");
    const heading = document.createElement("h2");
    heading.id = "preview-title";
    heading.textContent = t(title);
    const pre = document.createElement("pre");
    pre.textContent = text;
    const controls = document.createElement("div");
    controls.className = "actions";
    const previous = document.activeElement;
    function end(answer) {
      overlay.remove();
      previous?.focus();
      resolve(answer);
    }
    function button(label, answer) {
      const b = document.createElement("button");
      b.textContent = t(label);
      b.onclick = () => end(answer);
      controls.append(b);
      return b;
    }
    const cancel = button(apply ? "Annulla" : "Chiudi", false);
    const last = apply ? button(apply, true) : cancel;
    box.append(heading, pre, controls);
    overlay.append(box);
    document.body.append(overlay);
    cancel.focus();
    overlay.onkeydown = (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        end(false);
      }
      if (
        e.key === "Tab" &&
        ((e.shiftKey && document.activeElement === cancel) ||
          (!e.shiftKey && document.activeElement === last))
      ) {
        e.preventDefault();
        (e.shiftKey ? last : cancel).focus();
      }
    };
  });
}
async function snapshot(clean) {
  const doc = parse(st.xml),
    root = doc.documentElement.firstElementChild;
  const inventory = await rpc("inventory");
  st.w.inventory = inventory;
  const changes = [],
    warnings = [];
  for (const [group, [section, key, fieldName]] of Object.entries(groups)) {
    if (!clean && group !== "Kexts") continue;
    const list = get(get(root, section), key);
    if (list?.tagName !== "array") {
      warnings.push(section + " → " + key + " · " + t("Array assente"));
      continue;
    }
    const path = (n) =>
      n.tagName === "string" ? n.textContent : get(n, fieldName)?.textContent;
    if (clean) {
      const files = inventory[group],
        registered = new Set();
      for (const n of Array.from(list.children)) {
        const p = path(n);
        if (!p) {
          warnings.push(section + " → " + key + " · " + t("Percorso vuoto"));
          continue;
        }
        if (!files.some((f) => pathKey(f.path) === pathKey(p))) {
          changes.push("− " + group + "/" + p);
          n.remove();
        } else registered.add(pathKey(p));
      }
      for (const f of files) {
        if (registered.has(pathKey(f.path))) continue;
        if (f.valid === false) {
          warnings.push(f.path + " · " + t("Bundle incompleto"));
          continue;
        }
        changes.push("+ " + group + "/" + f.path);
        list.append(componentEntry(doc, group, { file: f, path: f.path }));
      }
    }
    if (group === "Kexts") {
      const nodes = Array.from(list.children);
      const result = await rpc("order", {
        paths: nodes.map((n) => path(n) || ""),
      });
      for (const [k, d] of result.missing)
        warnings.push(k + " → " + t("Dipendenza assente") + ": " + d);
      if (result.cycles.length)
        warnings.push(
          t("Dipendenza circolare") + ": " + result.cycles.join(", "),
        );
      if (result.order.some((v, i) => v !== i)) {
        changes.push(
          ...result.order.flatMap((v, i) =>
            v === i ? [] : [v + 1 + " → " + (i + 1) + "  " + path(nodes[v])],
          ),
        );
        for (const index of result.order) list.append(nodes[index]);
      }
    }
  }
  if (
    await preview(
      clean ? "Sincronizza cartelle EFI" : "Ordina per dipendenze",
      [...changes, ...warnings].join("\n") || t("Nessuna modifica"),
      changes.length ? "Applica alla bozza" : null,
    )
  ) {
    st.doc = doc;
    sync();
  }
}
function markField(el, path) {
  if (st.focusPath && JSON.stringify(path) === JSON.stringify(st.focusPath)) {
    el.classList.add("highlight");
    requestAnimationFrame(() => {
      el.scrollIntoView({ block: "center" });
      el.focus();
    });
  }
}
function diagnosticsView(result) {
  const doc = (() => {
    try {
      return parse(st.xml);
    } catch {
      return null;
    }
  })();
  st.links = [];
  const link = (path, label) => {
    st.links.push(path);
    return btn(label, "jump-" + (st.links.length - 1), "compact");
  };
  const lines = result.log.split("\n").map((line) => {
    if (!doc) return esc(line);
    const sections = entries(doc.documentElement.firstElementChild);
    const words = new Set(line.match(/[A-Za-z_][A-Za-z_0-9]*/g) || []);
    for (const [k, n] of sections) {
      if (line.includes("Check" + k + " returns"))
        return esc(line) + " " + link([k], k);
      const start = line.indexOf(k + "->");
      if (start < 0) continue;
      const tokens = (line
        .slice(start)
        .match(/^[A-Za-z0-9_]+(?:(?:->)[A-Za-z0-9_]+|\[\d+\])+/) || [])[0];
      if (!tokens) continue;
      const parts = tokens.match(/[A-Za-z_][A-Za-z0-9_]*|\d+/g).slice(1);
      let node = n;
      const path = [k];
      for (const part of parts) {
        const index = /^\d+$/.test(part);
        const next = index ? node?.children[Number(part)] : get(node, part);
        if (!next) break;
        node = next;
        path.push(index ? Number(part) : part);
      }
      if (node?.tagName === "dict") {
        const named = entries(node).filter(([key]) => words.has(key));
        if (named.length === 1) path.push(named[0][0]);
      }
      return esc(line) + " " + link(path, path.join(" → "));
    }
    const context = line.match(/context\s*<([^>]+)>/i)?.[1];
    let candidates = [];
    function visit(n, path, score) {
      if (n.tagName === "dict")
        for (const [k, v] of entries(n)) {
          const weight =
            score + (words.has(k) ? 1 : 0) + (k === context ? 2 : 0);
          if (words.has(k)) candidates.push({ path: [...path, k], weight });
          visit(v, [...path, k], weight);
        }
      else if (n.tagName === "array")
        Array.from(n.children).forEach((v, i) => visit(v, [...path, i], score));
    }
    for (const [k, n] of sections) {
      if (words.has(k)) candidates.push({ path: [k], weight: 1 });
      visit(n, [k], words.has(k) ? 1 : 0);
    }
    if (!candidates.length) return esc(line);
    const best = Math.max(...candidates.map((c) => c.weight));
    const matches = candidates.filter((c) => c.weight === best);
    const common = matches[0].path.slice();
    while (
      common.length &&
      !matches.every((c) => common.every((v, i) => c.path[i] === v))
    )
      common.pop();
    return common.length
      ? esc(line) + " " + link(common, common.join(" → "))
      : esc(line);
  });
  const labels = {
    cores: "Core fisici diversi dal byte Replace",
    pattern: "Formato patch non riconosciuto",
    conflict: "Conflitto",
    duplicate: "Bundle duplicato",
    order: "Caricato prima della dipendenza",
    model: "Modello configurato e modello del Mac",
  };
  return (
    '<div class="pair"><div class="card"><h2>ocvalidate</h2><pre>' +
    lines.join("\n") +
    '</pre></div><div class="card"><h2>' +
    t("Diagnostica e suggerimenti") +
    "</h2>" +
    ((result.issues || [])
      .map(
        (i) =>
          "<p>" +
          esc(t(labels[i.kind])) +
          " · " +
          esc(i.details.join(" → ")) +
          " " +
          link(i.path, i.path.join(" → ")) +
          "</p>",
      )
      .join("") || esc(t("Nessuna segnalazione"))) +
    "</div></div>"
  );
}
function selectedDisk() {
  return st.disks.find(
    (d) =>
      d.mount &&
      st.w &&
      (st.w.root === d.mount || st.w.root.startsWith(d.mount + "/")),
  );
}
function volumeStatus() {
  const current = selectedDisk(),
    mounted = st.disks.filter((d) => d.mount);
  const el = $("#active-volume");
  el.textContent = current
    ? current.id + " · " + current.mount
    : mounted.map((d) => d.id + " · " + d.mount).join(" / ") ||
      t("Nessuna EFI montata");
  if (current)
    el.insertAdjacentHTML(
      "beforeend",
      " " +
        btn(
          current.ejectable ? "Salva, smonta ed espelli" : "Salva e smonta",
          "finish",
          "compact",
        ),
    );
}
