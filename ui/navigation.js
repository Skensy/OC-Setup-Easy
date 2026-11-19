"use strict";
function matchingLines(text, query) {
  return text
    .split("\n")
    .filter((line) => line.toLowerCase().includes(query))
    .join("\n");
}
function filterView() {
  const query = $("#find").value.trim().toLowerCase();
  if (st.page === "files") {
    document.querySelectorAll(".component").forEach((row) => {
      row.hidden = !row
        .querySelector(".entry")
        .textContent.toLowerCase()
        .includes(query);
    });
  } else if (st.page === "system" && st.logs) {
    document.querySelectorAll("#content details").forEach((row, i) => {
      const file = st.logs[i];
      const [highlights, log] = row.querySelectorAll("pre");
      highlights.textContent = matchingLines(file.highlights.join("\n"), query);
      log.textContent = matchingLines(file.text, query);
      row.hidden = !!query && !log.textContent;
      if (query) row.open = !row.hidden;
    });
  } else if (st.page === "config" && st.w) {
    document.querySelector("#matches")?.remove();
    for (const el of document.querySelectorAll(
      "#xml, #sections, #tree, label[for=xml]",
    ))
      el.hidden = !!query;
    if (!query) return;
    const results = document.createElement("div");
    results.id = "matches";
    results.className = "table";
    $("#content").append(results);
    let doc;
    try {
      doc = st.mode === "xml" ? parse(st.xml) : st.doc;
    } catch {
      results.textContent = matchingLines(st.xml, query);
      return;
    }
    function visit(node, path) {
      const dict = node.tagName === "dict",
        array = node.tagName === "array";
      const value =
        dict || array
          ? ""
          : ["true", "false"].includes(node.tagName)
            ? node.tagName
            : node.textContent;
      if (
        path.length &&
        (path.join("/").toLowerCase().includes(query) ||
          value.toLowerCase().includes(query))
      ) {
        const b = document.createElement("button");
        b.className = "match";
        b.textContent = path.join(" / ") + (value ? "   " + value : "");
        b.onclick = () => {
          st.queries.config = "";
          st.doc = doc;
          st.mode = "fields";
          st.section = path[0];
          st.focusPath = path;
          draw();
        };
        results.append(b);
      }
      if (dict)
        for (const [key, child] of entries(node)) visit(child, [...path, key]);
      if (array)
        Array.from(node.children).forEach((child, i) =>
          visit(child, [...path, i]),
        );
    }
    visit(doc.documentElement.firstElementChild, []);
    if (!results.children.length) results.textContent = t("Nessun risultato");
  }
}
window.ocShortcut = async (key) => {
  if (st.busy || document.querySelector(".overlay")) return;
  if (/^[1-6]$/.test(key))
    document.querySelectorAll("[data-page]")[Number(key) - 1].click();
  if (key === "f") {
    if ($("#findbar").hidden)
      document
        .querySelector(st.w ? '[data-page="config"]' : '[data-page="system"]')
        .click();
    if (st.page === "system" && !st.logs) await work(() => act("logs"));
    $("#find").focus();
    $("#find").select();
  }
  if (key === "e") {
    const disk =
      st.disks.find((d) => d.uuid === st.volumeUUID) || selectedDisk();
    if (disk)
      work(() =>
        act((disk.mount ? "unmount-" : "mount-") + st.disks.indexOf(disk)),
      );
    else {
      document.querySelector('[data-page="disks"]').click();
      document.querySelector("[data-volume]")?.focus();
    }
  }
};
document.querySelector("#find").addEventListener("input", (e) => {
  st.queries ||= {};
  st.queries[st.page] = e.target.value;
  filterView();
});

// The textarea exposes UTF-16 offsets. Keep surrogate pairs together on the wire.
function textChange(before, after) {
  let start = 0;
  while (
    start < before.length &&
    start < after.length &&
    before[start] === after[start]
  )
    start++;
  const low = (n) => n >= 0xdc00 && n <= 0xdfff;
  if (low(before.charCodeAt(start)) || low(after.charCodeAt(start))) start--;
  let end = before.length,
    nextEnd = after.length;
  while (
    end > start &&
    nextEnd > start &&
    before[end - 1] === after[nextEnd - 1]
  ) {
    end--;
    nextEnd--;
  }
  if (low(before.charCodeAt(end)) || low(after.charCodeAt(nextEnd))) {
    end++;
    nextEnd++;
  }
  return {
    offset: start,
    old: before.slice(start, end),
    next: after.slice(start, nextEnd),
  };
}
