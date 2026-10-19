// Hotkey display and recording helpers shared by Settings and the setup flow.

/** Title-case a stored hotkey string such as `ctrl+space` for display. */
export function prettyHotkey(hotkey: string): string {
  return hotkey
    .split('+')
    .map(part => (part.length <= 2 ? part.toUpperCase() : part.charAt(0).toUpperCase() + part.slice(1).toLowerCase()))
    .join('+');
}

/** Map a browser KeyboardEvent.code to a token the shortcut parser accepts. */
export function keyLabel(code: string): string {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1];
  const digit = /^Digit(\d)$/.exec(code);
  if (digit) return digit[1];
  if (code.startsWith('Arrow')) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  switch (code) {
    case 'Space': return 'Space';
    case 'Enter': return 'Enter';
    case 'Tab': return 'Tab';
    case 'Backspace': return 'Backspace';
    case 'Minus': return '-';
    case 'Equal': return '=';
    case 'Comma': return ',';
    case 'Period': return '.';
    case 'Slash': return '/';
    case 'Semicolon': return ';';
    case 'Quote': return "'";
    case 'BracketLeft': return '[';
    case 'BracketRight': return ']';
    case 'Backslash': return '\\';
    case 'Backquote': return '`';
    default: return code;
  }
}
