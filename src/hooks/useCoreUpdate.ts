import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface CoreUpdateStatus {
  bundled_version: string;
  required_version: string;
  latest_version: string | null;
  compatible: boolean;
  update_available: boolean;
  latest_app_compatible: boolean | null;
  compatibility_warning: string | null;
  check_error: string | null;
}

export function useCoreUpdate() {
  const [status, setStatus] = useState<CoreUpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const unlisten = listen<CoreUpdateStatus>('core-update-status', event => {
      if (active) { setStatus(event.payload); setError(null); }
    });
    // Register first, then load the cached startup result so an unopened Settings
    // window does not lose the automatic core check.
    unlisten.then(() => invoke<CoreUpdateStatus>('get_core_update_status'))
      .then(value => { if (active) setStatus(value); })
      .catch(reason => { if (active) setError(String(reason)); });
    return () => { active = false; void unlisten.then(stop => stop()).catch(() => undefined); };
  }, []);

  const check = useCallback(async () => {
    setChecking(true);
    setError(null);
    try {
      setStatus(await invoke<CoreUpdateStatus>('check_core_update'));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setChecking(false);
    }
  }, []);

  return { status, checking, error, check };
}
