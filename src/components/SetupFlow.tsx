import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AnimatePresence, motion, useReducedMotion } from 'motion/react';
import { SPRING } from '../animations';
import { describeError } from '../utils/errors';
import { prettyHotkey } from '../utils/hotkey';
import { useHotkeyInfo } from '../hooks/useHotkeyInfo';
import { useHotkeyRecorder } from '../hooks/useHotkeyRecorder';

const STEPS = ['welcome', 'folder', 'hotkey', 'crash', 'done'] as const;

interface SetupFlowProps {
  onDone: () => void;
}

/**
 * First-launch setup, covering the overlay until it is finished. Each step can
 * be skipped; everything chosen here can be changed later in Settings.
 */
export const SetupFlow: React.FC<SetupFlowProps> = ({ onDone }) => {
  const reduceMotion = useReducedMotion();
  const [stepIndex, setStepIndex] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [addedDirs, setAddedDirs] = useState<string[]>([]);
  const [hotkey, setHotkey] = useState('ctrl+space');
  const [hotkeyChanged, setHotkeyChanged] = useState(false);
  const recorder = useHotkeyRecorder(combo => {
    setHotkey(combo);
    setHotkeyChanged(true);
  });
  const shortcut = useHotkeyInfo();

  useEffect(() => {
    invoke<string | null>('get_setting', { key: 'hotkey' })
      .then(saved => {
        if (saved) setHotkey(saved);
      })
      .catch(() => undefined);
  }, []);

  const step = STEPS[stepIndex] ?? 'done';
  // On Wayland the desktop owns the shortcut, so show what it reports instead.
  const hotkeyLabel = shortcut.info?.wayland ? shortcut.info.trigger : prettyHotkey(hotkey);

  const next = () => {
    setError(null);
    setStepIndex(i => Math.min(i + 1, STEPS.length - 1));
  };

  const chooseFolder = async () => {
    setError(null);
    setBusy(true);
    try {
      const path = await invoke<string | null>('open_dir_picker');
      if (path) {
        await invoke('add_indexed_dir', { path });
        setAddedDirs(dirs => [...dirs, path]);
      }
    } catch (e) {
      setError(describeError(e, 'Could not add that folder. Choose another, or add folders later in Settings.'));
    } finally {
      setBusy(false);
    }
  };

  const chooseCrashReports = async (enabled: boolean) => {
    setError(null);
    setBusy(true);
    try {
      await invoke('set_setting', { key: 'crash_reports_enabled', value: String(enabled) });
      next();
    } catch (e) {
      setError(describeError(e, 'Could not save your choice. Try again.'));
    } finally {
      setBusy(false);
    }
  };

  const finish = async () => {
    setError(null);
    setBusy(true);
    try {
      await invoke('set_setting', { key: 'onboarding_complete', value: 'true' });
      onDone();
    } catch (e) {
      setError(describeError(e, 'Could not finish setup. Try again.'));
      setBusy(false);
    }
  };

  const content = (() => {
    switch (step) {
      case 'welcome':
        return {
          heading: 'Welcome to Synapt',
          body: 'A launcher and file transfer tool for your local network. Takes 30 seconds to set up.',
          actions: (
            <button type="button" autoFocus onClick={next} className="btn btn-primary">
              Get started
            </button>
          ),
        };
      case 'folder':
        return {
          heading: 'Add a folder to search',
          body: 'Synapt indexes files in folders you choose. You can add more later in Settings.',
          detail: addedDirs.length > 0 && (
            <ul className="flex flex-col gap-1">
              {addedDirs.map(dir => (
                <li
                  key={dir}
                  className="text-[13px] break-all"
                  style={{ color: 'var(--text)', fontFamily: 'monospace' }}
                >
                  {dir}
                </li>
              ))}
            </ul>
          ),
          actions:
            addedDirs.length > 0 ? (
              <>
                <button type="button" autoFocus onClick={next} className="btn btn-primary">
                  Continue
                </button>
                <button type="button" onClick={chooseFolder} disabled={busy} className="btn btn-secondary">
                  Add another folder
                </button>
              </>
            ) : (
              <>
                <button type="button" autoFocus onClick={chooseFolder} disabled={busy} className="btn btn-primary">
                  Choose folder
                </button>
                <button type="button" onClick={next} className="btn btn-ghost">
                  Skip for now
                </button>
              </>
            ),
        };
      case 'hotkey':
        return {
          heading: 'Your hotkey',
          body: shortcut.info?.wayland
            ? 'Your desktop manages this shortcut and may ask for permission. Press it anywhere to open Synapt.'
            : 'Press this combination anywhere to open Synapt.',
          detail: (
            <p className="text-[13px]" style={{ color: 'var(--text)', fontFamily: 'monospace' }}>
              {recorder.recording ? 'Press any key...' : hotkeyLabel ?? 'No shortcut set yet'}
            </p>
          ),
          actions: (
            <>
              <button type="button" autoFocus onClick={next} disabled={recorder.recording} className="btn btn-primary">
                {hotkeyChanged ? 'Continue' : 'Keep default'}
              </button>
              <button
                type="button"
                disabled={shortcut.busy || shortcut.info === null}
                onClick={() => {
                  if (shortcut.info?.wayland) void shortcut.configure();
                  else recorder.setRecording(r => !r);
                }}
                aria-pressed={recorder.recording}
                className="btn btn-secondary"
              >
                {shortcut.busy
                  ? 'Waiting for desktop...'
                  : shortcut.info?.wayland
                    ? 'Configure shortcut'
                    : recorder.recording
                      ? 'Press any key...'
                      : 'Change'}
              </button>
            </>
          ),
          extraError: recorder.error ?? shortcut.error ?? shortcut.info?.error ?? null,
        };
      case 'crash':
        return {
          heading: 'Help improve Synapt',
          body: 'If the app crashes, you can send a report to help fix it. Reports are sent by email and contain no personal data.',
          actions: (
            <>
              <button
                type="button"
                autoFocus
                onClick={() => chooseCrashReports(true)}
                disabled={busy}
                className="btn btn-primary"
              >
                Enable crash reports
              </button>
              <button type="button" onClick={() => chooseCrashReports(false)} disabled={busy} className="btn btn-ghost">
                No thanks
              </button>
            </>
          ),
        };
      case 'done':
        return {
          heading: 'You are ready',
          body: hotkeyLabel
            ? `Press ${hotkeyLabel} to open Synapt from anywhere.`
            : 'Open Synapt from the tray icon, or set a shortcut in Settings.',
          actions: (
            <button type="button" autoFocus onClick={finish} disabled={busy} className="btn btn-primary">
              Start using Synapt
            </button>
          ),
        };
    }
  })();

  const message = error ?? ('extraError' in content ? content.extraError : null);
  const offset = reduceMotion ? 0 : 40;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="setup-heading"
      className="absolute inset-0 flex items-center justify-center overflow-hidden"
      style={{ zIndex: 1500, backgroundColor: 'var(--bg)' }}
    >
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={step}
          initial={{ opacity: 0, x: offset }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: -offset }}
          transition={SPRING}
          className="flex flex-col gap-4 w-full"
          style={{ maxWidth: 400, padding: '0 24px' }}
        >
          <p className="text-[13px]" style={{ color: 'var(--muted)' }}>
            Step {stepIndex + 1} of {STEPS.length}
          </p>
          <div className="flex flex-col gap-2">
            <h1 id="setup-heading" className="text-sm font-medium" style={{ color: 'var(--text)' }}>
              {content.heading}
            </h1>
            <p className="text-[13px]" style={{ color: 'var(--muted)' }}>
              {content.body}
            </p>
          </div>
          {'detail' in content && content.detail}
          {message && (
            <p role="alert" className="error-text">
              {message}
            </p>
          )}
          <div className="flex items-center gap-2">{content.actions}</div>
        </motion.div>
      </AnimatePresence>
    </div>
  );
};
