import React, { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

type TransferStatus = 'in_progress' | 'complete' | 'failed';

interface TransferEntry {
  transfer_id: string;
  filename: string;
  peer_name: string;
  bytes_received: number;
  total: number;
  status: TransferStatus;
  error?: string;
}

interface ProgressPayload {
  transfer_id: string;
  filename: string;
  peer_name: string;
  bytes_received: number;
  total: number;
}

interface CompletePayload {
  transfer_id: string;
  filename: string;
  peer_name: string;
}

interface FailedPayload {
  transfer_id: string;
  filename: string;
  reason: string;
}

/**
 * Card below the search bar that tracks active file transfers by listening to
 * the transfer-progress, transfer-complete, and transfer-failed Tauri events.
 */
export const TransferCard: React.FC = () => {
  const [entries, setEntries] = useState<Map<string, TransferEntry>>(new Map());
  const interactedRef = useRef(false);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    listen<ProgressPayload>('transfer-progress', e => {
      const p = e.payload;
      setEntries(prev => {
        const next = new Map(prev);
        const existing = next.get(p.transfer_id);
        next.set(p.transfer_id, {
          transfer_id: p.transfer_id,
          filename: p.filename,
          peer_name: p.peer_name,
          bytes_received: p.bytes_received,
          total: p.total,
          status: 'in_progress',
          error: existing?.error,
        });
        return next;
      });
    }).then(u => unlisteners.push(u));

    listen<CompletePayload>('transfer-complete', e => {
      const p = e.payload;
      setEntries(prev => {
        const next = new Map(prev);
        const existing = next.get(p.transfer_id);
        next.set(p.transfer_id, {
          transfer_id: p.transfer_id,
          filename: existing?.filename ?? p.filename,
          peer_name: existing?.peer_name ?? p.peer_name,
          total: existing?.total ?? 0,
          // A completed transfer fills its bar fully.
          bytes_received: existing?.total ?? existing?.bytes_received ?? 0,
          status: 'complete',
        });
        return next;
      });
    }).then(u => unlisteners.push(u));

    listen<FailedPayload>('transfer-failed', e => {
      const p = e.payload;
      setEntries(prev => {
        const next = new Map(prev);
        const existing = next.get(p.transfer_id);
        next.set(p.transfer_id, {
          transfer_id: p.transfer_id,
          filename: existing?.filename ?? p.filename,
          peer_name: existing?.peer_name ?? '',
          total: existing?.total ?? 0,
          bytes_received: existing?.bytes_received ?? 0,
          status: 'failed',
          error: p.reason,
        });
        return next;
      });
    }).then(u => unlisteners.push(u));

    return () => unlisteners.forEach(u => u());
  }, []);

  // Auto-dismiss 5s after all transfers settle, unless the user interacted.
  useEffect(() => {
    if (entries.size === 0) return;
    const list = [...entries.values()];
    const allDone = list.every(e => e.status === 'complete' || e.status === 'failed');
    if (!allDone || interactedRef.current) return;
    const t = setTimeout(() => {
      if (!interactedRef.current) setEntries(new Map());
    }, 5000);
    return () => clearTimeout(t);
  }, [entries]);

  if (entries.size === 0) return null;

  const list = [...entries.values()];
  const totalBytes = list.reduce((sum, e) => sum + e.total, 0);
  const receivedBytes = list.reduce((sum, e) => sum + e.bytes_received, 0);
  const completed = list.filter(e => e.status === 'complete').length;
  const anyInProgress = list.some(e => e.status === 'in_progress');
  const anyFailed = list.some(e => e.status === 'failed');
  const firstError = list.find(e => e.status === 'failed')?.error ?? 'transfer failed';

  const pct = totalBytes > 0 ? Math.min(100, (receivedBytes / totalBytes) * 100) : anyInProgress ? 0 : 100;

  const fillColor = anyFailed
    ? '#f87171'
    : !anyInProgress
      ? 'var(--success)'
      : 'var(--accent)';

  const headerText =
    list.length === 1
      ? `${list[0].filename} from ${list[0].peer_name}`
      : `${completed} / ${list.length} files`;

  const statusText = anyInProgress
    ? 'Downloading...'
    : anyFailed
      ? `Failed: ${firstError}`
      : 'Complete';

  const dismiss = () => {
    interactedRef.current = true;
    setEntries(new Map());
  };

  return (
    <div
      className="shrink-0"
      onMouseEnter={() => {
        interactedRef.current = true;
      }}
      style={{
        backgroundColor: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: '8px',
        padding: '10px 14px',
        margin: '4px',
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <span
          className="truncate"
          style={{ color: 'var(--text)', fontSize: '13px', minWidth: 0 }}
        >
          {headerText}
        </span>
        <button
          type="button"
          aria-label="Dismiss transfers"
          onClick={dismiss}
          className="flex items-center justify-center shrink-0"
          style={{ border: 'none', background: 'transparent', color: 'var(--muted)', cursor: 'pointer' }}
        >
          <X size={16} />
        </button>
      </div>
      <div
        style={{
          width: '100%',
          height: '3px',
          backgroundColor: 'var(--border)',
          borderRadius: '2px',
          overflow: 'hidden',
          marginTop: '8px',
        }}
      >
        <div
          style={{
            height: '100%',
            width: `${pct}%`,
            backgroundColor: fillColor,
            transition: 'width 200ms ease-out',
          }}
        />
      </div>
      <p style={{ fontSize: '11px', color: 'var(--muted)', marginTop: '6px' }}>{statusText}</p>
    </div>
  );
};
