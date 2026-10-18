import { invoke } from '@tauri-apps/api/core';

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
    if (on) document.documentElement.setAttribute('data-experimental-ui', 'true');
    return on;
  });
