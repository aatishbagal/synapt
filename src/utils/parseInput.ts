import { ParsedInput } from '../types';

function looksLikeExpression(s: string): boolean {
  const hasDigit = /[0-9]/.test(s);
  const hasOp =
    /[+*/(]/.test(s) ||
    (s.match(/-/g) || []).length > 1 ||
    (s.includes('-') && !/^-/.test(s.trim()));
  return hasDigit && hasOp;
}

export function parseInput(raw: string): ParsedInput {
  if (raw.startsWith('/')) {
    return { raw, mode: 'folder', query: raw.slice(1).trim(), deviceName: null };
  }
  if (raw.toLowerCase().startsWith('@settings')) {
    return { raw, mode: 'settings', query: '', deviceName: null };
  }
  if (raw.startsWith('@')) {
    const rest = raw.slice(1);
    const spaceIdx = rest.indexOf(' ');
    if (spaceIdx === -1) {
      return { raw, mode: 'remote', query: '', deviceName: rest };
    }
    return {
      raw,
      mode: 'remote',
      query: rest.slice(spaceIdx + 1).trim(),
      deviceName: rest.slice(0, spaceIdx),
    };
  }
  if (looksLikeExpression(raw)) {
    return { raw, mode: 'calc', query: raw.trim(), deviceName: null };
  }
  return { raw, mode: 'local', query: raw.trim(), deviceName: null };
}
