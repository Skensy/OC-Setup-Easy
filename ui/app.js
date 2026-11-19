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
  undo: "M9 4L3 10l6 6 M4 10h9a7 7 0 0 1 7 7v3",
  redo: "M15 4l6 6-6 6m5-6h-9a7 7 0 0 0-7 7v3",
  diff: "M3 3h7v18H3z M14 3h7v18h-7z M5 8h3m8 6h3",
  sliders: "M5 3v4m0 4v10m7-18v10m0 4v4m7-18v2m0 4v12 M2 9h6m1 6h6m1-8h6",
  sort: "M4 6h16M4 12h11M4 18h6",
  eject: "M3 15l9-11 9 11zM3 19h18",
  download: "M12 3v12m-5-5 5 5 5-5M3 16v5h18v-5",
  export: "M12 15V2m-5 5 5-5 5 5M7 9H3v12h18V9h-4",
  search: "M16 16l5 5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
  up: "M5 14l7-7 7 7",
  down: "M5 10l7 7 7-7",
  usb: "M9 3h6v5H9z M7 8h10v11l-3 3h-4l-3-3z",
  external: "M4 5h16v14H4z M7 15h6m3 0h1",
  acpi: "M5 2h10l4 4v16H5z M9 11h6m-6 4h6",
  driver: "M4 6h16v11H4z M8 17v3m4-3v3m4-3v3 M8 10h8",
  warn: "M12 3l10 17H2z M12 9v4m0 3v1",
  close: "M6 6l12 12M6 18L18 6",
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
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round" aria-hidden="true"><path d="' +
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
  source: "",
  treeEdited: false,
  get xml() {
    if (this.treeEdited) {
      this.source = new XMLSerializer().serializeToString(this.doc);
      this.treeEdited = false;
    }
    return this.source;
  },
  set xml(text) {
    this.source = text;
    this.treeEdited = false;
  },
  original: "",
  doc: null,
  dirty: false,
  mode: "fields",
  section: "",
  group: "Kexts",
  validation: null,
  reports: {},
  history: {},
  changed: {},
  category: "graphics",
  busy: false,
};
const groups = {
  Kexts: ["Kernel", "Add", "BundlePath"],
  ACPI: ["ACPI", "Add", "Path"],
  Drivers: ["UEFI", "Drivers", "Path"],
  Tools: ["Misc", "Tools", "Path"],
};
const toolbarIcons = {
  undo: "undo",
  redo: "redo",
  diff: "diff",
  fields: "sliders",
  xml: "code",
  snapshot: "reload",
  sort: "sort",
  finder: "folder",
  backups: "folder",
  backup: "box",
  finish: "eject",
  checkKexts: "chip",
  checkUpdate: "download",
  sampleFile: "diff",
  sampleRelease: "diff",
  export: "export",
  logs: "terminal",
  close: "close",
};
const btn = (name, action, cls = "", glyph = toolbarIcons[action]) =>
  '<button type="button" data-act="' +
  action +
  '" class="' +
  cls +
  '">' +
  (glyph ? icon(glyph) : "") +
  esc(name) +
  "</button>";
const tag = (name, warn = false) =>
  '<span class="tag' +
  (warn ? " warn" : "") +
  '">' +
  icon(warn ? "warn" : "check") +
  esc(name) +
  "</span>";
