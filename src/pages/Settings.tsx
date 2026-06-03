import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft } from 'lucide-react';
import { QueueEntry, TrustedPeer } from '../types';

interface LocalDevice {
  device_id: string;
  device_name: string;
  pubkey_b64: string;
  fingerprint: string;
}

interface IndexedDir {
  path: string;
  file_count: number;
  last_indexed: number | null;
}

interface IndexStatus {
  indexed_dirs_count: number;
  file_count: number;
  tantivy_ready: boolean;
  last_scan: number | null;
}

interface TransferHistory {
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

const inputStyle: React.CSSProperties = {
  backgroundColor: 'var(--surface)',
  color: 'var(--text)',
  border: '1px solid var(--border)',
};

const subtleButton: React.CSSProperties = {
  backgroundColor: 'var(--surface)',
  color: 'var(--text)',
  border: '1px solid var(--border)',
};

const accentButton: React.CSSProperties = {
  backgroundColor: 'var(--surface-hover)',
  color: 'var(--accent)',
  border: '1px solid var(--accent)',
};

const dangerButton: React.CSSProperties = {
  backgroundColor: 'var(--surface)',
  color: 'var(--danger)',
  border: '1px solid var(--border)',
};

const itemCard: React.CSSProperties = {
  backgroundColor: 'var(--surface)',
  border: '1px solid var(--border)',
  borderRadius: '8px',
};

function relativeDate(unixSeconds: number): string {
  const diff = Date.now() - unixSeconds * 1000;
  const day = 86_400_000;
  if (diff < 60_000) return 'just now';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < day) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / day)}d ago`;
}

function formatBytes(n: number): string {
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

type QueueStatus = QueueEntry['status'];

function isFailed(s: QueueStatus): s is { Failed: { reason: string } } {
  return typeof s === 'object' && s !== null && 'Failed' in s;
}

function statusLabel(s: QueueStatus): string {
  return isFailed(s) ? 'Failed' : s;
}

function statusBadgeStyle(s: QueueStatus): React.CSSProperties {
  const base: React.CSSProperties = { backgroundColor: 'var(--surface-hover)' };
  if (isFailed(s)) return { ...base, color: 'var(--danger)', border: '1px solid var(--border)' };
  switch (s) {
    case 'InProgress':
      return { ...base, color: 'var(--accent)', border: '1px solid var(--accent)' };
    case 'Complete':
      return { ...base, color: 'var(--success)', border: '1px solid var(--border)' };
    case 'Partial':
      return { ...base, color: 'var(--warning)', border: '1px solid var(--border)' };
    case 'Queued':
    default:
      return { ...base, color: 'var(--muted)', border: '1px solid var(--border)' };
  }
}

export const Settings: React.FC = () => {
  const nav = useNavigate();

  const [local, setLocal] = useState<LocalDevice | null>(null);
  const [deviceName, setDeviceName] = useState('');
  const [trusted, setTrusted] = useState<TrustedPeer[]>([]);
  const [sharedDirs, setSharedDirs] = useState<string[]>([]);
  const [newDir, setNewDir] = useState('');
  const [indexedDirs, setIndexedDirs] = useState<IndexedDir[]>([]);
  const [newIndexedDir, setNewIndexedDir] = useState('');
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);
  const [rescanning, setRescanning] = useState(false);
  const [maxResults, setMaxResults] = useState('50');
  const [includeHidden, setIncludeHidden] = useState(false);
  const [history, setHistory] = useState<TransferHistory[]>([]);
  const [queue, setQueue] = useState<QueueEntry[]>([]);

  const loadTrusted = async () => {
    setTrusted(await invoke<TrustedPeer[]>('get_trusted_peers').catch(() => []));
  };
  const loadSharedDirs = async () => {
    setSharedDirs(await invoke<string[]>('get_shared_dirs').catch(() => []));
  };
  const loadIndexedDirs = async () => {
    setIndexedDirs(await invoke<IndexedDir[]>('get_indexed_dirs').catch(() => []));
  };
  const loadIndexStatus = async () => {
    setIndexStatus(await invoke<IndexStatus>('get_index_status').catch(() => null));
  };

  useEffect(() => {
    (async () => {
      const dev = await invoke<LocalDevice>('get_local_device').catch(() => null);
      setLocal(dev);
      const nameOverride = await invoke<string | null>('get_setting', { key: 'device_name' }).catch(() => null);
      setDeviceName(nameOverride ?? dev?.device_name ?? '');

      await loadTrusted();
      await loadSharedDirs();
      await loadIndexedDirs();
      await loadIndexStatus();

      const mr = await invoke<string | null>('get_setting', { key: 'max_results' }).catch(() => null);
      setMaxResults(mr ?? '50');
      const ih = await invoke<string | null>('get_setting', { key: 'include_hidden' }).catch(() => null);
      setIncludeHidden(ih === 'true');

      setHistory(await invoke<TransferHistory[]>('get_transfer_history').catch(() => []));
    })();
  }, []);

  useEffect(() => {
    let cancelled = false;
    let running = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const isActive = (e: QueueEntry) => e.status === 'InProgress' || e.status === 'Queued';
    const refresh = async () => {
      const q = await invoke<QueueEntry[]>('get_transfer_queue').catch(() => []);
      if (!cancelled) setQueue(q);
      return q;
    };
    const tick = async () => {
      const q = await refresh();
      if (cancelled) {
        running = false;
        return;
      }
      if (q.some(isActive)) {
        timer = setTimeout(tick, 2000);
      } else {
        running = false;
      }
    };
    const ensureRunning = () => {
      if (running || cancelled) return;
      running = true;
      tick();
    };
    ensureRunning();
    const unlistenProgress = listen('transfer-progress', ensureRunning);
    const unlistenComplete = listen('transfer-complete', () => {
      refresh();
    });
    const unlistenFailed = listen('transfer-failed', () => {
      refresh();
    });
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      unlistenProgress.then(fn => fn());
      unlistenComplete.then(fn => fn());
      unlistenFailed.then(fn => fn());
    };
  }, []);

  const saveDeviceName = async () => {
    await invoke('set_setting', { key: 'device_name', value: deviceName }).catch(() => undefined);
  };
  const saveMaxResults = async () => {
    await invoke('set_setting', { key: 'max_results', value: maxResults }).catch(() => undefined);
  };
  const toggleIncludeHidden = async (checked: boolean) => {
    setIncludeHidden(checked);
    await invoke('set_setting', { key: 'include_hidden', value: checked ? 'true' : 'false' }).catch(() => undefined);
  };
  const removePeer = async (deviceId: string) => {
    await invoke('revoke_peer_cmd', { device_id: deviceId }).catch(() => undefined);
    await loadTrusted();
  };
  const addDir = async () => {
    if (!newDir.trim()) return;
    await invoke('add_shared_dir', { path: newDir.trim() }).catch(() => undefined);
    setNewDir('');
    await loadSharedDirs();
  };
  const removeDir = async (path: string) => {
    await invoke('remove_shared_dir', { path }).catch(() => undefined);
    await loadSharedDirs();
  };
  const addIndexedDir = async () => {
    if (!newIndexedDir.trim()) return;
    setRescanning(true);
    await invoke('add_indexed_dir', { path: newIndexedDir.trim() }).catch(() => undefined);
    setNewIndexedDir('');
    await loadIndexedDirs();
    await loadIndexStatus();
    setRescanning(false);
  };
  const removeIndexedDir = async (path: string) => {
    await invoke('remove_indexed_dir', { path }).catch(() => undefined);
    await loadIndexedDirs();
  };
  const rescan = async () => {
    setRescanning(true);
    await invoke('trigger_reindex').catch(() => undefined);
    await loadIndexedDirs();
    await loadIndexStatus();
    setRescanning(false);
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
          <img
            src="/assets/images/logo/png/SynaptV2_White_PNG.png"
            alt="Synapt"
            className="h-4 w-4"
          />
          <span className="text-xs font-medium">Synapt Settings</span>
        </div>
        <button
          type="button"
          onClick={() => nav('/')}
          title="Back"
          className="flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors"
          style={{ color: 'var(--muted)' }}
        >
          <ArrowLeft size={12} />
          Back
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3 flex flex-col gap-5">
        <Section title="Device">
          <div className="flex flex-col gap-3 px-3 py-2 rounded" style={itemCard}>
            <input
              className="rounded px-2 py-1 text-xs w-full"
              style={inputStyle}
              value={deviceName}
              onChange={e => setDeviceName(e.target.value)}
              onBlur={saveDeviceName}
              placeholder="Device name"
            />
            <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
              ID: {local ? local.device_id.slice(0, 18) : '...'}
            </p>
            <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
              Fingerprint: {local ? local.fingerprint.slice(0, 16) : '...'}
            </p>
          </div>
        </Section>

        <Section title="Trusted Devices">
          {trusted.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              No trusted devices. Pair with a device from the main screen.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              {trusted.map(p => (
                <div key={p.device_id} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <div className="min-w-0">
                    <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>{p.device_name}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>{p.fingerprint.slice(0, 16)}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)' }}>paired {relativeDate(p.paired_at)}</p>
                  </div>
                  <button onClick={() => removePeer(p.device_id)} className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors" style={dangerButton}>Remove</button>
                </div>
              ))}
            </div>
          )}
        </Section>

        <Section title="Shared Directories">
          <p className="text-xs" style={{ color: 'var(--muted)' }}>Only directories added here are accessible to trusted peers.</p>
          <div className="flex flex-col gap-2">
            {sharedDirs.map(d => (
              <div key={d} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                <p className="text-xs break-all" style={{ color: 'var(--text)', fontFamily: 'monospace' }}>{d}</p>
                <button onClick={() => removeDir(d)} className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors" style={dangerButton}>Remove</button>
              </div>
            ))}
          </div>
          <div className="flex gap-2">
            <input
              className="flex-1 rounded px-2 py-1 text-xs"
              style={inputStyle}
              value={newDir}
              onChange={e => setNewDir(e.target.value)}
              placeholder="/absolute/path/to/dir"
            />
            <button onClick={addDir} className="rounded px-2 py-1 text-xs shrink-0 transition-colors" style={subtleButton}>Add</button>
          </div>
        </Section>

        <Section
          title="Indexed Directories"
          action={
            <button
              onClick={rescan}
              disabled={rescanning}
              className="rounded px-2 py-1 text-xs transition-colors disabled:opacity-50"
              style={subtleButton}
            >
              {rescanning ? 'Rescanning...' : 'Rescan'}
            </button>
          }
        >
          <p className="text-xs" style={{ color: 'var(--muted)' }}>
            Directories added here are scanned and searchable from the overlay.
            {indexStatus && ` ${indexStatus.file_count} files indexed.`}
          </p>
          {indexedDirs.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>No indexed directories. Add one below to start searching.</p>
          ) : (
            <div className="flex flex-col gap-2">
              {indexedDirs.map(d => (
                <div key={d.path} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <div className="min-w-0">
                    <p className="text-xs break-all" style={{ color: 'var(--text)', fontFamily: 'monospace' }}>{d.path}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)' }}>
                      {d.file_count} files{d.last_indexed ? ` - scanned ${relativeDate(d.last_indexed)}` : ' - not scanned yet'}
                    </p>
                  </div>
                  <button onClick={() => removeIndexedDir(d.path)} className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors" style={dangerButton}>Remove</button>
                </div>
              ))}
            </div>
          )}
          <div className="flex gap-2">
            <input
              className="flex-1 rounded px-2 py-1 text-xs"
              style={inputStyle}
              value={newIndexedDir}
              onChange={e => setNewIndexedDir(e.target.value)}
              placeholder="/absolute/path/to/dir"
            />
            <button onClick={addIndexedDir} disabled={rescanning} className="rounded px-2 py-1 text-xs shrink-0 transition-colors disabled:opacity-50" style={accentButton}>Add</button>
          </div>
        </Section>

        <Section title="Preferences">
          <div className="flex flex-col gap-3 px-3 py-2 rounded" style={itemCard}>
            <label className="flex items-center justify-between">
              <span className="text-xs" style={{ color: 'var(--text)' }}>Max results</span>
              <input
                type="number"
                min={10}
                max={200}
                className="rounded px-2 py-1 text-xs w-24"
                style={inputStyle}
                value={maxResults}
                onChange={e => setMaxResults(e.target.value)}
                onBlur={saveMaxResults}
              />
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={includeHidden}
                onChange={e => toggleIncludeHidden(e.target.checked)}
                style={{ accentColor: 'var(--accent)' }}
              />
              <span className="text-xs" style={{ color: 'var(--text)' }}>Include hidden files</span>
            </label>
          </div>
        </Section>

        <Section title="Transfer Queue">
          {queue.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>No transfers yet.</p>
          ) : (
            <div className="flex flex-col gap-2">
              {queue.map(t => (
                <div key={t.transfer_id} className="px-3 py-2" style={itemCard}>
                  <div className="flex items-center justify-between">
                    <div className="min-w-0">
                      <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>{t.filename}</p>
                      <p className="text-xs" style={{ color: 'var(--muted)' }}>{t.peer_name}</p>
                    </div>
                    <span className="text-[10px] px-1.5 py-0.5 rounded shrink-0" style={statusBadgeStyle(t.status)}>{statusLabel(t.status)}</span>
                  </div>
                  {t.status === 'InProgress' && (
                    <div className="h-1 rounded mt-2" style={{ backgroundColor: 'var(--border)' }}>
                      <div
                        className="h-1 rounded"
                        style={{ width: `${t.total > 0 ? (t.bytes_received / t.total) * 100 : 0}%`, backgroundColor: 'var(--accent)' }}
                      />
                    </div>
                  )}
                  {t.status === 'Complete' && (
                    <p className="text-xs mt-1" style={{ color: 'var(--muted)' }}>{formatBytes(t.total)}</p>
                  )}
                  {isFailed(t.status) && (
                    <p className="text-xs mt-1" style={{ color: 'var(--danger)' }}>{t.status.Failed.reason}</p>
                  )}
                </div>
              ))}
            </div>
          )}
        </Section>

        <Section title="Transfer History">
          {history.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>No transfers yet.</p>
          ) : (
            <div className="flex flex-col gap-2">
              {history.map((t, i) => (
                <div key={`${t.filename}-${t.started_at}-${i}`} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <div className="min-w-0">
                    <p className="text-[13px]" style={{ color: 'var(--text)' }}>{t.filename}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>{t.peer_device_id.slice(0, 12)}</p>
                  </div>
                  <div className="text-right shrink-0 ml-2">
                    <span className="text-[10px] px-1.5 py-0.5 rounded" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--accent)', border: '1px solid var(--accent)' }}>{t.status}</span>
                    <p className="text-xs mt-1" style={{ color: 'var(--muted)' }}>{relativeDate(t.started_at)}</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Section>
      </div>
    </div>
  );
};

interface SectionProps {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}

function Section({ title, action, children }: SectionProps) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-[11px] uppercase tracking-wider" style={{ color: 'var(--muted)' }}>
          {title}
        </h2>
        {action}
      </div>
      {children}
    </div>
  );
}
