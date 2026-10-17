import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface HotkeyInfo {
  wayland: boolean;
  registered: boolean;
  trigger: string | null;
  error: string | null;
}

export function useHotkeyInfo() {
  const [info, setInfo] = useState<HotkeyInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useCallback(async () => {
    try {
      setInfo(await invoke<HotkeyInfo>("get_hotkey_info"));
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
    window.addEventListener("focus", refresh);
    const timer = setInterval(() => { void refresh(); }, 2000);
    return () => {
      window.removeEventListener("focus", refresh);
      clearInterval(timer);
    };
  }, [refresh]);

  const configure = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("configure_hotkey");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
      await refresh();
    }
  }, [refresh]);

  return { info, error, busy, configure, refresh };
}
