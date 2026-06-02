import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { TrustedPeer } from '../types';

interface LocalDevice {
  device_id: string;
  device_name: string;
  pubkey_b64: string;
  fingerprint: string;
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

export const Settings: React.FC = () => {
  const nav = useNavigate();

  const [local, setLocal] = useState<LocalDevice | null>(null);
  const [deviceName, setDeviceName] = useState('');
  const [trusted, setTrusted] = useState<TrustedPeer[]>([]);
  const [sharedDirs, setSharedDirs] = useState<string[]>([]);
  const [newDir, setNewDir] = useState('');
  const [maxResults, setMaxResults] = useState('50');
  const [includeHidden, setIncludeHidden] = useState(false);
  const [history, setHistory] = useState<TransferHistory[]>([]);

  const loadTrusted = async () => {
    setTrusted(await invoke<TrustedPeer[]>('get_trusted_peers').catch(() => []));
  };
  const loadSharedDirs = async () => {
    setSharedDirs(await invoke<string[]>('get_shared_dirs').catch(() => []));
  };

  useEffect(() => {
    (async () => {
      const dev = await invoke<LocalDevice>('get_local_device').catch(() => null);
      setLocal(dev);
      const nameOverride = await invoke<string | null>('get_setting', { key: 'device_name' }).catch(() => null);
      setDeviceName(nameOverride ?? dev?.device_name ?? '');

      await loadTrusted();
      await loadSharedDirs();

      const mr = await invoke<string | null>('get_setting', { key: 'max_results' }).catch(() => null);
      setMaxResults(mr ?? '50');
      const ih = await invoke<string | null>('get_setting', { key: 'include_hidden' }).catch(() => null);
      setIncludeHidden(ih === 'true');

      setHistory(await invoke<TransferHistory[]>('get_transfer_history').catch(() => []));
    })();
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
