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

/** @param {string} shortcut @param {string} [platform] */
export function displayShortcut(shortcut, platform = "unknown") {
  const primary = platform === "macos" ? "Cmd" : ["windows", "linux"].includes(platform) ? "Ctrl" : "Ctrl/Cmd";
  const system = platform === "macos" ? "Cmd" : platform === "windows" ? "Win" : "Super";
  const modifiers = { commandorcontrol: primary, cmdorctrl: primary, control: "Ctrl", ctrl: "Ctrl", super: system, command: "Cmd", cmd: "Cmd", alt: platform === "macos" ? "Option" : "Alt", shift: "Shift" };
  return shortcut.split("+").map((token) => {
    const key = token.trim();
    return modifiers[/** @type {keyof typeof modifiers} */ (key.toLowerCase())] ?? key.replace(/^Key([A-Z])$/, "$1").replace(/^Digit([0-9])$/, "$1");
  }).join(" + ");
}
