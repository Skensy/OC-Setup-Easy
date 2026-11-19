"use strict";
const $ = (s) => document.querySelector(s);
const esc = (value) =>
  String(value ?? "").replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
const icons = {
  layers: "M3 8l9-5 9 5-9 5z M3 12l9 5 9-5 M3 16l9 5 9-5",
  home: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
  disk: "M5 5h14l3 11v3H2v-3z M3 15h18 M6 17h1",
  folder: "M2 7V4h7l3 3h9v13H2z",
  code: "M8 6l-5 6 5 6 M16 6l5 6-5 6 M14 3l-4 18",
  chip: "M6 6h12v12H6z M9 1v5m6-5v5M9 18v5m6-5v5M1 9h5m-5 6h5m12-6h5m-5 6h5",
  check: "M3 12l6 6L21 6",
  activity: "M2 12h4l3-8 5 16 3-8h5",
  reload: "M20 10a8 8 0 1 0 0 6 M20 3v7h-7",
  files: "M5 3h9l5 5v13H5z M14 3v6h5 M8 14h8 M8 17h5",
  arrow: "M5 12h14m-6-6 6 6-6 6",
  save: "M3 3h15l3 3v15H3z M7 3v6h9V3 M7 21v-8h10v8",
  box: "M3 7l9-4 9 4v10l-9 4-9-4z M3 7l9 4 9-4m-9 4v10",
  terminal: "M2 4h20v16H2z M5 8l4 4-4 4m7 0h5",
  shield: "M12 2l8 3v6c0 5-4 8-8 11-4-3-8-6-8-11V5z",
};
function icon(key) {
  return (
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round" aria-hidden="true"><path d="' +
    icons[key] +
    '"/></svg>'
  );
}
document
  .querySelectorAll("[data-icon]")
  .forEach((el) => (el.innerHTML = icon(el.dataset.icon)));
const pages = {
  home: ["Panoramica", "home"],
  disks: ["Volumi EFI", "disk"],
  config: ["Editor plist", "code"],
  files: ["Kext e componenti", "layers"],
  validate: ["Verifica e backup", "shield"],
  system: ["Hardware e sistema", "activity"],
};
const st = {
  page: "home",
  system: {},
  disks: [],
  w: null,
  xml: "",
  original: "",
  doc: null,
  dirty: false,
  mode: "fields",
  section: "",
  group: "Kexts",
  validation: null,
  report: "",
  category: "graphics",
  busy: false,
};
const groups = {
  Kexts: ["Kernel", "Add", "BundlePath"],
  ACPI: ["ACPI", "Add", "Path"],
  Drivers: ["UEFI", "Drivers", "Path"],
  Tools: ["Misc", "Tools", "Path"],
};
const btn = (name, action, cls = "", glyph = "") =>
  '<button type="button" data-act="' +
  action +
  '" class="' +
  cls +
  '">' +
  (glyph ? icon(glyph) : "") +
  esc(name) +
  "</button>";
const tag = (name, warn = false) =>
  '<span class="tag' + (warn ? " warn" : "") + '">' + esc(name) + "</span>";
const pairs = (entries) =>
  "<dl>" +
  entries
    .map(
      ([k, v]) =>
        "<div><dt>" +
        esc(k) +
        "</dt><dd>" +
        esc(v || "Non disponibile") +
        "</dd></div>",
    )
    .join("") +
  "</dl>";
const codeText = (s) => "<pre>" + esc(s) + "</pre>";
function notice(msg, error = false) {
  const el = $("#notice");
  el.textContent = msg;
  el.hidden = !msg;
  el.className = error ? "error" : "";
}
const bridge = (op, params = {}) =>
  window.webkit.messageHandlers.oc.postMessage({ op, ...params });
