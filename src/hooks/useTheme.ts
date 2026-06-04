import { invoke } from '@tauri-apps/api/core';

/**
 * Theme application and loading. The chosen theme name (`dark` | `light` |
 * `system`) is written to both the document root and localStorage; the latter
 * lets main.tsx apply the theme synchronously before the first render, while
 * SQLite remains the source of truth that load() reconciles against.
 */
export function useTheme() {
  const apply = (theme: string) => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('synapt-theme', theme);
  };
  const load = async () => {
    const t = await invoke<string | null>('get_setting', { key: 'theme' }).catch(() => null);
    apply(t ?? 'dark');
  };
  return { apply, load };
}
