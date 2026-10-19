import { useEffect } from 'react';
import { useAnimationControls, type Transition } from 'motion/react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

interface Entrance {
  initial: Record<string, number>;
  animate: Record<string, number>;
  transition: Transition;
}

/**
 * Replays an entrance animation every time the window gains focus. The window
 * stays mounted and is shown and hidden from Rust (the global hotkey shows and
 * focuses it, losing focus hides it), so a mount animation would only ever play
 * once. Replaying through animation controls keeps all component state intact,
 * which re-mounting the root would not.
 *
 * The window is put back on screen with whatever it last painted, before the
 * focus event arrives. So the content drops to the entrance's start state when
 * Rust is about to hide the window (`window:will-hide`, see entrance.rs) and
 * while it starts hidden, which makes that leftover frame the first frame of
 * the entrance instead of a fully drawn window that then snaps back.
 */
export function useEntranceOnShow(entrance: Entrance) {
  const controls = useAnimationControls();

  useEffect(() => {
    const win = getCurrentWindow();
    const reset = () => {
      controls.stop();
      controls.set(entrance.initial);
    };
    const play = () => {
      controls.set(entrance.initial);
      void controls.start({ ...entrance.animate, transition: entrance.transition });
    };

    // Already on screen when mounted after a route change, so play now;
    // otherwise wait for the focus that comes with being shown.
    reset();
    win
      .isVisible()
      .then(visible => {
        if (visible) play();
      })
      .catch(play);

    const unlistenFocus = win.onFocusChanged(({ payload: focused }) => {
      if (focused) play();
    });
    const unlistenHide = listen('window:will-hide', reset);
    return () => {
      unlistenFocus.then(fn => fn()).catch(() => undefined);
      unlistenHide.then(fn => fn()).catch(() => undefined);
    };
    // The entrance presets are module constants; run once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return controls;
}
