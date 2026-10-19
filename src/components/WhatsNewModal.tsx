import React, { useCallback, useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { motion, useIsPresent, useReducedMotion } from 'motion/react';
import { FADE_IN, SPRING } from '../animations';

/** One release from the bundled whatsnew.json (`get_whats_new`). */
export interface WhatsNewEntry {
  version: string;
  /** YYYY-MM-DD, or null for versions that were never published. */
  date: string | null;
  headline: string;
  highlights: string[];
}

interface WhatsNewModalProps {
  data: WhatsNewEntry;
  onDismiss: () => void;
  onSeeAll: () => void;
}

/** "2026-10-17" as "October 2026", read as local midnight so the month never shifts. */
function formatReleaseDate(date: string): string {
  const parsed = new Date(`${date}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return date;
  return parsed.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
}

/**
 * Release notes for one version, over a dimmed window. The parent wraps it in
 * AnimatePresence for the exit, during which further actions are ignored.
 */
export const WhatsNewModal: React.FC<WhatsNewModalProps> = ({ data, onDismiss, onSeeAll }) => {
  const reduceMotion = useReducedMotion();
  const isPresent = useIsPresent();
  const gotItRef = useRef<HTMLButtonElement>(null);

  const dismiss = useCallback(() => {
    if (isPresent) onDismiss();
  }, [isPresent, onDismiss]);

  const seeAll = () => {
    if (!isPresent) return;
    onSeeAll();
    onDismiss();
  };

  useEffect(() => {
    gotItRef.current?.focus();
  }, []);

  // Captured so Escape closes the modal instead of hiding the window behind it.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      dismiss();
    };
    document.addEventListener('keydown', onKeyDown, true);
    return () => document.removeEventListener('keydown', onKeyDown, true);
  }, [dismiss]);

  const cardHidden = reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.95 };

  return (
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-labelledby="whats-new-headline"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={FADE_IN.transition}
      onMouseDown={e => {
        if (e.target === e.currentTarget) dismiss();
      }}
      className="fixed inset-0 flex items-center justify-center"
      style={{ zIndex: 2000, backgroundColor: 'var(--backdrop)' }}
    >
      <motion.div
        initial={cardHidden}
        animate={{ opacity: 1, scale: 1 }}
        exit={cardHidden}
        transition={SPRING}
        style={{
          width: 400,
          maxWidth: '90vw',
          backgroundColor: 'var(--surface)',
          border: '1px solid var(--border)',
          borderRadius: 8,
          padding: 24,
        }}
      >
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
            v{data.version}
          </span>
          <button
            type="button"
            aria-label="Close"
            title="Close (Esc)"
            onClick={dismiss}
            className="icon-button"
          >
            <X size={14} />
          </button>
        </div>

        <h2
          id="whats-new-headline"
          className="text-[13px] font-medium"
          style={{ color: 'var(--text)', marginTop: 8 }}
        >
          {data.headline}
        </h2>
        {data.date && (
          <p className="text-xs" style={{ color: 'var(--muted)', marginTop: 4 }}>
            {formatReleaseDate(data.date)}
          </p>
        )}

        <ul className="list-disc space-y-2" style={{ marginTop: 16, paddingLeft: 16 }}>
          {data.highlights.map(item => (
            <li key={item} className="text-[13px]" style={{ color: 'var(--text)' }}>
              {item}
            </li>
          ))}
        </ul>

        <div className="flex items-center justify-between gap-2" style={{ marginTop: 24 }}>
          <button
            type="button"
            onClick={seeAll}
            className="text-xs hover:underline"
            style={{ color: 'var(--accent)' }}
          >
            See all changes
          </button>
          <button ref={gotItRef} type="button" onClick={dismiss} className="btn btn-primary">
            Got it
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
};
