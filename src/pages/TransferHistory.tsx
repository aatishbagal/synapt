import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, FolderOpen } from 'lucide-react';
import { TrustedPeer } from '../types';
import { EmptyState } from '../components/EmptyState';
import { describeError } from '../utils/errors';

interface TransferEntry {
  peer_device_id: string;
  filename: string;
  remote_path: string;
  local_path: string;
  size: number | null;
  bytes_received: number;
  status: string;
  started_at: number;
  completed_at: number | null;
}

const itemCard: React.CSSProperties = {
  backgroundColor: 'var(--surface)',
  border: '1px solid var(--border)',
  borderRadius: '8px',
};

function relativeDate(unixSeconds: number | null): string {
  if (!unixSeconds) return 'never';
  const diff = Date.now() - unixSeconds * 1000;
  const day = 86_400_000;
  if (diff < 60_000) return 'just now';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < day) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / day)}d ago`;
}

function formatBytes(n: number | null): string {
  if (n === null) return '-';
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = n / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(1)} ${units[i]}`;
}

/** Plain-language status label; colour backs up the text, never replaces it. */
function statusLabel(status: string): { text: string; color: string } {
  switch (status.toLowerCase()) {
    case 'complete':
    case 'completed':
      return { text: 'Completed', color: 'var(--muted)' };
    case 'failed':
      return { text: 'Failed', color: 'var(--danger)' };
    case 'partial':
      return { text: 'Incomplete', color: 'var(--warning)' };
    case 'in_progress':
    case 'inprogress':
      return { text: 'In progress', color: 'var(--text)' };
    default:
      return { text: status.charAt(0).toUpperCase() + status.slice(1).replace(/_/g, ' '), color: 'var(--muted)' };
  }
}

export const TransferHistory: React.FC = () => {
  const nav = useNavigate();
  const [history, setHistory] = useState<TransferEntry[]>([]);
  const [trusted, setTrusted] = useState<TrustedPeer[]>([]);
  const [expanded, setExpanded] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<TransferEntry[]>('get_transfer_history').then(setHistory).catch(() => setHistory([]));
    invoke<TrustedPeer[]>('get_trusted_peers').then(setTrusted).catch(() => setTrusted([]));
  }, []);

  const peerName = (deviceId: string): string => {
    const match = trusted.find(p => p.device_id === deviceId);
    return match ? match.device_name : deviceId.slice(0, 12);
  };

  const openLocation = (path: string) => {
    setError(null);
    invoke('reveal_in_files', { path }).catch(e =>
      setError(describeError(e, 'Could not open the file manager. The file may have been moved or deleted.')),
    );
  };

  return (
    <div
      className="flex flex-col w-screen h-screen overflow-hidden"
      style={{ backgroundColor: 'var(--bg)', color: 'var(--text)', borderRadius: '8px' }}
    >
      <div
        className="flex items-center justify-between px-3 shrink-0"
        style={{ height: '40px', borderBottom: '1px solid var(--border)', backgroundColor: 'var(--bg)' }}
      >
        <div className="flex items-center gap-2">
          <img src="/assets/images/logo/png/SynaptV2_White_PNG.png" alt="Synapt" className="h-4 w-4" />
          <span className="text-xs font-medium">Transfer History</span>
        </div>
        <button
          type="button"
          onClick={() => nav('/settings')}
          title="Back to Settings"
          className="icon-button icon-button-labeled"
        >
          <ArrowLeft size={12} />
          <span>Settings</span>
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {error && (
          <p role="alert" className="error-text mb-2">
            {error}
          </p>
        )}
        {history.length === 0 ? (
          <EmptyState
            title="No transfers yet"
            detail="Files you send to or receive from paired devices appear here."
          />
        ) : (
          <div className="flex flex-col gap-2">
            {history.map((t, i) => {
              const isOpen = expanded === i;
              return (
                <div key={`${t.filename}-${t.started_at}-${i}`} style={itemCard}>
                  <button
                    type="button"
                    onClick={() => setExpanded(isOpen ? null : i)}
                    aria-expanded={isOpen}
                    className="flex items-center justify-between gap-2 px-3 py-2 w-full text-left"
                  >
                    <div className="min-w-0">
                      <p className="text-[13px] truncate" style={{ color: 'var(--text)' }}>
                        {t.filename}
                      </p>
                      <p className="text-xs truncate" style={{ color: 'var(--muted)' }}>
                        {peerName(t.peer_device_id)}
                      </p>
                    </div>
                    <div className="text-right shrink-0">
                      <span className="text-xs" style={{ color: statusLabel(t.status).color }}>
                        {statusLabel(t.status).text}
                      </span>
                      <p className="text-xs" style={{ color: 'var(--muted)' }}>
                        {formatBytes(t.size)} | {relativeDate(t.started_at)}
                      </p>
                    </div>
                  </button>

                  {isOpen && (
                    <div
                      className="px-3 py-2 flex flex-col gap-2"
                      style={{ borderTop: '1px solid var(--border)' }}
                    >
                      <p className="text-xs break-all" style={{ color: 'var(--muted)' }}>
                        {t.local_path}
                      </p>
                      <button
                        type="button"
                        onClick={() => openLocation(t.local_path)}
                        className="btn btn-secondary self-start"
                      >
                        <FolderOpen size={12} />
                        Open in file manager
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
