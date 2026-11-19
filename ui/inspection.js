"use strict";
const scanBits = [
  [0, "Blocca filesystem"],
  [1, "Blocca dispositivi"],
  [8, "APFS"],
  [9, "HFS+"],
  [10, "Partizione di sistema EFI"],
  [11, "NTFS"],
  [12, "Radice Linux"],
  [13, "Dati Linux"],
  [14, "XBOOTLDR"],
  [16, "SATA"],
  [17, "SAS"],
  [18, "SCSI"],
  [19, "NVMe"],
  [20, "ATAPI"],
  [21, "USB"],
  [22, "FireWire"],
  [23, "SD"],
  [24, "PCI"],
];
const csrBits = [
  ["UNTRUSTED_KEXTS", "Kext senza firma"],
  ["UNRESTRICTED_FS", "Filesystem senza restrizioni"],
  ["TASK_FOR_PID", "Accesso ai task di altri processi"],
  ["KERNEL_DEBUGGER", "Debugger del kernel"],
  ["APPLE_INTERNAL", "Funzioni interne Apple"],
  ["UNRESTRICTED_DTRACE", "DTrace senza restrizioni"],
  ["UNRESTRICTED_NVRAM", "Scrittura NVRAM senza restrizioni"],
  ["DEVICE_CONFIGURATION", "Configurazione dispositivo"],
  ["ANY_RECOVERY_OS", "Qualsiasi sistema di recupero"],
  ["UNAPPROVED_KEXTS", "Kext senza approvazione"],
  ["EXECUTABLE_POLICY_OVERRIDE", "Eccezioni alla politica di esecuzione"],
  ["UNAUTHENTICATED_ROOT", "Volume di sistema senza autenticazione"],
  ["RESEARCH_GUESTS", "Guest di ricerca"],
].map(([symbol, description], i) => [i, description, "CSR_ALLOW_" + symbol]);

function decodeMask(raw, format) {
  const s = raw.trim();
  if (format === "Base64") {
    const bytes = atob(s);
    if (bytes.length !== 4) throw Error();
    return new DataView(
      Uint8Array.from(bytes, (c) => c.charCodeAt(0)).buffer,
    ).getUint32(0, true);
  }
  if (!(format === "Hex" ? /^(?:0x)?[0-9a-f]{1,8}$/i : /^\d+$/).test(s))
    throw Error();
  const value = parseInt(s, format === "Hex" ? 16 : 10);
  if (value > 0xffffffff) throw Error();
  return value;
}
function encodeMask(value, format) {
  if (format === "Hex")
    return "0x" + value.toString(16).padStart(8, "0").toUpperCase();
  if (format === "Base64") {
    const data = new DataView(new ArrayBuffer(4));
    data.setUint32(0, value, true);
    return btoa(String.fromCharCode(...new Uint8Array(data.buffer)));
  }
  return String(value);
}
function maskControl(input, csr) {
  const box = document.createElement("details"),
    summary = document.createElement("summary");
  box.className = "mask";
  summary.textContent = t("Calcolatore bitmask");
  box.append(summary);
  const panel = document.createElement("div");
  panel.className = "mask-panel";
  box.append(panel);
  const source = csr ? "Base64" : "Dec";
  const fields = new Map(),
    bits = new Map();
  const status = document.createElement("small");
  status.setAttribute("role", "status");
  function refresh(skip) {
    let value;
    try {
      value = decodeMask(input.value, source);
      status.textContent = "";
    } catch {
      status.textContent = t("Valore a 32 bit non valido");
    }
    for (const [format, el] of fields)
      if (el !== skip)
        el.value = value === undefined ? "" : encodeMask(value, format);
    for (const [i, el] of bits) {
      el.disabled = value === undefined;
      el.checked = !!(value & (1 << i));
    }
  }
  let editing;
  function apply(value, el) {
    editing = el;
    input.value = encodeMask(value, source);
    input.dispatchEvent(new Event("input"));
    editing = null;
  }
  for (const format of ["Dec", "Hex", "Base64"]) {
    const label = document.createElement("label"),
      el = document.createElement("input");
    label.textContent = format;
    label.append(el);
    panel.append(label);
    fields.set(format, el);
    el.oninput = () => {
      try {
        apply(decodeMask(el.value, format), el);
      } catch {
        status.textContent = t("Valore a 32 bit non valido");
      }
    };
    el.onblur = () => refresh();
  }
  panel.append(status);
  for (const [i, name, symbol] of csr ? csrBits : scanBits) {
    const label = document.createElement("label"),
      el = document.createElement("input");
    el.type = "checkbox";
    label.title =
      (symbol ? symbol + " · " : "") + encodeMask((1 << i) >>> 0, "Hex");
    const description =
      !csr && i >= 8 ? t("Consenti scansione") + " · " + t(name) : t(name);
    const caption = document.createElement("span");
    caption.textContent = description;
    if (symbol) {
      const code = document.createElement("small");
      code.textContent = symbol;
      caption.append(code);
    }
    label.append(el, caption);
    panel.append(label);
    bits.set(i, el);
    el.onchange = () =>
      apply((decodeMask(input.value, source) ^ (1 << i)) >>> 0);
  }
  input.addEventListener("input", () => refresh(editing));
  refresh();
  return box;
}

