"use strict";

const bootPath = [
  "NVRAM",
  "Add",
  "7C436110-AB2A-4BBB-A880-FE41995C9F82",
  "boot-args",
];
const bootFlags = {
  "-v": "Mostra i messaggi durante l’avvio.",
  "debug=0x100": "Mantiene visibile il kernel panic.",
  "keepsyms=1": "Conserva i simboli nei backtrace del kernel.",
  "agdpmod=pikera": "Applica la correzione AGDP di WhateverGreen.",
  "alcid=": "Seleziona il layout audio di AppleALC.",
  "-wegnoegpu": "Disabilita le GPU esterne con WhateverGreen.",
  "revpatch=": "Seleziona le patch di RestrictEvents.",
  "-ctrsmt": "Modalità SMT di CpuTopologyRebuild 1.x.",
  "-no_compat_check": "Salta il controllo di compatibilità del modello.",
};
function bootControls(node, input) {
  const box = document.createElement("div");
  box.className = "card";
  const chips = document.createElement("div");
  chips.className = "actions";
  const list = document.createElement("datalist");
  list.id = "boot-suggestions";
  for (const [flag, label] of Object.entries(bootFlags))
    list.append(new Option(t(label), flag));
  const add = document.createElement("input");
  add.setAttribute("aria-label", t("Aggiungi argomento"));
  add.setAttribute("list", list.id);
  const info = document.createElement("small");
  const describe = (value) =>
    t(bootFlags[value] || bootFlags[value.replace(/=.*/, "=")] || "");
  function render() {
    chips.replaceChildren();
    const source = node.textContent;
    for (const m of source.matchAll(/(?:[^\s"']+|"[^"]*"|'[^']*')+/g)) {
      const button = document.createElement("button");
      button.textContent = m[0] + " ×";
      button.title = describe(m[0]);
      button.setAttribute("aria-label", t("Rimuovi") + " " + m[0]);
      button.onclick = () =>
        update(
          (
            source.slice(0, m.index) + source.slice(m.index + m[0].length)
          ).trim(),
        );
      chips.append(button);
    }
  }
  function update(value) {
    input.value = value;
    input.dispatchEvent(new Event("input"));
  }
  input.addEventListener("input", render);
  add.oninput = () => {
    info.textContent = describe(add.value);
  };
  const button = document.createElement("button");
  button.textContent = t("Aggiungi argomento");
  button.onclick = () => {
    const value = add.value.trim();
    if (!value || value.endsWith("=")) {
      add.focus();
      return;
    }
    update([node.textContent.trim(), value].filter(Boolean).join(" "));
    add.value = "";
    info.textContent = "";
  };
  add.onkeydown = (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      button.click();
    }
  };
  box.append(chips, list, add, button, info);
  render();
  return box;
}
function quickBoot() {
  if (!st.w) return;
  try {
    st.doc = parse(st.xml);
  } catch {
    return;
  }
  const node = bootPath.reduce(get, root());
  if (node?.tagName !== "string") return;
  const label = document.createElement("label");
  label.textContent = "boot-args";
  const input = document.createElement("input");
  input.value = node.textContent;
  input.oninput = () => {
    node.textContent = input.value;
    sync();
  };
  label.append(input);
  $("#content").append(label, bootControls(node, input));
}
function setProperty(dict, name, type, value) {
  const old = get(dict, name);
  const node = st.doc.createElement(type);
  if (value !== undefined) node.textContent = value;
  if (old) old.replaceWith(node);
  else {
    const key = st.doc.createElement("key");
    key.textContent = name;
    dict.append(key, node);
  }
  return node;
}
function pciPanel(tree, add) {
  const panel = document.createElement("section");
  panel.className = "card";
  panel.innerHTML =
    '<div class="actions"><label>' +
    t("Aggiungi dispositivo PCI rilevato") +
    ' <select id="pci-list"></select></label>' +
    btn(t("Aggiungi"), "addPci") +
    btn(t("Aggiorna"), "refreshPci") +
    '</div><div class="actions"><label>' +
    t("Percorso PCI") +
    ' <select id="pci-target"></select></label><label>' +
    t("Proprietà") +
    ' <input id="pci-key" value="layout-id" list="pci-properties"></label><datalist id="pci-properties"></datalist></div>' +
    '<div class="actions"><label>' +
    t("Formato sorgente") +
    ' <select id="pci-kind">' +
    ["Int", "String", "Hex", "Data"]
      .map((x) => "<option>" + x + "</option>")
      .join("") +
    "</select></label><label>" +
    t("Valore") +
    ' <input id="pci-value" value="11"></label><label>' +
    t("Tipo plist") +
    ' <select id="pci-type"><option>Data</option><option>Int</option><option>String</option></select></label>' +
    btn(t("Applica alla bozza"), "applyProperty") +
    '</div><pre id="pci-result" aria-live="polite"></pre>';
  tree.append(panel);
  const target = $("#pci-target"),
    key = $("#pci-key"),
    source = $("#pci-value"),
    kind = $("#pci-kind"),
    result = $("#pci-result");
  for (const [path, n] of entries(add))
    if (n.tagName === "dict") target.add(new Option(path, path));
  function properties() {
    const node = get(add, target.value);
    const list = $("#pci-properties");
    list.replaceChildren(
      ...entries(node).map(([name]) => new Option(name, name)),
    );
    const old = get(node, key.value);
    if (old && ["integer", "data", "string"].includes(old.tagName)) {
      kind.value = { integer: "Int", data: "Data", string: "String" }[
        old.tagName
      ];
      $("#pci-type").value = kind.value;
      source.value = old.textContent;
    }
    conversion();
  }
  let revision = 0;
  async function conversion() {
    const id = ++revision;
    try {
      const values = await rpc("convert", {
        kind: kind.value,
        value: source.value,
      });
      if (id === revision)
        result.textContent = Object.entries(values)
          .map(([k, v]) => k + " · " + (v == null ? "—" : JSON.stringify(v)))
          .join("\n");
    } catch (e) {
      if (id === revision) result.textContent = e.message;
    }
  }
  source.oninput = kind.onchange = conversion;
  target.onchange = key.onchange = properties;
  properties();
  fillPci();
  loadReport("pciDevices");
}
function fillPci() {
  const list = $("#pci-list");
  if (!list) return;
  const state = st.reports.pciDevices;
  list.replaceChildren();
  for (const d of state?.value || [])
    list.add(
      new Option(
        d.name + " · " + d.vendor + ":" + d.device + " · " + d.path,
        d.path,
      ),
    );
  if (!list.length)
    list.add(
      new Option(
        t(
          state?.state === "error"
            ? "Errore report"
            : state?.state === "ready"
              ? "Nessun dispositivo PCI"
              : "Caricamento…",
        ),
        "",
      ),
    );
}
async function editProperty(op) {
  const add = get(get(root(), "DeviceProperties"), "Add");
  if (op === "refreshPci") {
    loadReport("pciDevices", true);
    return;
  }
  if (op === "addPci") {
    const path = $("#pci-list").value;
    if (path && !get(add, path)) {
      setProperty(add, path, "dict");
      sync();
    }
    draw();
    if (path && $("#pci-target")) {
      $("#pci-target").value = path;
      $("#pci-target").onchange();
    }
    return;
  }
  const target = get(add, $("#pci-target").value),
    key = $("#pci-key").value.trim();
  if (!target || !key)
    throw Error(t("Seleziona un dispositivo e una proprietà"));
  const values = await rpc("convert", {
    kind: $("#pci-kind").value,
    value: $("#pci-value").value,
  });
  const type = $("#pci-type").value,
    value = values[type];
  if (
    value == null ||
    (type === "String" &&
      /[\u0000-\u0008\u000B\u000C\u000E-\u001F\uFFFE\uFFFF]/.test(value))
  )
    throw Error(t("Valore incompatibile con il tipo plist"));
  setProperty(
    target,
    key,
    { Int: "integer", String: "string", Data: "data" }[type],
    String(value),
  );
  sync();
  draw();
}
