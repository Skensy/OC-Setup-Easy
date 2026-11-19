const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const test = require("node:test");
const context = vm.createContext({
  window: {},
  document: { querySelector: () => ({ addEventListener() {} }) },
});
const script = (file) =>
  vm.runInContext(fs.readFileSync("ui/" + file, "utf8"), context);
script("navigation.js");
test("log search retains the original matching lines", () => {
  assert.equal(
    context.matchingLines("EFI\nPanic 1\nBOOT\npanic 2", "panic"),
    "Panic 1\npanic 2",
  );
});
Object.assign(context, { atob, btoa });
script("inspection.js");
test("SIP little-endian data and unsigned scan policy round trip", () => {
  for (const value of [0, 0x803, 0x80000000, 0xffffffff])
    for (const type of ["Dec", "Hex", "Base64"])
      assert.equal(
        context.decodeMask(context.encodeMask(value, type), type),
        value,
      );
  assert.equal(context.encodeMask(0x803, "Base64"), "AwgAAA==");
  for (const raw of ["-1", "3.4", "4294967296", ""])
    assert.throws(() => context.decodeMask(raw, "Dec"));
});
test("XML text deltas preserve Unicode and incomplete XML", () => {
  for (const [before, after] of [
    ["🍎 x", "🍏 x"],
    ["<", "<🍎"],
    ["🍎a", "a"],
    ["a🍎", "a"],
    ["<dict/>", "<d"],
  ]) {
    const { offset, old, next } = context.textChange(before, after);
    assert.equal(before.slice(offset, offset + old.length), old);
    assert.equal(
      before.slice(0, offset) + next + before.slice(offset + old.length),
      after,
    );
    assert.doesNotThrow(() => encodeURIComponent(old + next));
  }
});
