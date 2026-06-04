import React, { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { IndexPhase, IndexProgress } from '../types';
import { UnderlineLoader } from './UnderlineLoader';

/** Human-readable label for the current indexing phase. */
function phaseLabel(phase: IndexPhase, filesScanned: number): string {
  switch (phase.type) {
    case 'Starting':
      return 'Preparing index...';
    case 'Scanning':
      return `Scanning — ${filesScanned.toLocaleString()} files`;
    case 'BuildingIndex':
      return 'Building search index...';
    case 'Complete':
      return `Index complete — ${phase.total_files.toLocaleString()} files`;
    case 'Failed':
      return `Index failed: ${phase.reason}`;
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
    return `${progress.dirs_done} / ${progress.total_dirs} dirs`;
  }
  if (phase.type === 'Scanning') {
    return `${progress.files_scanned.toLocaleString()} files`;
  }
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

  if (!visible || !progress) return null;

  const { phase } = progress;

  return (
    <div
      style={{
        position: 'absolute',
        bottom: 0,
        left: 0,
        right: 0,
        background: 'var(--surface)',
        borderTop: '1px solid var(--border)',
        padding: '6px 16px',
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
            transition: 'width 300ms ease-out',
          }}
        />
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <UnderlineLoader state={loaderState(phase)} width={20} />
        <span style={{ flex: 1, color: 'var(--text)', fontSize: 12, fontWeight: 500 }}>
          {phaseLabel(phase, progress.files_scanned)}
        </span>
        <span style={{ color: 'var(--muted)', fontSize: 11 }}>{rightText(progress)}</span>
      </div>
    </div>
  );
};
