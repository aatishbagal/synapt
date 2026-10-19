import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { motion, useReducedMotion } from 'motion/react';
import { SPRING_GENTLE } from '../animations';
import { describeError } from '../utils/errors';

/** A crash log left by the previous session (`check_crash_log`). */
export interface CrashReport {
  path: string;
  content: string;
  version: string;
  os: string;
}

interface CrashPopupProps {
  report: CrashReport;
  onSend: () => void;
  onDismiss: () => void;
}

/**
 * Offers to email the previous session's crash log. Sits in the layout at the
 * bottom of the window rather than over it, so the overlay stays usable while
 * the offer is open. The parent wraps it in AnimatePresence for the exit.
 */
export const CrashPopup: React.FC<CrashPopupProps> = ({ report, onSend, onDismiss }) => {
  const reduceMotion = useReducedMotion();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const send = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke('open_crash_report_email', { content: report.content });
    } catch (e) {
      setError(describeError(e, `Could not open your email client. The crash log is at ${report.path}.`));
      setBusy(false);
      return;
    }
    // The draft is already open, so a log that will not delete only means the
    // offer returns on the next launch; it is not worth holding the popup for.
    await invoke('dismiss_crash_log').catch(e => console.error('Failed to clear crash log:', e));
    onSend();
  };

  const dismiss = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke('dismiss_crash_log');
      onDismiss();
    } catch (e) {
      setError(describeError(e, `Could not clear the crash log. You can delete it at ${report.path}.`));
      setBusy(false);
    }
  };

  const hidden = reduceMotion ? { opacity: 0 } : { opacity: 0, y: '100%' };

  return (
    <motion.div
      role="region"
      aria-label="Crash report"
      initial={hidden}
      animate={{ opacity: 1, y: 0, transition: SPRING_GENTLE }}
      exit={{ ...hidden, transition: SPRING_GENTLE }}
      className="shrink-0"
      style={{
        // Above the indexing banner, which is pinned to the same edge.
        position: 'relative',
        zIndex: 1,
        backgroundColor: 'var(--surface)',
        borderTop: '1px solid var(--border)',
        padding: '12px 16px',
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>
          Crash detected
        </h2>
        <button
          type="button"
          aria-label="Dismiss"
          title="Dismiss"
          onClick={dismiss}
          disabled={busy}
          className="icon-button"
        >
          <X size={14} />
        </button>
      </div>

      <p className="text-xs" style={{ color: 'var(--muted)', marginTop: 4 }}>
        Synapt crashed during the last session.
      </p>

      {error && (
        <p role="alert" className="error-text" style={{ marginTop: 8 }}>
          {error}
        </p>
      )}

      <div className="flex items-center" style={{ gap: 8, marginTop: 12 }}>
        <button type="button" onClick={send} disabled={busy} className="btn btn-primary">
          Send report by email
        </button>
        <button type="button" onClick={dismiss} disabled={busy} className="btn btn-ghost">
          Dismiss
        </button>
      </div>

      <p className="text-xs" style={{ color: 'var(--muted)', marginTop: 8 }}>
        Your email client will open with a pre-filled report. No data is sent automatically.
      </p>
    </motion.div>
  );
};
