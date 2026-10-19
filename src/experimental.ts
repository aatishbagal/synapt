import { invoke } from '@tauri-apps/api/core';

/**
 * Whether the experimental UI can be used at all. Off for now: the Settings
 * section is hidden and a previously saved "true" is ignored, so nobody is left
 * with a setting they can no longer see or change.
 */
export const EXPERIMENTAL_UI_AVAILABLE = false;

/**
 * Whether the experimental UI is on for this run. Read once when the webview
 * starts and applied as data-experimental-ui on <html>. Changing the setting
 * only takes effect after a restart, so Settings compares the saved value with
 * this one to decide whether to offer a restart.
 */
export const experimentalUiAtLaunch: Promise<boolean> = invoke<string | null>('get_setting', {
  key: 'experimental_ui',
})
  .then(value => value === 'true')
  .catch(() => false)
  .then(on => {
    const active = EXPERIMENTAL_UI_AVAILABLE && on;
    if (active) document.documentElement.setAttribute('data-experimental-ui', 'true');
    return active;
  });