async function rpc(op, params = {}) {
  const r = await bridge(op, params);
  if (r.error) throw Error(r.error);
  return r.cancelled ? null : r.ok;
}
async function work(fn) {
  if (st.busy) return;
  st.busy = true;
  $("main").inert = true;
  $("aside").inert = true;
  document.body.classList.add("wait");
  $("#busy").hidden = false;
  try {
    await fn();
  } catch (e) {
    notice(e.message || String(e), true);
  } finally {
    st.busy = false;
    $("main").inert = false;
    $("aside").inert = false;
    document.body.classList.remove("wait");
    $("#busy").hidden = true;
  }
}
function dirty() {
  const value = st.xml !== st.original;
  if (value !== st.dirty) {
    st.dirty = value;
    bridge("dirty", { value }).catch((e) => notice(e.message, true));
  }
  st.validation = null;
  const save = $("[data-act=save]");
  if (save) save.disabled = !value;
  const flag = $("#dirty");
  if (flag) flag.textContent = value ? "Modifiche in bozza" : "Su disco";
}
function parse(text) {
  const doc = new DOMParser().parseFromString(text, "application/xml");
  if (
    doc.querySelector("parsererror") ||
    doc.documentElement.tagName !== "plist" ||
    doc.documentElement.children.length !== 1 ||
    doc.documentElement.firstElementChild.tagName !== "dict"
  )
    throw Error(
      "XML non valido. Controlla la struttura del plist nella vista XML.",
    );
  for (const dict of doc.getElementsByTagName("dict")) {
    const keys = new Set(),
      items = dict.children;
    for (let i = 0; i < items.length; i += 2) {
      if (
        items[i].tagName !== "key" ||
        !items[i + 1] ||
        items[i + 1].tagName === "key" ||
        keys.has(items[i].textContent)
      )
        throw Error("Dizionario con chiave duplicata o valore mancante.");
      keys.add(items[i].textContent);
    }
  }
  return doc;
}
function entries(dict) {
  if (dict?.tagName !== "dict") return [];
  const fields = Array.from(dict.children);
  return fields
    .filter((_, i) => i % 2 === 0)
    .map((k) => [k.textContent, k.nextElementSibling]);
}
const get = (d, k) => entries(d).find(([key]) => key === k)?.[1];
const root = () => st.doc.documentElement.firstElementChild;
function sync() {
  st.xml = new XMLSerializer().serializeToString(st.doc);
  dirty();
}
function typed(doc, value) {
  const n = doc.createElement(
    typeof value === "boolean" ? String(value) : "string",
  );
  if (typeof value !== "boolean") n.textContent = value;
  return n;
}
function discard() {
  return !st.dirty || confirm("Scartare le modifiche in bozza?");
}
function openData(data) {
  let doc = null;
  try {
    doc = parse(data.xml);
  } catch (err) {
    notice(err.message, true);
  }
  Object.assign(st, {
    w: data,
    xml: data.xml,
    original: data.xml,
    doc,
    section: "",
    mode: doc ? "fields" : "xml",
    validation: null,
    page: doc ? "files" : "config",
  });
  dirty();
}
function draw() {
  const { page, w, system: s } = st;
  $("#nav").innerHTML = Object.entries(pages)
    .map(
      ([id, [title, glyph]], i) =>
        (i === 0 || i === 2 || i === 5
          ? '<span class="navgroup">' +
            { 0: "Workspace", 2: "Configurazione", 5: "Diagnostica" }[i] +
            "</span>"
          : "") +
        '<button class="nav" data-page="' +
        id +
        '" ' +
        (id === page ? 'aria-current="page"' : "") +
        ">" +
        icon(glyph) +
        title +
        "</button>",
    )
    .join("");
  $("#crumb").textContent = "Workbench   /   " + pages[page][0];
  if (page === "home") return home();
  let head =
    '<div class="heading"><div><div class="label">Workbench</div><h1>' +
    pages[page][0] +
    "</h1></div></div>";
  if (w && ["config", "files", "validate"].includes(page))
    head +=
      '<div class="subline paths">' +
      esc(w.root) +
      '/config.plist · <span id="dirty">' +
      (st.dirty ? "Modifiche in bozza" : "Su disco") +
      "</span></div>";
  let body;
  if (page === "disks") body = disks();
  else if (page === "system") body = systemPanel(s);
  else if (!w)
    body =
      '<div class="card empty"><h2>Apri una configurazione OpenCore</h2><p>Seleziona una cartella EFI o OC, anche da un backup. Per accedere a una partizione usa Volumi EFI.</p>' +
      btn("Apri cartella EFI", "open", "primary", "folder") +
      "</div>";
  else if (page === "config") body = config();
  else if (page === "files") body = filePanel();
  else body = validation();
  $("#content").innerHTML = head + body;
  if (page === "config" && w) {
    if (st.mode === "xml") {
      $("#xml").value = st.xml;
    } else buildTree();
  }
  const save = $("[data-act=save]");
  if (save) save.disabled = !st.dirty;
}
function home() {
  const s = st.system,
    w = st.w;
  const stat = (k, v, n) =>
    '<div class="stat"><span class="label">' +
    k +
    "</span><strong>" +
    esc(v) +
    "</strong><small>" +
    esc(n) +
    "</small></div>";
  $("#content").innerHTML =
    '<div class="hero"><div><span class="label">OpenCore Workbench</span><h1>Panoramica di sistema</h1><p>' +
    esc(s.cpu || "Processore non disponibile") +
    '</p></div><img src="chip.svg" alt=""></div><div class="stats">' +
    stat(
      "Sistema operativo",
      "macOS " + (s.os || "—"),
      "Build " + (s.build || "—"),
    ) +
    stat(
      "Memoria fisica",
      s.ram == null ? "—" : s.ram + " GiB",
      (s.cores || "—") + " core · " + (s.threads || "—") + " thread",
    ) +
    stat(
      "Partizioni EFI",
      String(st.disks.length).padStart(2, "0"),
      st.disks.filter((d) => d.mount).length + " montate",
    ) +
    "</div>" +
    '<div class="pair"><div class="card"><div class="head"><h2>Configurazione</h2><span class="tile">' +
    icon("folder") +
    "</span></div><p>" +
    (w ? "Cartella di lavoro aperta" : "Scegli l’EFI su cui vuoi lavorare.") +
    "</p>" +
    (w
      ? '<p class="paths">' +
        esc(w.root) +
        "</p>" +
        btn("Apri editor", "go-config", "primary", "code") +
        " " +
        btn("Chiudi", "close")
      : btn("Apri cartella EFI", "open", "primary", "folder") +
        " " +
        btn("Volumi EFI", "go-disks")) +
    '</div><div class="card"><div class="head"><h2>Ambiente di avvio</h2><span class="tile">' +
    icon("chip") +
    "</span></div>" +
    pairs([
      ["OpenCore attivo", s.oc],
      ["SMBIOS", s.model],
      ["Darwin", s.kernel],
      ["Architettura", s.arch],
    ]) +
    "</div></div>";
}
function disks() {
  return (
    '<div class="actions">' +
    btn("Aggiorna dischi", "refresh", "", "reload") +
    '</div><div class="table">' +
    (st.disks
      .map(
        (d, i) =>
          '<div class="row"><span class="tile">' +
          icon("disk") +
          '</span><div class="entry"><strong>' +
          esc(d.name || "EFI") +
          "</strong><small>" +
          esc([d.id, d.model, d.bus].filter(Boolean).join(" · ")) +
          " · " +
          esc((d.size / 1e9).toFixed(2)) +
          " GB" +
          (d.mount ? " · " + esc(d.mount) : "") +
          "</small></div>" +
          tag(d.mount ? "Montata" : "Smontata", !d.mount) +
          (d.mount
            ? btn("Apri EFI", "volume-" + i) + btn("Smonta", "unmount-" + i)
            : btn("Monta", "mount-" + i, "primary")) +
          "</div>",
      )
      .join("") ||
      '<div class="empty">Nessuna partizione EFI rilevata.</div>') +
    "</div>" +
    (st.w
      ? '<div class="actions">' +
        btn("Chiudi configurazione", "close") +
        "</div>"
      : "")
  );
}
const categories = {
  graphics: "Grafica e monitor",
  pci: "Dispositivi PCI",
  audio: "Audio",
  storage: "Controller NVMe",
  memory: "Memoria",
  usb: "Dispositivi USB",
  network: "Rete",
  loaded: "Kext caricati",
  power: "Energia",
};
function systemPanel(s) {
  return (
    '<div class="card"><h2>Hardware e avvio</h2>' +
    pairs([
      ["CPU", s.cpu],
      ["Core / thread", s.cores + " / " + s.threads],
      ["Memoria", s.ram == null ? "" : s.ram + " GiB"],
      ["Modello SMBIOS", s.model],
      ["Versione OpenCore (NVRAM)", s.oc],
      ["Boot arguments", s.boot],
      ["Build macOS", s.build],
    ]) +
    '</div><div class="actions"><label for="category">Report macOS</label><select id="category">' +
    Object.entries(categories)
      .map(
        ([k, v]) =>
          '<option value="' +
          k +
          '" ' +
          (st.category === k ? "selected" : "") +
          ">" +
          v +
          "</option>",
      )
      .join("") +
    '<optgroup label="Tutti i report di system_profiler">' +
    (s.reports || [])
      .map(
        (k) =>
          '<option value="' +
          esc(k) +
          '" ' +
          (st.category === k ? "selected" : "") +
          ">" +
          esc(k.slice(2, -8)) +
          "</option>",
      )
      .join("") +
    "</optgroup></select>" +
    btn("Leggi report", "report", "", "terminal") +
    "</div>" +
    (st.report ? codeText(st.report) : "")
  );
}
function config() {
  return (
    '<div class="actions">' +
    btn("Campi", "fields") +
    btn("XML", "xml") +
    btn("Verifica", "verify", "", "shield") +
    btn("Salva plist", "save", "primary", "save") +
    "</div>" +
    (st.mode === "xml"
      ? '<label for="xml" class="label">Sorgente XML</label><textarea spellcheck="false" autocapitalize="off" id="xml"></textarea>'
      : '<div class="filters" id="sections"></div><div class="card tree" id="tree"></div>')
  );
}
function buildTree() {
  const all = entries(root());
  if (!all.some(([k]) => k === st.section)) st.section = all[0]?.[0] || "";
  const tabs = $("#sections");
  for (const [k] of all) {
    const b = document.createElement("button");
    b.textContent = k;
    b.setAttribute("aria-pressed", k === st.section);
    b.onclick = () => {
      st.section = k;
      draw();
    };
    tabs.append(b);
  }
  const tree = $("#tree");
  const node = get(root(), st.section);
  if (!node) {
    tree.textContent = "Dizionario vuoto. Aggiungi chiavi nella vista XML.";
    return;
  }
  function append(parent, label, n, depth) {
    const type = n.tagName;
    if (type === "dict" || type === "array") {
      const wrap = document.createElement("details");
      wrap.open = depth === 0;
      const summary = document.createElement("summary");
      const children =
        type === "dict"
          ? entries(n)
          : Array.from(n.children, (el, i) => ["[" + i + "]", el]);
      summary.textContent = label + " · " + children.length;
      wrap.append(summary);
      parent.append(wrap);
      const fill = () => {
        for (const [k, v] of children) append(wrap, k, v, depth + 1);
      };
      if (wrap.open) fill();
      else
        wrap.addEventListener("toggle", () => {
          if (wrap.open && wrap.childElementCount === 1) fill();
        });
    } else {
      const row = document.createElement("label");
      row.className = "field";
      const name = document.createElement("span");
      name.textContent =
        label + " · " + (type === "true" || type === "false" ? "bool" : type);
      const input = document.createElement("input");
      const bool = type === "true" || type === "false";
      if (bool) {
        input.type = "checkbox";
        input.checked = type === "true";
      } else {
        input.value = n.textContent;
        input.spellcheck = false;
      }
      input.addEventListener("input", () => {
        if (bool) {
          const next = typed(st.doc, input.checked);
          n.replaceWith(next);
          n = next;
        } else n.textContent = input.value;
        sync();
      });
      row.append(name, input);
      parent.append(row);
    }
  }
  append(tree, st.section, node, 0);
}
function listNode() {
  const [section, key] = groups[st.group];
  return get(get(root(), section), key);
}
function inventoryRows() {
  const list = listNode(),
    key = groups[st.group][2],
    physical = st.w.inventory[st.group];
  const configured = Array.from(list?.children || []).map((n, index) => {
    const path =
      n.tagName === "string" ? n.textContent : get(n, key)?.textContent || "";
    return {
      path,
      index,
      n,
      file: physical.find((f) => f.path === path),
      enabled: n.tagName === "string" || get(n, "Enabled")?.tagName === "true",
    };
  });
  return configured.concat(
    physical
      .filter((f) => !configured.some((c) => c.path === f.path))
      .map((file) => ({ path: file.path, file })),
  );
}
function groupControls() {
  return (
    '<div class="filters">' +
    Object.keys(groups)
      .map(
        (g) =>
          '<button data-group="' +
          g +
          '" aria-pressed="' +
          (st.group === g) +
          '">' +
          g +
          "</button>",
      )
      .join("") +
    "</div>"
  );
}
function filePanel() {
  let rows = inventoryRows();
  return (
    groupControls() +
    '<div class="actions">' +
    (st.group === "Kexts" ? btn("Copia un kext…", "import", "", "box") : "") +
    btn("Rileggi file", "inventory", "", "reload") +
    btn("Apri nel Finder", "finder") +
    btn("Salva plist", "save", "primary", "save") +
    '</div><div class="table">' +
    (rows
      .map((r, i) => {
        const configured = r.index !== undefined;
        const orderIssues =
          st.group === "Kexts" && r.enabled
            ? (r.file?.dependencies || []).flatMap((dep) => {
                const providers = rows.filter(
                  (other) => other.file?.id === dep,
                );
                if (
                  !providers.length ||
                  providers.some((p) => p.enabled && p.index < r.index)
                )
                  return [];
                return [
                  dep +
                    (providers.some((p) => p.enabled)
                      ? " segue questo kext"
                      : " non attivo"),
                ];
              })
            : [];
        const detail = [
          r.file?.version,
          r.file?.id,
          r.file?.bytes != null ? r.file.bytes + " byte" : "",
          r.file?.dependencies?.length
            ? "Richiede: " + r.file.dependencies.join(", ")
            : "",
        ]
          .filter(Boolean)
          .join(" · ");
        return (
          '<div class="row"><div class="entry"><strong>' +
          esc(r.path || "(percorso vuoto)") +
          "</strong><small>" +
          esc(detail) +
          "</small>" +
          (orderIssues.length
            ? '<small class="error">' +
              esc(orderIssues.join(" · ")) +
              "</small>"
            : "") +
          "</div>" +
          tag(
            !r.file
              ? "File assente"
              : !configured
                ? "Solo su disco"
                : r.enabled
                  ? "Attivo"
                  : "Disattivato",
            !r.file || !configured,
          ) +
          (configured
            ? (get(r.n, "Enabled")
                ? btn(
                    r.enabled ? "Disattiva" : "Attiva",
                    "toggle-" + i,
                    "compact",
                  )
                : "") +
              (i > 0 ? btn("↑", "up-" + i, "compact") : "") +
              (i + 1 < listNode().children.length
                ? btn("↓", "down-" + i, "compact")
                : "") +
              btn("Rimuovi voce", "remove-" + i, "compact")
            : r.file.valid !== false
              ? btn("Registra", "add-" + i, "compact")
              : "Bundle incompleto") +
          "</div>"
        );
      })
      .join("") ||
      '<div class="empty">Nessuna voce in questa categoria.</div>') +
    "</div>"
  );
}
function changeEntry(op, i) {
  const r = inventoryRows()[i],
    list = listNode();
  if (!list || list.tagName !== "array")
    throw Error(
      "Array " +
        groups[st.group].slice(0, 2).join(" → ") +
        " assente. Aggiungilo nella vista XML.",
    );
  if (op === "add") {
    let fields = { Comment: "", Enabled: false, Path: r.path };
    if (st.group === "Kexts")
      fields = {
        Arch: "Any",
        BundlePath: r.path,
        Comment: "",
        Enabled: false,
        ExecutablePath: r.file.executable,
        MaxKernel: "",
        MinKernel: "",
        PlistPath: "Contents/Info.plist",
      };
    if (st.group === "Drivers")
      Object.assign(fields, {
        Arguments: "",
        LoadEarly: false,
        HideVerbose: false,
      });
    if (st.group === "Tools")
      Object.assign(fields, {
        Arguments: "",
        Auxiliary: true,
        Flavour: "Auto",
        FullNvramAccess: false,
        Name: r.path.split("/").pop(),
        RealPath: false,
        TextMode: false,
      });
    const dict = st.doc.createElement("dict");
    for (const [k, v] of Object.entries(fields)) {
      const key = st.doc.createElement("key");
      key.textContent = k;
      dict.append(key, typed(st.doc, v));
    }
    list.append(dict);
  } else if (op === "remove") {
    if (
      !confirm(
        "Rimuovere dal plist la voce " +
          r.path +
          "? Il file resterà sul disco.",
      )
    )
      return;
    r.n.remove();
  } else if (op === "toggle")
    get(r.n, "Enabled").replaceWith(typed(st.doc, !r.enabled));
  else if (op === "up") list.insertBefore(r.n, r.n.previousElementSibling);
  else if (op === "down") list.insertBefore(r.n.nextElementSibling, r.n);
  sync();
  draw();
}
function validation() {
  return (
    '<div class="pair"><div class="card"><div class="head"><h2>Controllo configurazione</h2>' +
    tag("ocvalidate 1.0.8") +
    "</div><p>Analizza la bozza con le regole di OpenCore 1.0.8. Per altre versioni il risultato può differire.</p>" +
    btn("Verifica bozza", "verify", "primary", "shield") +
    '</div><div class="card"><h2>Archivio di ripristino</h2><p>Crea uno ZIP della cartella EFI su disco, oppure della sola OC se aperta separatamente.</p>' +
    btn("Crea backup ZIP", "backup", "", "box") +
    " " +
    btn("Mostra backup", "backups") +
    "</div></div>" +
    (st.validation
      ? '<div class="actions">' +
        tag(
          st.validation.pass ? "Verifica superata" : "Anomalie rilevate",
          !st.validation.pass,
        ) +
        "</div>" +
        codeText(st.validation.log)
      : "")
  );
}
async function act(a) {
  notice("");
  if (a.startsWith("go-")) {
    st.page = a.slice(3);
    draw();
    return;
  }
  const parts = a.match(
    /^(volume|mount|unmount|add|toggle|up|down|remove)-(\d+)$/,
  );
  if (parts) {
    const [, op, index] = parts,
      i = Number(index);
    if (op === "volume") {
      if (!discard()) return;
      const w = await rpc("open", { volume: st.disks[i].mount });
      openData(w);
    } else if (op === "mount" || op === "unmount") {
      st.disks = await rpc(op, { uuid: st.disks[i].uuid, id: st.disks[i].id });
      notice(op === "mount" ? "EFI montata." : "EFI smontata.");
    } else {
      changeEntry(op, i);
      return;
    }
  } else
    switch (a) {
      case "open": {
        if (!discard()) return;
        const w = await rpc("open");
        if (w) {
          openData(w);
        }
        break;
      }
      case "close":
        if (!discard()) return;
        await rpc("close");
        Object.assign(st, { w: null, xml: "", original: "", doc: null });
        dirty();
        break;
      case "refresh":
        st.disks = await rpc("disks");
        break;
      case "report":
        st.report =
          (await rpc("report", { category: st.category })) ||
          "Nessun dato restituito da macOS.";
        break;
      case "fields":
        st.doc = parse(st.xml);
        st.mode = "fields";
        break;
      case "xml":
        st.mode = "xml";
        break;
      case "inventory":
        st.w.inventory = await rpc("inventory");
        break;
      case "finder":
      case "backups":
        await rpc(a);
        break;
      case "import": {
        const data = await rpc("import");
        if (data) {
          st.w.inventory = data;
          notice(
            "Bundle copiato. Registra e attiva le voci desiderate, poi salva il plist.",
          );
        }
        break;
      }
      case "backup":
        notice("Backup ZIP creato in " + (await rpc("backup")));
        break;
      case "verify":
        st.validation = await rpc("validate", { xml: st.xml });
        st.page = "validate";
        break;
      case "save": {
        const doc = parse(st.xml);
        const check = await rpc("validate", { xml: st.xml });
        if (!check.pass) {
          st.validation = check;
          st.page = "validate";
          draw();
          if (
            !confirm(
              "ocvalidate 1.0.8 segnala anomalie. Salvare comunque questa configurazione?",
            )
          )
            return;
        }
        const backup = await rpc("save", { xml: st.xml });
        st.doc = doc;
        st.original = st.xml;
        dirty();
        notice("Configurazione salvata. Copia precedente in " + backup);
        break;
      }
    }
  draw();
}
document.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b || st.busy) return;
  if (b.dataset.page || b.dataset.group) {
    try {
      if (st.w && (b.dataset.page === "files" || b.dataset.group))
        st.doc = parse(st.xml);
      if (b.dataset.page) st.page = b.dataset.page;
      if (b.dataset.group) st.group = b.dataset.group;
      notice("");
      draw();
    } catch (err) {
      notice(err.message, true);
    }
  } else if (b.dataset.act || b.id === "open")
    work(() => act(b.dataset.act || "open"));
});
document.addEventListener("input", (e) => {
  if (e.target.id === "xml") {
    st.xml = e.target.value;
    dirty();
  }
});
document.addEventListener("change", (e) => {
  if (e.target.id === "category") {
    st.category = e.target.value;
    st.report = "";
    draw();
  }
});
document.addEventListener("keydown", (e) => {
  if (e.metaKey && e.key.toLowerCase() === "s") {
    e.preventDefault();
    if (st.w && st.dirty) work(() => act("save"));
  }
});
work(async () => {
  st.system = await rpc("system");
  try {
    st.disks = await rpc("disks");
  } finally {
    draw();
  }
});
