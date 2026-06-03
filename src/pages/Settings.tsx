import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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

const SECTION = 'text-text-muted text-xs uppercase tracking-wider mb-2';
const INPUT = 'bg-border border border-border text-text-primary rounded-btn px-3 py-1.5 text-sm w-full';

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

function statusBadgeClass(s: QueueStatus): string {
  const base = 'text-xs rounded px-2 py-0.5';
  if (isFailed(s)) return `${base} bg-red-900/30 text-red-400`;
  switch (s) {
    case 'Queued':
      return `${base} bg-border text-text-muted`;
    case 'InProgress':
      return `${base} bg-accent/10 text-accent`;
    case 'Complete':
      return `${base} bg-green-900/30 text-green-400`;
    case 'Partial':
      return `${base} bg-yellow-900/30 text-yellow-400`;
    default:
      return `${base} bg-border text-text-muted`;
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
    <div className="w-full h-screen bg-bg p-6 overflow-y-auto">
      <button onClick={() => nav('/')} className="text-text-muted text-sm mb-6 hover:text-text-primary transition-colors">Back</button>

      <section className="mb-8">
        <h2 className={SECTION}>Device</h2>
        <div className="flex flex-col gap-3">
          <input
            className={INPUT}
            value={deviceName}
            onChange={e => setDeviceName(e.target.value)}
            onBlur={saveDeviceName}
            placeholder="Device name"
          />
          <p className="text-text-muted text-xs font-mono">ID: {local ? local.device_id.slice(0, 18) : '...'}</p>
          <p className="text-text-muted text-xs font-mono">Fingerprint: {local ? local.fingerprint.slice(0, 16) : '...'}</p>
        </div>
      </section>

      <section className="mb-8">
        <h2 className={SECTION}>Trusted Devices</h2>
        {trusted.length === 0
          ? <p className="text-text-muted text-xs">No trusted devices. Pair with a device from the main screen.</p>
          : (
            <div className="flex flex-col gap-2">
              {trusted.map(p => (
                <div key={p.device_id} className="flex items-center justify-between bg-surface border border-border rounded-card px-4 py-2">
                  <div>
                    <p className="text-text-primary text-sm font-medium">{p.device_name}</p>
                    <p className="text-text-muted text-xs font-mono">{p.fingerprint.slice(0, 16)}</p>
                    <p className="text-text-muted text-xs">paired {relativeDate(p.paired_at)}</p>
                  </div>
                  <button onClick={() => removePeer(p.device_id)} className="text-xs px-3 py-1 border border-border text-text-primary rounded-btn hover:bg-border transition-colors">Remove</button>
                </div>
              ))}
            </div>
          )}
      </section>

      <section className="mb-8">
        <h2 className={SECTION}>Shared Directories</h2>
        <p className="text-text-muted text-xs mb-2">Only directories added here are accessible to trusted peers.</p>
        <div className="flex flex-col gap-2 mb-2">
          {sharedDirs.map(d => (
            <div key={d} className="flex items-center justify-between bg-surface border border-border rounded-card px-4 py-2">
              <p className="text-text-primary text-sm font-mono break-all">{d}</p>
              <button onClick={() => removeDir(d)} className="text-xs px-3 py-1 border border-border text-text-primary rounded-btn hover:bg-border transition-colors shrink-0 ml-2">Remove</button>
            </div>
          ))}
        </div>
        <div className="flex gap-2">
          <input
            className={INPUT}
            value={newDir}
            onChange={e => setNewDir(e.target.value)}
            placeholder="/absolute/path/to/dir"
          />
          <button onClick={addDir} className="text-xs px-4 py-1.5 bg-accent text-bg rounded-btn hover:opacity-80 transition-opacity shrink-0">Add</button>
        </div>
      </section>

      <section className="mb-8">
        <div className="flex items-center justify-between mb-2">
          <h2 className={`${SECTION} mb-0`}>Indexed Directories</h2>
          <button
            onClick={rescan}
            disabled={rescanning}
            className="text-xs px-3 py-1 border border-border text-text-primary rounded-btn hover:bg-border transition-colors disabled:opacity-50"
          >
            {rescanning ? 'Rescanning...' : 'Rescan'}
          </button>
        </div>
        <p className="text-text-muted text-xs mb-2">
          Directories added here are scanned and searchable from the overlay.
          {indexStatus && ` ${indexStatus.file_count} files indexed.`}
        </p>
        {indexedDirs.length === 0
          ? <p className="text-text-muted text-xs mb-2">No indexed directories. Add one below to start searching.</p>
          : (
            <div className="flex flex-col gap-2 mb-2">
              {indexedDirs.map(d => (
                <div key={d.path} className="flex items-center justify-between bg-surface border border-border rounded-card px-4 py-2">
                  <div className="min-w-0">
                    <p className="text-text-primary text-sm font-mono break-all">{d.path}</p>
                    <p className="text-text-muted text-xs">
                      {d.file_count} files{d.last_indexed ? ` - scanned ${relativeDate(d.last_indexed)}` : ' - not scanned yet'}
                    </p>
                  </div>
                  <button onClick={() => removeIndexedDir(d.path)} className="text-xs px-3 py-1 border border-border text-text-primary rounded-btn hover:bg-border transition-colors shrink-0 ml-2">Remove</button>
                </div>
              ))}
            </div>
          )}
        <div className="flex gap-2">
          <input
            className={INPUT}
            value={newIndexedDir}
            onChange={e => setNewIndexedDir(e.target.value)}
            placeholder="/absolute/path/to/dir"
          />
          <button onClick={addIndexedDir} disabled={rescanning} className="text-xs px-4 py-1.5 bg-accent text-bg rounded-btn hover:opacity-80 transition-opacity shrink-0 disabled:opacity-50">Add</button>
        </div>
      </section>

      <section className="mb-8">
        <h2 className={SECTION}>Preferences</h2>
        <div className="flex flex-col gap-3">
          <label className="flex items-center justify-between">
            <span className="text-text-primary text-sm">Max results</span>
            <input
              type="number"
              min={10}
              max={200}
              className="bg-border border border-border text-text-primary rounded-btn px-3 py-1.5 text-sm w-24"
              value={maxResults}
              onChange={e => setMaxResults(e.target.value)}
              onBlur={saveMaxResults}
            />
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={includeHidden}
              onChange={e => toggleIncludeHidden(e.target.checked)}
            />
            <span className="text-text-primary text-sm">Include hidden files</span>
          </label>
        </div>
      </section>

      <section className="mb-8">
        <h2 className={SECTION}>Transfer Queue</h2>
        {queue.length === 0
          ? <p className="text-text-muted text-xs">No transfers yet.</p>
          : (
            <div className="flex flex-col gap-2">
              {queue.map(t => (
                <div key={t.transfer_id} className="bg-surface border border-border rounded-card px-4 py-2">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-text-primary text-sm font-medium">{t.filename}</p>
                      <p className="text-text-muted text-xs">{t.peer_name}</p>
                    </div>
                    <span className={statusBadgeClass(t.status)}>{statusLabel(t.status)}</span>
                  </div>
                  {t.status === 'InProgress' && (
                    <div className="h-1 rounded bg-border mt-2">
                      <div
                        className="h-1 rounded bg-accent"
                        style={{ width: `${t.total > 0 ? (t.bytes_received / t.total) * 100 : 0}%` }}
                      />
                    </div>
                  )}
                  {t.status === 'Complete' && (
                    <p className="text-text-muted text-xs mt-1">{formatBytes(t.total)}</p>
                  )}
                  {isFailed(t.status) && (
                    <p className="text-red-400 text-xs mt-1">{t.status.Failed.reason}</p>
                  )}
                </div>
              ))}
            </div>
          )}
      </section>

      <section className="mb-8">
        <h2 className={SECTION}>Transfer History</h2>
        {history.length === 0
          ? <p className="text-text-muted text-xs">No transfers yet.</p>
          : (
            <div className="flex flex-col gap-2">
              {history.map((t, i) => (
                <div key={`${t.filename}-${t.started_at}-${i}`} className="flex items-center justify-between bg-surface border border-border rounded-card px-4 py-2">
                  <div>
                    <p className="text-text-primary text-sm">{t.filename}</p>
                    <p className="text-text-muted text-xs font-mono">{t.peer_device_id.slice(0, 12)}</p>
                  </div>
                  <div className="text-right">
                    <span className="text-xs px-2 py-0.5 rounded text-accent bg-accent/10">{t.status}</span>
                    <p className="text-text-muted text-xs mt-1">{relativeDate(t.started_at)}</p>
                  </div>
                </div>
              ))}
            </div>
          )}
      </section>
    </div>
  );
};
