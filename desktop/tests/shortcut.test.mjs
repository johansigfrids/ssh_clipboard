import assert from "node:assert/strict";
import test from "node:test";
import { shortcutFromEvent, displayShortcut } from "../src/shortcut.js";

test("shortcut recorder preserves physical key and modifiers", () => {
  assert.equal(shortcutFromEvent({ key: "V", code: "KeyV", ctrlKey: true, shiftKey: true }), "Control+Shift+KeyV");
  assert.equal(shortcutFromEvent({ key: "k", code: "KeyK", metaKey: true, altKey: true }), "Super+Alt+KeyK");
});

test("shortcut recorder does not capture typing or standalone modifiers", () => {
  for (const event of [{ key: "a", code: "KeyA" }, { key: "Control", code: "ControlLeft", ctrlKey: true }, { key: ";", code: "Semicolon", ctrlKey: true }]) {
    assert.equal(shortcutFromEvent(event), null);
  }
});

test("display shortcut keeps the registered key visible", () => {
  assert.equal(displayShortcut("Control+Shift+KeyV"), "Ctrl + Shift + V");
  assert.equal(displayShortcut("control+alt+KeyC"), "Ctrl + Alt + C");
});

test("shortcut labels follow the platform without rewriting explicit modifiers or stored bindings", () => {
  const stored="CommandOrControl+Shift+KeyV";
  assert.equal(displayShortcut(stored,"macos"),"Cmd + Shift + V");
  assert.equal(displayShortcut(stored,"windows"),"Ctrl + Shift + V");
  assert.equal(displayShortcut(stored,"linux"),"Ctrl + Shift + V");
  assert.equal(displayShortcut("Super+Alt+KeyK","macos"),"Cmd + Option + K");
  assert.equal(displayShortcut("Super+KeyK","windows"),"Win + K");
  assert.equal(displayShortcut("Super+KeyK","linux"),"Super + K");
  assert.equal(displayShortcut("Control+KeyK","macos"),"Ctrl + K");
});
