/**
 * Turns an error from a Tauri command into a plain-language sentence that
 * says what went wrong and, where possible, what to do next. Backend errors
 * are written for logs ("peer not online", "connection refused"), so they are
 * matched against known causes; anything unrecognised falls back to the
 * caller's sentence for the action that failed. The raw error still goes to
 * the console for diagnosis.
 */
const KNOWN_CAUSES: [RegExp, string][] = [
  [
    /not online|offline|unreachable|connection refused|connection reset|broken pipe|no route|could not connect|failed to connect/i,
    'The other device is offline or unreachable. Check that it is awake, running Synapt, and on the same network.',
  ],
  [/timed out|timeout|deadline/i, 'The other device took too long to respond. Try again.'],
  [/not trusted|peer not found|unknown peer/i, 'That device is no longer paired. Pair it again in Settings.'],
  [/no pending pair/i, 'The pairing request expired. Start pairing again.'],
  [/app not found/i, 'That app is no longer installed.'],
  [/no such file|not a file|file not found|os error 2\b/i, 'The file could not be found. It may have been moved or deleted.'],
  [/permission denied|access is denied|not permitted|os error 13\b/i, 'Synapt does not have permission to access that location.'],
  [/no space|disk full|os error 28\b/i, 'There is not enough disk space to finish.'],
];

/** True when the backend already wrote a sentence meant for people. */
function isUserFacing(message: string): boolean {
  return /^[A-Z][\s\S]*[.!?]$/.test(message.trim());
}

/** Plain-language form of an error. Pure, so it is safe to call while rendering. */
export function plainError(error: unknown, fallback: string): string {
  const message = error instanceof Error ? error.message : String(error ?? '');
  for (const [pattern, sentence] of KNOWN_CAUSES) {
    if (pattern.test(message)) return sentence;
  }
  return isUserFacing(message) ? message.trim() : fallback;
}

/** Logs the raw error, then returns its plain-language form. Use in catch handlers. */
export function describeError(error: unknown, fallback: string): string {
  console.error(fallback, error);
  return plainError(error, fallback);
}