function livePanel() {
  const rows = st.live || [];
  return (
    '<section class="card"><div class="head"><h2>' +
    t("NVRAM attiva e su disco") +
    "</h2>" +
    btn(t("Leggi NVRAM"), "live", "", "reload") +
    "</div>" +
    rows
      .map(
        (r) =>
          '<div class="nvrow"><strong>' +
          esc(r.key) +
          '</strong><small class="paths">' +
          esc(r.guid) +
          "</small>" +
          pairs([
            [t("NVRAM attiva"), r.live ?? "—"],
            ["config.plist", r.config ?? "—"],
          ]) +
          (r.different
            ? '<p class="warn">' +
              t(
                "Il valore attivo differisce. Se questa EFI avvia il Mac e la differenza persiste, valuta Reset NVRAM dal picker OpenCore.",
              ) +
              "</p>"
            : "") +
          "</div>",
      )
      .join("") +
    "</section>"
  );
}
function readiness() {
  let doc;
  try {
    doc = parse(st.xml);
  } catch {
    return "";
  }
  const config = doc.documentElement.firstElementChild;
  const at = (...path) => path.reduce(get, config);
  const boot = at(...bootPath)?.textContent || "";
  const enabled = Array.from(at("Kernel", "Add")?.children || []).filter(
    (k) =>
      get(k, "Enabled")?.tagName === "true" &&
      ["Any", "", "x86_64"].includes(get(k, "Arch")?.textContent || ""),
  );
  const target = st.targetDarwin || Number(st.system.kernel?.split(".")[0]) + 1;
  const version = (s) => s.split(".").map(Number);
  const before = (a, b) => {
    for (let i = 0; i < 3; i++)
      if ((a[i] || 0) !== (b[i] || 0)) return (a[i] || 0) < (b[i] || 0);
    return false;
  };
  const limits = enabled.filter((k) => {
    const max = get(k, "MaxKernel")?.textContent;
    return max && before(version(max), [target, 0, 0]);
  });
  const secure = at("Misc", "Security", "SecureBootModel")?.textContent;
  const xhci = at("Kernel", "Quirks", "XhciPortLimit")?.tagName;
  const isRestrict = (path) =>
    path?.split("/").pop()?.toLowerCase() === "restrictevents.kext";
  const restrict = enabled.find((k) =>
    isRestrict(get(k, "BundlePath")?.textContent),
  );
  const installed = st.w.inventory.Kexts.some((f) => isRestrict(f.path));
  const restrictStatus =
    restrict && !installed
      ? "File assente"
      : !installed
        ? "Assente o disattivato"
        : !restrict
          ? "Disattivato"
          : limits.includes(restrict)
            ? "Fuori intervallo per la destinazione"
            : "Attivo";
  const guid = "4D1FDA02-38C7-4A6A-9CC6-4BCCA8B30102";
  const variable = at("NVRAM", "Add", guid, "revpatch");
  let flags = variable?.textContent;
  if (variable?.tagName === "data") {
    try {
      flags = atob(flags).replace(/\0+$/, "");
    } catch {
      flags = t("Base64 non valido");
    }
  }
  const patch = boot.match(/(?:^|\s)revpatch=([^\s]+)/)?.[1] || flags;
  return (
    '<section class="card"><h2>' +
    t("Prontezza aggiornamento macOS") +
    "</h2><label>" +
    t("Versione Darwin di destinazione") +
    ' <input type="number" min="1" max="999" id="targetDarwin" value="' +
    (Number.isFinite(target) ? target : "") +
    '"></label>' +
    pairs([
      [
        "SecureBootModel",
        secure === "Disabled"
          ? t("Disabilitato · verificare i requisiti OTA")
          : secure,
      ],
      ["RestrictEvents", t(restrictStatus)],
      ["revpatch", patch || t("Impostazione predefinita")],
      [
        "MaxKernel",
        limits
          .map(
            (k) =>
              get(k, "BundlePath")?.textContent +
              " ≤ " +
              get(k, "MaxKernel").textContent,
          )
          .join(", ") || t("Nessun limite inferiore alla destinazione"),
      ],
      [
        "XhciPortLimit",
        xhci === "true"
          ? t("Attivo · verificare la mappa USB")
          : xhci === "false"
            ? t("Disattivato")
            : t("Non disponibile"),
      ],
    ]) +
    "</section>"
  );
}
