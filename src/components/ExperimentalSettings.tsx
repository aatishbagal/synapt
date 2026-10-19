import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AnimatePresence, motion, useReducedMotion } from 'motion/react';
import { experimentalUiAtLaunch } from '../experimental';
import { INLINE_CONFIRM } from '../animations';
import { describeError } from '../utils/errors';

/**
 * Body of the Experimental section in Settings. The experimental UI is read
 * once at launch, so a change only applies after a restart: when the saved
 * value differs from the one this run started with, offer to restart now.
 * Switching back to the running value withdraws the offer.
 */
export const ExperimentalSettings: React.FC = () => {
  const [saved, setSaved] = useState<boolean | null>(null);
  const [atLaunch, setAtLaunch] = useState<boolean | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reduceMotion = useReducedMotion();

  useEffect(() => {
    void experimentalUiAtLaunch.then(setAtLaunch);
    invoke<string | null>('get_setting', { key: 'experimental_ui' })
      .then(value => setSaved(value === 'true'))
      .catch(() => setSaved(false));
  }, []);

  const toggle = async (checked: boolean) => {
    setSaved(checked);
    setDismissed(false);
    setError(null);
    try {
      await invoke('set_setting', { key: 'experimental_ui', value: String(checked) });
    } catch (e) {
      setSaved(!checked);
      setError(describeError(e, 'Could not save this setting. Try again.'));
    }
  };

  const restart = () => {
    setError(null);
    invoke('restart_app').catch(e =>
      setError(describeError(e, 'Could not restart Synapt. Quit it from the tray and open it again.')),
    );
  };

  const showRestart = saved !== null && atLaunch !== null && saved !== atLaunch && !dismissed;

  return (
    <div className="flex flex-col gap-3">
      <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
        These settings may change or be removed in future versions. Restart required to apply changes.
      </p>

      <label className="flex items-center justify-between gap-3 cursor-pointer">
        <span className="flex flex-col">
          <span className="text-xs" style={{ color: 'var(--text)' }}>Experimental UI</span>
          <span className="text-[11px]" style={{ color: 'var(--muted)' }}>
            Try the new UI design currently in development.
          </span>
        </span>
        <input
          type="checkbox"
          checked={saved ?? false}
          disabled={saved === null}
          onChange={e => void toggle(e.target.checked)}
          style={{ accentColor: 'var(--accent)' }}
        />
      </label>

      <AnimatePresence initial={false}>
        {showRestart && (
          <motion.div
            key="restart"
            initial={reduceMotion ? { opacity: 0 } : INLINE_CONFIRM.initial}
            animate={reduceMotion ? { opacity: 1 } : INLINE_CONFIRM.animate}
            exit={reduceMotion ? { opacity: 0 } : INLINE_CONFIRM.exit}
            transition={INLINE_CONFIRM.transition}
            style={{
              backgroundColor: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: '6px',
              overflow: 'hidden',
            }}
          >
            <div style={{ padding: '8px 12px' }}>
              <p role="status" style={{ color: 'var(--text)', fontSize: '13px' }}>
                Restart Synapt to apply this change.
              </p>
              <div className="flex items-center gap-2" style={{ marginTop: '8px' }}>
                <button
                  type="button"
                  onClick={restart}
                  className="btn btn-secondary"
                  style={{ color: 'var(--accent)' }}
                >
                  Restart now
                </button>
                <button type="button" onClick={() => setDismissed(true)} className="btn btn-ghost">
                  Later
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {error && (
        <p role="alert" className="error-text">
          {error}
        </p>
      )}
    </div>
  );
};
