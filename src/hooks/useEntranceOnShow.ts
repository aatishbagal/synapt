import { useEffect } from 'react';
import { useAnimationControls, type Transition } from 'motion/react';
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
 */
export function useEntranceOnShow(entrance: Entrance) {
  const controls = useAnimationControls();

  useEffect(() => {
    const play = () => {
      controls.set(entrance.initial);
      void controls.start({ ...entrance.animate, transition: entrance.transition });
    };
    play();
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) play();
    });
    return () => {
      unlisten.then(fn => fn()).catch(() => undefined);
    };
    // The entrance presets are module constants; run once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return controls;
}
