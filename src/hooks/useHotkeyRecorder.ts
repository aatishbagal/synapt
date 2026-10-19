import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { keyLabel } from '../utils/hotkey';
import { describeError } from '../utils/errors';

/**
 * Records a new global shortcut from the next key combination pressed while
 * `recording` is on, registers it through `set_hotkey`, and reports the saved
 * combination to `onSaved`. Escape cancels without changing anything.
 */
export function useHotkeyRecorder(onSaved: (combo: string) => void) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pendingHotkey = useRef<string | null>(null);
  // Held in a ref so a new callback each render does not restart recording.
  const onSavedRef = useRef(onSaved);
  onSavedRef.current = onSaved;

  useEffect(() => {
    if (!recording) return;
    // Keys pressed while recording belong to the recorder alone, so Escape
    // cancels recording rather than also closing the window.
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape') {
        pendingHotkey.current = null;
        setRecording(false);
        return;
      }
      // Ignore lone modifier presses; wait for a main key.
      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
      const parts: string[] = [];
      if (e.ctrlKey) parts.push('Ctrl');
      if (e.shiftKey) parts.push('Shift');
      if (e.altKey) parts.push('Alt');
      if (e.metaKey) parts.push('Super');
      parts.push(keyLabel(e.code));
      pendingHotkey.current = parts.join('+');
    };
    const onKeyUp = async (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const combo = pendingHotkey.current;
      if (!combo) return;
      pendingHotkey.current = null;
      setError(null);
      try {
        await invoke('set_hotkey', { hotkey: combo });
        onSavedRef.current(combo);
      } catch (cause) {
        setError(describeError(cause, 'That shortcut could not be registered. Try a different combination.'));
      }
      setRecording(false);
    };
    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('keyup', onKeyUp, true);
    return () => {
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
    };
  }, [recording]);

  return { recording, setRecording, error };
}
