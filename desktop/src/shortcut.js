/** @param {KeyboardEvent} event */
export function shortcutFromEvent(event) {
  if (["Control", "Shift", "Alt", "Meta"].includes(event.key)) return null;
  const modifiers = [];
  if (event.ctrlKey) modifiers.push("Control");
  if (event.metaKey) modifiers.push("Super");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (modifiers.length === 0) return null;
  const allowed = /^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-9]|2[0-4])|Space|Enter|Tab|Backspace|Delete|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right))$/;
  if (!allowed.test(event.code)) return null;
  return [...modifiers, event.code].join("+");
}

/** @param {string} shortcut */
export function displayShortcut(shortcut) {
  return shortcut.replace("CommandOrControl", "Ctrl/Cmd").replace("Control", "Ctrl").replace("Super", "Cmd").replace(/Key([A-Z])/g, "$1").replace(/Digit([0-9])/g, "$1").replaceAll("+", " + ");
}