const pairs = (entries) =>
  "<dl>" +
  entries
    .map(
      ([k, v]) =>
        "<div><dt>" +
        esc(k) +
        "</dt><dd>" +
        esc(v || t("Non disponibile")) +
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
  if (r.error) throw Error(translatedError(r.error));
  return r.cancelled ? null : r.ok;
}
let volumesChanged = false;
window.addEventListener("ocVolumesChanged", () => {
  volumesChanged = true;
  if (!st.busy) work(async () => {});
});
async function work(fn) {
  if (st.busy) return;
  st.busy = true;
  bridge("editingTask", { value: true });
  $("main").inert = true;
  $("aside").inert = true;
  document.body.classList.add("wait");
  $("#busy").hidden = false;
  try {
    st.beforeAction = st.patched ? st.xml : undefined;
    await fn();
  } catch (e) {
    notice(e.message || String(e), true);
  } finally {
    try {
      while (volumesChanged) {
        volumesChanged = false;
        st.disks = await rpc("disks");
        volumeStatus();
        if (st.page === "disks" || st.page === "home") draw();
      }
    } catch (e) {
      notice(e.message, true);
    }
    await bridge("editingTask", { value: false }).catch((e) =>
      notice(e.message, true),
    );
    st.busy = false;
    $("main").inert = false;
    $("aside").inert = false;
    document.body.classList.remove("wait");
    $("#busy").hidden = true;
  }
}
function historyButtons() {
  for (const op of ["undo", "redo"]) {
    const b = document.querySelector('[data-act="' + op + '"]');
    if (b) b.disabled = !st.history[op];
  }
}
function checkpoint(
  op = "checkpoint",
  data = { xml: st.xml, previous: st.beforeAction },
) {
  const w = st.w;
  st.patched = op === "patch";
  rpc(op, data)
    .then((h) => {
      if (w === st.w) {
        st.history = h;
        historyButtons();
      }
    })
    .catch((e) => notice(e.message, true));
  dirty(op === "patch" ? true : undefined);
}
function watchBanner() {
  const el = $("#watched");
  el.hidden = !st.w || !(st.changed.config || st.changed.folders);
  el.innerHTML = [
    st.changed.config &&
      t("config.plist è cambiato sul disco.") +
        " " +
        btn(t("Ricarica"), "reload"),
    st.changed.folders &&
      t("Le cartelle EFI sono cambiate.") +
        " " +
        btn(t("Sincronizza cartelle EFI"), "watchSync"),
  ]
    .filter(Boolean)
    .join(" ");
}
window.filesChanged = async (root, config, folders) => {
  const w = st.w;
  if (w?.root !== root) return;
  if (folders) st.changed.folders = true;
  if (config) {
    const changed = await rpc("diskChanged");
    if (st.w !== w) return;
    st.changed.config = changed;
  }
  watchBanner();
};
const pendingReports = new Set();
async function loadReport(category, refresh = false) {
  if (pendingReports.has(category) || (!refresh && st.reports[category]))
    return;
  pendingReports.add(category);
  try {
    let state;
    do {
      state = await rpc("report", { category, refresh });
      refresh = false;
      st.reports[category] = state;
      if (category === "pciDevices") fillPci();
      if (category === "categories" && state.state === "ready")
        st.system.reports = state.value;
      if (st.page === "system") draw();
      if (state.state === "loading")
        await new Promise((resolve) => setTimeout(resolve, 400));
    } while (state.state === "loading");
  } catch (e) {
    st.reports[category] = { state: "error", detail: e.message };
    if (st.page === "system") draw();
  } finally {
    pendingReports.delete(category);
  }
}
function dirty(value = st.xml !== st.original) {
  if (value !== st.dirty) {
    st.dirty = value;
    bridge("dirty", { value }).catch((e) => notice(e.message, true));
  }
  st.validation = null;
  const save = $("[data-act=save]");
  if (save) save.disabled = !value;
  const flag = $("#dirty");
  if (flag) flag.textContent = value ? t("Modifiche in bozza") : t("Su disco");
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
      t("XML non valido. Controlla la struttura del plist nella vista XML."),
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
        throw Error(t("Dizionario con chiave duplicata o valore mancante."));
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
  checkpoint();
}
function scalarPatch(path, old, node) {
  st.treeEdited = true;
  checkpoint("patch", { path, old, next: [node.tagName, node.textContent] });
}
function typed(doc, value) {
  const n = doc.createElement(
    typeof value === "boolean" ? String(value) : "string",
  );
  if (typeof value !== "boolean") n.textContent = value;
  return n;
}
function discard() {
  return !st.dirty || confirm(t("Scartare le modifiche in bozza?"));
}
function openData(data) {
  let doc = null;
  if (!data.watching) notice(t("Monitoraggio cartella non disponibile."), true);
  try {
    doc = parse(data.xml);
  } catch (err) {
    notice(err.message, true);
  }
  Object.assign(st, {
    logs: null,
    patched: false,
    live: null,
    queries: {},
    w: data,
    history: {},
    changed: {},
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
            {
              0: t("Area di lavoro"),
              2: t("Configurazione"),
              5: t("Diagnostica"),
            }[i] +
            "</span>"
          : "") +
        '<button class="nav" data-page="' +
        id +
        '" ' +
        (id === page ? 'aria-current="page"' : "") +
        ">" +
        icon(glyph) +
        t(title) +
        "</button>",
    )
    .join("");
  $("#crumb").textContent = "EFI  /  " + t(pages[page][0]);
  const searchLabel = {
    config: "Cerca chiavi o valori",
    files: "Filtra componenti",
    system: "Cerca nei log",
  }[page];
  $("#findbar").hidden = !searchLabel || (page !== "system" && !w);
  $("#find").placeholder = t(searchLabel || "Cerca");
  $("#find").setAttribute("aria-label", $("#find").placeholder);
  $("#find").value = st.queries?.[page] || "";
  volumeStatus();
  watchBanner();
  if (page === "home") {
    home();
    quickBoot();
    return;
  }
  let head =
    '<div class="heading"><div><h1>' + t(pages[page][0]) + "</h1></div></div>";
  if (w && ["config", "files", "validate"].includes(page))
    head +=
      '<div class="subline paths">' +
      esc(w.root) +
      '/config.plist · <span id="dirty">' +
      (st.dirty ? t("Modifiche in bozza") : t("Su disco")) +
      "</span></div>";
  let body;
  if (page === "disks") body = disks();
  else if (page === "system") body = systemPanel(s);
  else if (!w)
    body =
      '<div class="card empty"><img src="partition.svg" alt=""><h2>' +
      t("Apri una configurazione OpenCore") +
      "</h2><p>" +
      t(
        "Seleziona una cartella EFI o OC, anche da un backup. Per accedere a una partizione usa Volumi EFI.",
      ) +
      "</p>" +
      btn(t("Apri cartella EFI"), "open", "primary", "folder") +
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
  if (page === "files" && w && st.group === "Kexts") {
    const rows = inventoryRows();
    for (const el of document.querySelectorAll("[data-range]")) {
      const entry = rows[Number(el.dataset.range)].n;
      for (const k of ["MinKernel", "MaxKernel"]) {
        const n = get(entry, k);
        if (n)
          el.append(
            kernelSelect(k, n.textContent, (v) => {
              const old = [n.tagName, n.textContent];
              n.textContent = v;
              scalarPatch(
                ["Kernel", "Add", Number(el.dataset.range), k],
                old,
                n,
              );
            }),
          );
      }
    }
  }
  filterView();
  historyButtons();
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
    '<div class="hero"><div><span class="label">OpenCore / EFI</span><h1>' +
    t("Panoramica di sistema") +
    "</h1><p>" +
    esc(s.cpu || t("Processore non disponibile")) +
    '</p></div><img src="boot-flow.svg" alt=""></div><div class="stats">' +
    stat(
      t("Sistema operativo"),
      "macOS " + (s.os || "—"),
      "Build " + (s.build || "—"),
    ) +
    stat(
      t("Memoria fisica"),
      s.ram == null ? "—" : s.ram + " GiB",
      (s.cores || "—") + t(" core · ") + (s.threads || "—") + t(" thread"),
    ) +
    stat(
      t("Partizioni EFI"),
      String(st.disks.length).padStart(2, "0"),
      st.disks.filter((d) => d.mount).length + t(" montate"),
    ) +
    "</div>" +
    ('<div class="pair"><div class="card"><div class="head"><h2>' +
      t("Configurazione") +
      '</h2><span class="tile">') +
    icon("folder") +
    "</span></div><p>" +
    (w
      ? t("Cartella di lavoro aperta")
      : t("Scegli l’EFI su cui vuoi lavorare.")) +
    "</p>" +
    (w
      ? '<p class="paths">' +
        esc(w.root) +
        "</p>" +
        btn(t("Apri editor"), "go-config", "primary", "code") +
        " " +
        btn(t("Chiudi"), "close")
      : '<img class="workspace-disk" src="partition.svg" alt="">' +
        btn(t("Apri cartella EFI"), "open", "primary", "folder") +
        " " +
        btn(t("Volumi EFI"), "go-disks")) +
    ('</div><div class="card"><div class="head"><h2>' +
      t("Ambiente di avvio") +
      '</h2><span class="tile">') +
    icon("chip") +
    "</span></div>" +
    pairs([
      [t("OpenCore attivo"), s.oc],
      ["SMBIOS", s.model],
      ["Darwin", s.kernel],
      [t("Architettura"), s.arch],
    ]) +
    "</div></div>";
}
function disks() {
  const selected =
    st.disks.find((d) => d.uuid === st.volumeUUID) || selectedDisk();
  const inspector = selected
    ? '<div class="diskinfo"><strong>' +
      esc(selected.name || "EFI") +
      "</strong>" +
      tag(t(selected.mount ? "Montata" : "Smontata"), !selected.mount) +
      pairs([
        [t("Dispositivo"), selected.id],
        ["UUID", selected.uuid],
        [t("Punto di mount"), selected.mount || "—"],
      ]) +
      "</div>"
    : "<div><h2>GPT / EFI</h2><p>" + t("Seleziona un volume") + "</p></div>";
  return (
    '<div class="volume-layout"><section><div class="actions">' +
    btn(t("Aggiorna dischi"), "refresh", "", "reload") +
    '</div><div class="table">' +
    (st.disks
      .map(
        (d, i) =>
          '<div class="row"><input type="radio" name="volume" aria-label="' +
          esc(t("Seleziona un volume") + " " + d.id) +
          '" data-volume="' +
          i +
          '" ' +
          (selected?.uuid === d.uuid ? "checked" : "") +
          '><span class="tile">' +
          icon(d.bus === "USB" ? "usb" : d.ejectable ? "external" : "disk") +
          '</span><div class="entry"><strong>' +
          esc(d.name || "EFI") +
          "</strong><small>" +
          esc([d.id, d.model, d.bus].filter(Boolean).join(" · ")) +
          " · " +
          esc((d.size / 1e9).toFixed(2)) +
          " GB" +
          (d.mount ? " · " + esc(d.mount) : "") +
          "</small></div>" +
          tag(d.mount ? t("Montata") : t("Smontata"), !d.mount) +
          (d.mount
            ? btn(t("Apri EFI"), "volume-" + i) +
              btn(t("Smonta"), "unmount-" + i)
            : btn(t("Monta"), "mount-" + i, "primary")) +
          "</div>",
      )
      .join("") ||
      '<div class="empty">' +
        t("Nessuna partizione EFI rilevata.") +
        "</div>") +
    "</div>" +
    (st.w
      ? '<div class="actions">' +
        btn(t("Chiudi configurazione"), "close") +
        "</div>"
      : "") +
    '</section><section class="volume-map"><img src="partition.svg" alt="GPT / EFI">' +
    inspector +
    "</section></div>"
  );
}
const categories = {
  ioreg: "IORegistry",
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
  loadReport("categories");
  loadReport(st.category);
  const report = st.reports[st.category];
  const status =
    !report || report.state === "loading"
      ? t("Caricamento…")
      : report.state === "error"
        ? t("Errore report") + " · " + translatedError(report.detail)
        : t("Report pronto");
  return (
    '<div class="card"><h2>' +
    t("Hardware e avvio") +
    "</h2>" +
    pairs([
      ["CPU", s.cpu],
      [t("Core / thread"), s.cores + " / " + s.threads],
      [t("Memoria"), s.ram == null ? "" : s.ram + " GiB"],
      [t("Modello SMBIOS"), s.model],
      [t("Versione OpenCore (NVRAM)"), s.oc],
      [t("Argomenti di avvio"), s.boot],
      [t("Build macOS"), s.build],
    ]) +
    ('</div><div class="actions"><label for="category">' +
      t("Report macOS") +
      '</label><select id="category">') +
    Object.entries(categories)
      .map(
        ([k, v]) =>
          '<option value="' +
          k +
          '" ' +
          (st.category === k ? "selected" : "") +
          ">" +
          t(v) +
          "</option>",
      )
      .join("") +
    ('<optgroup label="' + t("Tutti i report di system_profiler") + '">') +
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
    btn(t("Aggiorna report"), "report", "", "terminal") +
    "</div>" +
    (esc(status) +
      (report?.state === "ready"
        ? codeText(report.value || t("Nessun dato restituito da macOS."))
        : "")) +
    '<section class="card"><h2>' +
    t("Log di avvio e kernel panic") +
    "</h2>" +
    btn(t("Leggi log"), "logs") +
    (st.logs && !st.logs.some((l) => /panic/i.test(l.path))
      ? '<div class="log-empty"><img src="log.svg" alt="">' +
        t("Nessun log di panic rilevato") +
        "</div>"
      : "") +
    (st.logs || [])
      .map(
        (l) =>
          "<details><summary>" +
          esc(l.path) +
          '</summary><pre class="warnings">' +
          esc(l.highlights.join("\n")) +
          "</pre>" +
          codeText(l.text) +
          "</details>",
      )
      .join("") +
    "</section>"
  );
}
function config() {
  return (
    '<div class="actions">' +
    btn(t("Campi"), "fields") +
    btn("XML", "xml") +
    btn(language === "en" ? "Undo" : "Annulla", "undo") +
    btn(t("Ripeti"), "redo") +
    btn(t("Verifica"), "verify", "", "shield") +
    btn(t("Mostra modifiche"), "diff") +
    btn(t("Salva plist"), "save", "primary", "save") +
    "</div>" +
    (st.mode === "xml"
      ? '<label for="xml" class="label">' +
        t("Sorgente XML") +
        '</label><textarea spellcheck="false" autocapitalize="off" id="xml"></textarea>'
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
    tree.textContent = t("Dizionario vuoto. Aggiungi chiavi nella vista XML.");
    return;
  }
  function append(parent, label, n, depth, path) {
    const type = n.tagName;
    if (type === "dict" || type === "array") {
      const wrap = document.createElement("details");
      wrap.open =
        depth === 0 || path.every((part, i) => st.focusPath?.[i] === part);
      const summary = document.createElement("summary");
      const children =
        type === "dict"
          ? entries(n)
          : Array.from(n.children, (el, i) => ["[" + i + "]", el]);
      summary.textContent = label + " · " + children.length;
      wrap.append(summary);
      parent.append(wrap);
      markField(summary, path);
      const fill = () => {
        for (const [k, v] of children)
          append(wrap, k, v, depth + 1, [
            ...path,
            type === "array" ? Number(k.slice(1, -1)) : k,
          ]);
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
        const old = [n.tagName, n.textContent];
        if (bool) {
          const next = typed(st.doc, input.checked);
          n.replaceWith(next);
          n = next;
        } else n.textContent = input.value;
        scalarPatch(path, old, n);
      });
      row.append(name, input);
      parent.append(row);
      markField(input, path);
      const csr =
        path.length === 4 &&
        path[0] === "NVRAM" &&
        path[1] === "Add" &&
        String(path[2]).toUpperCase() === bootPath[2] &&
        label === "csr-active-config" &&
        type === "data";
      if (
        csr ||
        (path.join("/") === "Misc/Security/ScanPolicy" && type === "integer")
      )
        row.after(maskControl(input, csr));
      if (path.join("/") === bootPath.join("/") && type === "string")
        row.after(bootControls(n, input));
      if (["MinKernel", "MaxKernel"].includes(label) && type === "string") {
        row.after(
          kernelSelect(label, n.textContent, (v) => {
            input.value = v;
            input.dispatchEvent(new Event("input"));
          }),
        );
      }
    }
  }
  if (st.section === "PlatformInfo" && get(node, "Generic")) {
    const actions = document.createElement("div");
    actions.className = "actions";
    actions.innerHTML =
      btn(t("Verifica SMBIOS"), "smbios") +
      btn(t("Genera seriale, MLB e UUID"), "generate");
    tree.append(actions);
  }
  if (st.section === "DeviceProperties" && get(node, "Add")?.tagName === "dict")
    pciPanel(tree, get(node, "Add"));
  if (st.section === "NVRAM") tree.insertAdjacentHTML("beforeend", livePanel());
  append(tree, st.section, node, 0, [st.section]);
  st.focusPath = null;
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
      file: physical.find((f) => pathKey(f.path) === pathKey(path)),
      enabled: n.tagName === "string" || get(n, "Enabled")?.tagName === "true",
    };
  });
  return configured.concat(
    physical
      .filter(
        (f) => !configured.some((c) => pathKey(c.path) === pathKey(f.path)),
      )
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
          t(
            {
              Drivers: "Driver UEFI",
              Tools: "Strumenti UEFI",
              Kexts: "Kext",
              ACPI: "ACPI",
            }[g],
          ) +
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
    (st.group === "ACPI"
      ? st.w.iasl
        ? btn(t("Compila sorgente DSL"), "compile", "", "acpi")
        : "<small>" + t("Compilatore iasl non disponibile") + "</small>"
      : "") +
    (st.group === "Kexts"
      ? btn(t("Copia un kext…"), "import", "", "box")
      : "") +
    (st.group === "Kexts" ? btn(t("Ordina per dipendenze"), "sort") : "") +
    btn(t("Sincronizza cartelle EFI"), "snapshot") +
    btn(t("Rileggi file"), "inventory", "", "reload") +
    btn(t("Apri nel Finder"), "finder") +
    btn(t("Mostra modifiche"), "diff") +
    btn(t("Salva plist"), "save", "primary", "save") +
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
                      ? t(" segue questo kext")
                      : t(" non attivo")),
                ];
              })
            : [];
        const detail = [
          r.file?.header
            ? [
                r.file.header.signature,
                r.file.header.oem,
                t("Revisione") + " " + r.file.header.revision,
                r.file.header.length + " B",
              ].join(" / ")
            : "",
          r.file?.version,
          r.file?.id,
          r.file?.bytes != null ? r.file.bytes + t(" byte") : "",
          r.file?.dependencies?.length
            ? t("Richiede: ") + r.file.dependencies.join(", ")
            : "",
        ]
          .filter(Boolean)
          .join(" · ");
        return (
          '<div class="row component"><i>' +
          icon(
            {
              Kexts: "chip",
              ACPI: "acpi",
              Drivers: "driver",
              Tools: "terminal",
            }[st.group],
          ) +
          '</i><div class="entry"><strong>' +
          esc(r.path || t("(percorso vuoto)")) +
          "</strong><small>" +
          esc(detail) +
          "</small>" +
          (orderIssues.length
            ? '<small class="error">' +
              esc(orderIssues.join(" · ")) +
              "</small>"
            : "") +
          "</div>" +
          (configured && st.group === "Kexts"
            ? '<details><summary>Kernel</summary><div data-range="' +
              i +
              '"></div></details>'
            : "") +
          tag(
            !r.file
              ? t("File assente")
              : !configured
                ? t("Solo su disco")
                : r.enabled
                  ? t("Attivo")
                  : t("Disattivato"),
            !r.file || !configured,
          ) +
          (configured
            ? (get(r.n, "Enabled")
                ? btn(
                    r.enabled ? t("Disattiva") : t("Attiva"),
                    "toggle-" + i,
                    "compact",
                  )
                : "") +
              (i > 0 ? btn(t("Sposta su"), "up-" + i, "compact", "up") : "") +
              (i + 1 < listNode().children.length
                ? btn(t("Sposta giù"), "down-" + i, "compact", "down")
                : "") +
              btn(t("Rimuovi voce"), "remove-" + i, "compact")
            : r.file.valid !== false
              ? btn(t("Registra"), "add-" + i, "compact")
              : t("Bundle incompleto")) +
          "</div>"
        );
      })
      .join("") ||
      '<div class="empty">' +
        t("Nessuna voce in questa categoria.") +
        "</div>") +
    "</div>"
  );
}
function componentEntry(doc, group, r) {
  let fields = { Comment: "", Enabled: false, Path: r.path };
  if (group === "Kexts")
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
  if (group === "Drivers")
    Object.assign(fields, {
      Arguments: "",
      LoadEarly: false,
      HideVerbose: false,
    });
  if (group === "Tools")
    Object.assign(fields, {
      Arguments: "",
      Auxiliary: true,
      Flavour: "Auto",
      FullNvramAccess: false,
      Name: r.path.split("/").pop(),
      RealPath: false,
      TextMode: false,
    });
  const dict = doc.createElement("dict");
  for (const [k, v] of Object.entries(fields)) {
    const key = doc.createElement("key");
    key.textContent = k;
    dict.append(key, typed(doc, v));
  }
  return dict;
}
function changeEntry(op, i) {
  const r = inventoryRows()[i],
    list = listNode();
  if (!list || list.tagName !== "array")
    throw Error(
      "Array " +
        groups[st.group].slice(0, 2).join(" → ") +
        t(" assente. Aggiungilo nella vista XML."),
    );
  if (op === "add") {
    list.append(componentEntry(st.doc, st.group, r));
  } else if (op === "remove") {
    if (
      !confirm(
        t("Rimuovere dal plist la voce ") +
          r.path +
          t("? Il file resterà sul disco."),
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
    '<div class="bootstrip"><img src="boot-flow.svg" alt="EFI, OpenCore, NVRAM, Kernel"></div><div class="pair"><div class="card"><div class="head"><h2>' +
    t("Controllo configurazione") +
    "</h2>" +
    (st.validation
      ? tag("ocvalidate " + st.validation.version, !st.validation.pass)
      : "") +
    ("</div><p>" +
      t(
        "Il validatore segue OpenCore.efi quando la release è in cache. Versione di riserva 1.0.8.",
      ) +
      "</p>") +
    btn(t("Verifica bozza"), "verify", "primary", "shield") +
    ('</div><div class="card"><h2>' +
      t("Archivio di ripristino") +
      "</h2><p>" +
      t(
        "Crea uno ZIP della cartella EFI su disco, oppure della sola OC se aperta separatamente.",
      ) +
      "</p>") +
    btn(t("Controlla aggiornamenti kext"), "checkKexts") +
    btn(t("Confronta con Sample.plist"), "sampleFile") +
    (st.w.releaseSample
      ? btn(t("Confronta con il Sample.plist della release"), "sampleRelease")
      : "") +
    btn(t("Controlla aggiornamenti OpenCore"), "checkUpdate") +
    btn(t("Esporta config anonimizzato"), "export") +
    btn(t("Crea backup ZIP"), "backup", "", "box") +
    " " +
    btn(t("Mostra backup"), "backups") +
    "</div></div>" +
    readiness() +
    livePanel() +
    (st.validation
      ? '<div class="actions">' +
        tag(
          st.validation.pass ? t("Verifica superata") : t("Anomalie rilevate"),
          !st.validation.pass,
        ) +
        "</div>" +
        diagnosticsView(st.validation)
      : "")
  );
}
async function act(a) {
  notice("");
  if (["addPci", "applyProperty", "refreshPci"].includes(a))
    return editProperty(a);
  if (a.startsWith("usbFix-")) {
    const bundle = st.validation.issues[Number(a.slice(7))].details[0];
    const plan = await rpc("usbPreview", { bundle, xml: st.xml });
    if (
      await preview(
        "Allinea SMBIOS nel kext",
        bundle +
          "\n" +
          plan.changes
            .map(([key, v]) => key + " · " + v + " → " + plan.model)
            .join("\n"),
        "Allinea SMBIOS nel kext",
      )
    ) {
      const backup = await rpc("usbApply");
      st.w.inventory = await rpc("inventory");
      st.validation = await rpc("validate", { xml: st.xml });
      draw();
      notice(t("Copia precedente in ") + backup);
    }
    return;
  }
  if (a.startsWith("jump-")) {
    const target = st.links[Number(a.slice(5))];
    st.doc = parse(st.xml);
    st.focusPath = target;
    st.section = target[0];
    st.page = "config";
    st.mode = "fields";
    draw();
    return;
  }
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
      notice(op === "mount" ? t("EFI montata.") : t("EFI smontata."));
    } else {
      changeEntry(op, i);
      return;
    }
  } else
    switch (a) {
      case "watchSync":
        st.changed.folders = false;
        await snapshot(true);
        break;
      case "reload":
        if (discard()) openData(await rpc("open", { volume: st.w.root }));
        break;
      case "undo":
      case "redo": {
        const h = await rpc(a);
        if (h.xml === undefined && !h.patch) break;
        st.patched = !!h.patch;
        if (h.patch) {
          const node = h.patch.path.reduce(
            (n, k) => (typeof k === "number" ? n.children[k] : get(n, k)),
            root(),
          );
          const [type, value] = h.patch.node;
          const next = st.doc.createElement(type);
          next.textContent = value;
          node.replaceWith(next);
          st.treeEdited = true;
          st.history = h;
          dirty();
          break;
        }
        st.xml = h.xml;
        try {
          st.doc = parse(st.xml);
        } catch {
          st.doc = null;
        }
        if (!h.fields || !st.doc) {
          st.mode = "xml";
          st.page = "config";
        }
        st.history = h;
        dirty();
        break;
      }
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
        Object.assign(st, {
          logs: null,
          w: null,
          xml: "",
          original: "",
          doc: null,
        });
        dirty();
        break;
      case "refresh":
        st.disks = await rpc("disks");
        break;
      case "report":
        loadReport(st.category, true);
        loadReport("categories", true);
        break;
      case "fields":
        st.doc = parse(st.xml);
        st.mode = "fields";
        break;
      case "xml":
        st.mode = "xml";
        break;
      case "live":
        st.live = await rpc("live");
        break;
      case "logs":
        st.logs = await rpc("logs");
        if (!st.logs.length) notice(t("Nessun log disponibile"));
        break;
      case "sampleFile":
      case "sampleRelease": {
        const rows = await rpc("sample", {
          xml: st.xml,
          choose: a === "sampleFile",
        });
        if (rows)
          await preview(
            "Confronta con Sample.plist",
            rows
              .map(
                (r) =>
                  (r.kind === "missing"
                    ? t("Assente nella bozza")
                    : t("Solo nella bozza")) +
                  " · " +
                  r.path +
                  (r.default !== undefined
                    ? " · " + t("Valore del Sample") + " " + r.default
                    : ""),
              )
              .join("\n") || t("Nessuna modifica"),
          );
        break;
      }
      case "checkKexts": {
        const rows = await rpc("checkKexts");
        const list = document.createElement("div");
        const boxes = [];
        for (const r of rows) {
          const label = document.createElement("label");
          const check = document.createElement("input");
          check.type = "checkbox";
          check.value = r.index;
          check.disabled = r.index == null;
          if (!check.disabled) boxes.push(check);
          label.append(
            check,
            document.createTextNode(
              r.name +
                " · " +
                (r.installed || "—") +
                " → " +
                (r.latest || "—") +
                (r.error ? " · " + translatedError(r.error) : ""),
            ),
          );
          list.append(label, document.createElement("br"));
        }
        if (!rows.length)
          list.textContent = t("Nessun kext supportato nella EFI");
        const hint = document.createElement("p");
        hint.textContent = t(
          "Prima della sostituzione verrà creato e verificato un backup ZIP completo della EFI.",
        );
        if (boxes.length) list.append(hint);
        if (
          await preview(
            "Controlla aggiornamenti kext",
            list,
            boxes.length ? "Aggiorna selezionati" : null,
          )
        ) {
          const selected = boxes
            .filter((b) => b.checked)
            .map((b) => Number(b.value));
          if (selected.length) {
            const path = await rpc("applyKexts", { selected });
            st.w.inventory = await rpc("inventory");
            st.validation = null;
            notice(t("Backup ZIP creato in ") + path);
          }
        }
        break;
      }
      case "checkUpdate": {
        const update = await rpc("checkUpdate");
        st.w.releaseSample = true;
        const text = [
          "OpenCore " + update.version,
          "SHA-256 · " + update.digest,
          ...update.paths,
          t(
            update.paths.length
              ? "Backup ZIP completo prima della sostituzione."
              : "Nessuna modifica",
          ),
        ].join("\n");
        if (
          await preview(
            t("Aggiornamento binari OpenCore"),
            text,
            update.paths.length ? t("Aggiorna binari") : null,
          )
        )
          notice(t("Backup ZIP creato in ") + (await rpc("applyUpdate")));
        break;
      }
      case "diff": {
        const d = await rpc("diff", { xml: st.xml });
        await preview(
          t("Mostra modifiche"),
          [...d.paths, d.xml].filter(Boolean).join("\n") ||
            t("Nessuna modifica"),
        );
        break;
      }
      case "export": {
        const path = await rpc("export", { xml: st.xml });
        if (path) notice(t("Copia esportata") + " · " + path);
        break;
      }
      case "smbios":
      case "generate": {
        const result = await rpc(a, { xml: st.xml });
        if (a === "smbios")
          await preview(
            t("Verifica SMBIOS"),
            [
              "SystemSerialNumber · " +
                t(result.serial ? "Coerente" : "Da verificare"),
              "MLB · " + t(result.board ? "Coerente" : "Da verificare"),
            ].join("\n"),
          );
        else if (
          await preview(
            t("Genera seriale, MLB e UUID"),
            Object.entries(result)
              .map(([k, v]) => k + " · " + v)
              .join("\n"),
            t("Applica alla bozza"),
          )
        ) {
          const g = get(get(root(), "PlatformInfo"), "Generic");
          for (const [k, v] of Object.entries(result)) {
            const n = get(g, k);
            if (n) n.replaceWith(typed(st.doc, v));
            else {
              const key = st.doc.createElement("key");
              key.textContent = k;
              g.append(key, typed(st.doc, v));
            }
          }
          sync();
        }
        break;
      }
      case "sort":
      case "snapshot":
        await snapshot(a === "snapshot");
        break;
      case "inventory":
        st.w.inventory = await rpc("inventory");
        break;
      case "finder":
      case "backups":
        await rpc(a);
        break;
      case "compile": {
        const list = listNode();
        if (list?.tagName !== "array")
          throw Error(t("Manca ACPI/Add nella bozza"));
        const result = await rpc("compile");
        if (!result) break;
        st.w.inventory.ACPI = st.w.inventory.ACPI.filter(
          (f) => pathKey(f.path) !== pathKey(result.path),
        );
        st.w.inventory.ACPI.push(result);
        if (
          !inventoryRows().some(
            (r) => r.n && pathKey(r.path) === pathKey(result.path),
          )
        ) {
          list.append(componentEntry(st.doc, "ACPI", { path: result.path }));
          sync();
        }
        notice(t("Tabella compilata e registrata nella bozza"));
        break;
      }
      case "import": {
        const data = await rpc("import");
        if (data) {
          st.w.inventory = data;
          notice(
            t(
              "Bundle copiato. Registra e attiva le voci desiderate, poi salva il plist.",
            ),
          );
        }
        break;
      }
      case "backup":
        notice(t("Backup ZIP creato in ") + (await rpc("backup")));
        break;
      case "verify":
        st.validation = await rpc("validate", { xml: st.xml });
        st.page = "validate";
        break;
      case "finish":
      case "save": {
        const disk = a === "finish" ? selectedDisk() : null;
        if (
          a === "finish" &&
          (!disk ||
            !confirm(
              t(
                disk.ejectable
                  ? "Salvare, smontare ed espellere il disco esterno?"
                  : "Salvare e smontare la EFI interna?",
              ),
            ))
        )
          return;
        const xml = st.xml;
        const doc = parse(xml);
        const check = await rpc("validate", { xml });
        if (!check.pass) {
          st.validation = check;
          st.page = "validate";
          draw();
          if (
            !confirm(
              t(
                "ocvalidate segnala anomalie. Salvare comunque questa configurazione?",
              ),
            )
          )
            return;
        }
        if (st.xml !== xml)
          throw Error(t("La bozza è cambiata durante il salvataggio."));
        const backup = st.dirty ? await rpc("save", { xml }) : null;
        if (st.xml === xml) st.doc = doc;
        st.original = xml;
        st.live = null;
        dirty();
        if (backup)
          notice(t("Configurazione salvata. Copia precedente in ") + backup);
        if (disk) {
          if (st.dirty)
            throw Error(t("La bozza è cambiata durante il salvataggio."));
          const result = await rpc("finish", { id: disk.id, uuid: disk.uuid });
          st.disks = result.disks;
          Object.assign(st, {
            w: null,
            xml: "",
            original: "",
            doc: null,
            logs: null,
          });
          dirty();
          notice(
            t("EFI smontata.") + (result.warning ? " · " + result.warning : ""),
            !!result.warning,
          );
        }
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
document.addEventListener(
  "beforeinput",
  (e) => {
    if (st.busy && e.target.closest("main, aside")) e.preventDefault();
  },
  true,
);
document.addEventListener("input", (e) => {
  if (e.target.id === "xml") {
    const before = st.xml;
    const change = textChange(before, e.target.value);
    if (st.patched) change.previous = before;
    st.xml = e.target.value;
    checkpoint("xmlEdit", change);
  }
});
document.addEventListener("change", (e) => {
  if (e.target.hasAttribute("data-volume")) {
    st.volumeUUID = st.disks[Number(e.target.dataset.volume)].uuid;
    draw();
  }
  if (e.target.id === "language") {
    const lang = e.target.value;
    work(async () => {
      languageUI(await rpc("language", { value: lang }));
      notice("");
      draw();
    });
    return;
  }
  if (
    e.target.id === "targetDarwin" &&
    e.target.validity.valid &&
    e.target.value
  ) {
    st.targetDarwin = Number(e.target.value);
    draw();
  }
  if (e.target.id === "category") {
    st.category = e.target.value;
    draw();
  }
});
document.addEventListener("keydown", (e) => {
  if (
    e.metaKey &&
    e.key.toLowerCase() === "z" &&
    st.w &&
    !e.target.closest("input,textarea,[contenteditable=true]")
  ) {
    e.preventDefault();
    work(() => act(e.shiftKey ? "redo" : "undo"));
    return;
  }
  if (e.metaKey && e.key.toLowerCase() === "s") {
    e.preventDefault();
    if (st.w && st.dirty) work(() => act("save"));
  }
});
work(async () => {
  languageUI(await rpc("language"));
  st.system = await rpc("system");
  try {
    st.disks = await rpc("disks");
  } finally {
    draw();
  }
});
