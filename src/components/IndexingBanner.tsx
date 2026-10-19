import React, { useEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { FADE_IN } from '../animations';
import { listen } from '@tauri-apps/api/event';
import { IndexPhase, IndexProgress } from '../types';
import { UnderlineLoader } from './UnderlineLoader';
import { plainError } from '../utils/errors';

/** 6px filled circle separating the phase from its detail. */
const Separator: React.FC = () => (
  <div
    aria-hidden
    className="shrink-0"
    style={{ width: 6, height: 6, borderRadius: '50%', backgroundColor: 'var(--muted)' }}
  />
);

/** Human-readable label for the current indexing phase, plus an optional detail. */
function phaseLabel(phase: IndexPhase, filesScanned: number): { label: string; detail?: string } {
  switch (phase.type) {
    case 'Starting':
      return { label: 'Preparing index...' };
    case 'Scanning':
      return { label: 'Scanning', detail: `${filesScanned.toLocaleString()} files` };
    case 'BuildingIndex':
      return { label: 'Building search index...' };
    case 'Complete':
      return { label: 'Index complete', detail: `${phase.total_files.toLocaleString()} files` };
    case 'Failed':
      return { label: 'Index failed', detail: plainError(phase.reason, 'Rescan from Settings to try again.') };
  }
}

/** Map the current phase to the underline loader's state. */
function loaderState(phase: IndexPhase): 'active' | 'done' | 'failed' {
  if (phase.type === 'Complete') return 'done';
  if (phase.type === 'Failed') return 'failed';
  return 'active';
}

/** Fill width of the progress bar, as a percentage of the banner width. */
function fillPercent(progress: IndexProgress): number {
  switch (progress.phase.type) {
    case 'Scanning': {
      if (progress.total_dirs === 0) return 0;
      return Math.min((progress.dirs_done / progress.total_dirs) * 85, 85);
    }
    case 'BuildingIndex':
      return 90;
    case 'Complete':
      return 100;
    case 'Starting':
    case 'Failed':
      return 0;
  }
}

/** Secondary metric shown on the right of the banner. */
function rightText(progress: IndexProgress): string {
  const { phase } = progress;
  if (phase.type === 'Scanning' && progress.total_dirs > 0) {
    return `${progress.dirs_done} / ${progress.total_dirs} folders`;
  }
  // Without a directory total the file count is already in the label.
  return '';
}

/**
 * Slim banner pinned to the bottom of the overlay that reflects live indexing
 * progress. Self-managing: it listens for `index-progress` events, shows itself
 * during indexing, and hides 3 seconds after a Complete or Failed event.
 */
export const IndexingBanner: React.FC = () => {
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [visible, setVisible] = useState(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const unlisten = listen<IndexProgress>('index-progress', event => {
      const next = event.payload;
      setProgress(next);
      setVisible(true);
      if (hideTimer.current) {
        clearTimeout(hideTimer.current);
        hideTimer.current = null;
      }
      if (next.phase.type === 'Complete' || next.phase.type === 'Failed') {
        hideTimer.current = setTimeout(() => setVisible(false), 3000);
      }
    });
    return () => {
      unlisten.then(fn => fn()).catch(() => undefined);
      if (hideTimer.current) clearTimeout(hideTimer.current);
    };
  }, []);

  const { label, detail } = progress ? phaseLabel(progress.phase, progress.files_scanned) : { label: '', detail: undefined };

  return (
    <AnimatePresence>
      {visible && progress && (
        <motion.div
          key="indexing-banner"
          initial={{ ...FADE_IN.initial, y: 8 }}
          animate={{ ...FADE_IN.animate, y: 0 }}
          exit={{ opacity: 0, y: 8 }}
          transition={FADE_IN.transition}
          style={{
            position: 'absolute',
            bottom: 0,
            left: 0,
            right: 0,
            background: 'var(--surface)',
            borderTop: '1px solid var(--border)',
            padding: '8px 16px',
          }}
        >
          <div
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              right: 0,
              height: 3,
              background: 'var(--border)',
            }}
          >
            <div
              style={{
                height: '100%',
                width: `${fillPercent(progress)}%`,
                background: 'var(--accent)',
                transition: 'width var(--duration-medium) var(--ease-standard)',
              }}
            />
          </div>
          <div role="status" style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12 }}>
            <UnderlineLoader state={loaderState(progress.phase)} width={20} />
            <span className="flex flex-1 items-center gap-2 min-w-0">
              <span style={{ color: 'var(--text)' }}>{label}</span>
              {detail && (
                <>
                  <Separator />
                  <span className="truncate" style={{ color: 'var(--muted)' }}>{detail}</span>
                </>
              )}
            </span>
            <span style={{ color: 'var(--muted)' }}>{rightText(progress)}</span>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
