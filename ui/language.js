"use strict";
let language = "it";
function t(label) {
  return language === "en" &&
    Object.prototype.hasOwnProperty.call(english, label)
    ? english[label]
    : label;
}
function translatedError(message) {
  const s = String(message);
  for (const [it, en] of Object.entries(english)) {
    const from = language === "en" ? it : en,
      to = language === "en" ? en : it;
    if (s === from) return to;
    if (s.startsWith(from + ": "))
      return to + ": " + translatedError(s.slice(from.length + 2));
    if (from.endsWith(" ") && s.startsWith(from))
      return to + s.slice(from.length);
  }
  return s;
}
function languageUI(lang) {
  language = lang === "en" ? "en" : "it";
  document.documentElement.lang = language;
  document.querySelector("#language").value = language;
  document
    .querySelector("#language")
    .setAttribute("aria-label", language === "en" ? "Language" : "Lingua");
  document
    .querySelector("#nav")
    .setAttribute("aria-label", t("Navigazione principale"));
  document.querySelector("#local-label").textContent = t("Sessione locale");
  document.querySelector("#open-label").textContent = t("Apri cartella EFI");
  document.querySelector("#busy").textContent = t("Operazione in corso…");
}
