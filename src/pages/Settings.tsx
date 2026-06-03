import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft } from 'lucide-react';
import { TrustedPeer } from '../types';
import { useTheme } from '../hooks/useTheme';

interface LocalIdentity {
  device_id: string;
  device_name: string;
  fingerprint: string;
}

interface IndexedDir {
  path: string;
  file_count: number;
  last_indexed: number | null;
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

const APP_VERSION = '0.4.0';
const REPO_URL = 'https://github.com/aatishbagal/synapt';

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
  if (n === null) return '—';
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

/** Title-case a stored hotkey string such as `ctrl+space` for display. */
function prettyHotkey(hotkey: string): string {
  return hotkey
    .split('+')
    .map(part => (part.length <= 2 ? part.toUpperCase() : part.charAt(0).toUpperCase() + part.slice(1).toLowerCase()))
    .join('+');
}

/** Map a browser KeyboardEvent.code to a token the shortcut parser accepts. */
function keyLabel(code: string): string {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1];
  const digit = /^Digit(\d)$/.exec(code);
  if (digit) return digit[1];
  if (code.startsWith('Arrow')) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  switch (code) {
    case 'Space': return 'Space';
    case 'Enter': return 'Enter';
    case 'Tab': return 'Tab';
    case 'Backspace': return 'Backspace';
    case 'Minus': return '-';
    case 'Equal': return '=';
    case 'Comma': return ',';
    case 'Period': return '.';
    case 'Slash': return '/';
    case 'Semicolon': return ';';
    case 'Quote': return "'";
    case 'BracketLeft': return '[';
    case 'BracketRight': return ']';
    case 'Backslash': return '\\';
    case 'Backquote': return '`';
    default: return code;
  }
}

function statusBadgeStyle(status: string): React.CSSProperties {
  const base: React.CSSProperties = { backgroundColor: 'var(--surface-hover)' };
  switch (status.toLowerCase()) {
    case 'complete':
    case 'completed':
      return { ...base, color: 'var(--success)', border: '1px solid var(--border)' };
    case 'failed':
      return { ...base, color: 'var(--danger)', border: '1px solid var(--border)' };
    case 'partial':
      return { ...base, color: 'var(--warning)', border: '1px solid var(--border)' };
    case 'in_progress':
    case 'inprogress':
      return { ...base, color: 'var(--accent)', border: '1px solid var(--accent)' };
    default:
      return { ...base, color: 'var(--muted)', border: '1px solid var(--border)' };
  }
}

export const Settings: React.FC = () => {
  const nav = useNavigate();
  const { apply: applyTheme } = useTheme();

  const [identity, setIdentity] = useState<LocalIdentity | null>(null);
  const [deviceName, setDeviceName] = useState('');
  const [deviceNameError, setDeviceNameError] = useState<string | null>(null);

  const [trusted, setTrusted] = useState<TrustedPeer[]>([]);
  const [sharedDirs, setSharedDirs] = useState<string[]>([]);
  const [indexedDirs, setIndexedDirs] = useState<IndexedDir[]>([]);
  const [rescanning, setRescanning] = useState(false);

  const [hotkey, setHotkey] = useState('ctrl+space');
  const [recording, setRecording] = useState(false);

  const [downloadDir, setDownloadDir] = useState('');
  const [maxResults, setMaxResults] = useState('50');
  const [includeHidden, setIncludeHidden] = useState(false);
  const [notifications, setNotifications] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [theme, setTheme] = useState('dark');

  const [history, setHistory] = useState<TransferHistory[]>([]);

  const loadTrusted = useCallback(async () => {
    setTrusted(await invoke<TrustedPeer[]>('get_trusted_peers').catch(() => []));
  }, []);
  const loadSharedDirs = useCallback(async () => {
    setSharedDirs(await invoke<string[]>('get_shared_dirs').catch(() => []));
  }, []);
  const loadIndexedDirs = useCallback(async () => {
    setIndexedDirs(await invoke<IndexedDir[]>('get_indexed_dir_stats').catch(() => []));
  }, []);

  useEffect(() => {
    (async () => {
      const id = await invoke<LocalIdentity>('get_local_identity').catch(() => null);
      setIdentity(id);
      setDeviceName(id?.device_name ?? '');

      const settings = await invoke<Record<string, string>>('get_all_settings').catch(() => ({} as Record<string, string>));
      setMaxResults(settings.max_results ?? '50');
      setIncludeHidden(settings.include_hidden === 'true');
      setNotifications(settings.notifications_enabled !== 'false');
      setDownloadDir(settings.download_dir ?? '');
      setHotkey(settings.hotkey ?? 'ctrl+space');
      const t = settings.theme ?? 'dark';
      setTheme(t);
      applyTheme(t);

      setAutostart(await invoke<boolean>('get_autostart').catch(() => false));

      await loadTrusted();
      await loadSharedDirs();
      await loadIndexedDirs();
      setHistory(await invoke<TransferHistory[]>('get_transfer_history').catch(() => []));
    })();
  }, [loadTrusted, loadSharedDirs, loadIndexedDirs]);

  // --- Device ---
  const saveDeviceName = async () => {
    const name = deviceName.trim();
    if (!name) {
      setDeviceNameError('Device name cannot be empty.');
      return;
    }
    try {
      await invoke('set_device_name', { name });
      setDeviceNameError(null);
      const id = await invoke<LocalIdentity>('get_local_identity').catch(() => null);
      if (id) {
        setIdentity(id);
        setDeviceName(id.device_name);
      }
    } catch (e) {
      setDeviceNameError(String(e));
    }
  };

  // --- Trusted Devices ---
  const removePeer = async (deviceId: string) => {
    await invoke('revoke_peer_cmd', { device_id: deviceId }).catch(() => undefined);
    await loadTrusted();
  };

  // --- Shared Directories ---
  const addSharedDir = async () => {
    const path = await invoke<string | null>('open_dir_picker').catch(() => null);
    if (!path) return;
    await invoke('add_shared_dir', { path }).catch(() => undefined);
    await loadSharedDirs();
  };
  const removeSharedDir = async (path: string) => {
    await invoke('remove_shared_dir', { path }).catch(() => undefined);
    await loadSharedDirs();
  };

  // --- Indexed Directories ---
  const addIndexedDir = async () => {
    const path = await invoke<string | null>('open_dir_picker').catch(() => null);
    if (!path) return;
    setRescanning(true);
    await invoke('add_indexed_dir', { path }).catch(() => undefined);
    await loadIndexedDirs();
    setRescanning(false);
  };
  const removeIndexedDir = async (path: string) => {
    await invoke('remove_indexed_dir', { path }).catch(() => undefined);
    await loadIndexedDirs();
  };
  const rescanAll = async () => {
    setRescanning(true);
    await invoke('trigger_reindex').catch(() => undefined);
    await loadIndexedDirs();
    setRescanning(false);
  };

  // --- Hotkey recording ---
  const pendingHotkey = useRef<string | null>(null);
  useEffect(() => {
    if (!recording) return;
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === 'Escape') {
        pendingHotkey.current = null;
        setRecording(false);
        return;
      }
      // Ignore lone modifier presses; wait for a main key.
      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
      const parts: string[] = [];
      if (e.ctrlKey) parts.push('Ctrl');
      if (e.shiftKey) parts.push('Shift');
      if (e.altKey) parts.push('Alt');
      if (e.metaKey) parts.push('Super');
      parts.push(keyLabel(e.code));
      pendingHotkey.current = parts.join('+');
    };
    const onKeyUp = async (e: KeyboardEvent) => {
      e.preventDefault();
      const combo = pendingHotkey.current;
      if (!combo) return;
      pendingHotkey.current = null;
      try {
        await invoke('set_hotkey', { hotkey: combo });
        setHotkey(combo);
      } catch {
        /* keep the previous hotkey if the new one fails to register */
      }
      setRecording(false);
    };
    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('keyup', onKeyUp, true);
    return () => {
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
    };
  }, [recording]);

  // --- Preferences ---
  const changeDownloadDir = async () => {
    const path = await invoke<string | null>('open_dir_picker').catch(() => null);
    if (!path) return;
    setDownloadDir(path);
    await invoke('set_setting', { key: 'download_dir', value: path }).catch(() => undefined);
  };
  const saveMaxResults = async () => {
    await invoke('set_setting', { key: 'max_results', value: String(maxResults) }).catch(() => undefined);
  };
  const toggleIncludeHidden = async (checked: boolean) => {
    setIncludeHidden(checked);
    await invoke('set_setting', { key: 'include_hidden', value: String(checked) }).catch(() => undefined);
  };
  const toggleNotifications = async (checked: boolean) => {
    setNotifications(checked);
    await invoke('set_setting', { key: 'notifications_enabled', value: String(checked) }).catch(() => undefined);
  };
  const toggleAutostart = async (checked: boolean) => {
    setAutostart(checked);
    try {
      await invoke('set_autostart', { enabled: checked });
    } catch {
      setAutostart(!checked);
    }
  };
  const changeTheme = async (value: string) => {
    setTheme(value);
    applyTheme(value);
    await invoke('set_setting', { key: 'theme', value }).catch(() => undefined);
  };

  const peerName = (deviceId: string): string => {
    const match = trusted.find(p => p.device_id === deviceId);
    return match ? match.device_name : deviceId.slice(0, 12);
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
        {/* 1. Device */}
        <Section title="Device">
          <div className="flex flex-col gap-3 px-3 py-2 rounded" style={itemCard}>
            <div>
              <input
                className="rounded px-2 py-1 text-xs w-full"
                style={inputStyle}
                value={deviceName}
                onChange={e => setDeviceName(e.target.value)}
                onBlur={saveDeviceName}
                placeholder="Device name"
                maxLength={64}
              />
              {deviceNameError && (
                <p className="text-xs mt-1" style={{ color: 'var(--danger)' }}>{deviceNameError}</p>
              )}
            </div>
            <p className="text-xs break-all" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
              Device ID: {identity?.device_id ?? '...'}
            </p>
            <p className="text-xs break-all" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
              Public key fingerprint: {identity?.fingerprint ?? '...'}
            </p>
          </div>
        </Section>

        {/* 2. Trusted Devices */}
        <Section title="Trusted Devices">
          {trusted.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              No trusted devices. Pair a device from the main screen.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              {trusted.map(p => (
                <div key={p.device_id} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <div className="min-w-0">
                    <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>{p.device_name}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>{p.fingerprint.slice(0, 16)}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)' }}>
                      paired {relativeDate(p.paired_at)} · last seen {relativeDate(p.last_seen)}
                    </p>
                  </div>
                  <button onClick={() => removePeer(p.device_id)} className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors" style={dangerButton}>Remove</button>
                </div>
              ))}
            </div>
          )}
        </Section>

        {/* 3. Shared Directories */}
        <Section
          title="Shared Directories"
          action={<button onClick={addSharedDir} className="rounded px-2 py-1 text-xs transition-colors" style={subtleButton}>Add</button>}
        >
          {sharedDirs.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              No shared directories. Add one to allow file transfers from trusted devices.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              {sharedDirs.map(d => (
                <div key={d} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <p className="text-xs break-all" style={{ color: 'var(--text)', fontFamily: 'monospace' }}>{d}</p>
                  <button onClick={() => removeSharedDir(d)} className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors" style={dangerButton}>Remove</button>
                </div>
              ))}
            </div>
          )}
          <p className="text-xs" style={{ color: 'var(--muted)' }}>Trusted peers can only access files in these directories.</p>
        </Section>

        {/* 4. Indexed Directories */}
        <Section
          title="Indexed Directories"
          action={
            <div className="flex gap-2">
              <button onClick={rescanAll} disabled={rescanning} className="rounded px-2 py-1 text-xs transition-colors disabled:opacity-50" style={subtleButton}>
                {rescanning ? 'Scanning...' : 'Rescan All'}
              </button>
              <button onClick={addIndexedDir} disabled={rescanning} className="rounded px-2 py-1 text-xs transition-colors disabled:opacity-50" style={accentButton}>Add</button>
            </div>
          }
        >
          {indexedDirs.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              No indexed directories. Add one to start searching your files.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              {indexedDirs.map(d => (
                <div key={d.path} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <div className="min-w-0">
                    <p className="text-xs break-all" style={{ color: 'var(--text)', fontFamily: 'monospace' }}>{d.path}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)' }}>
                      {d.file_count.toLocaleString()} files · scanned {relativeDate(d.last_indexed)}
                    </p>
                  </div>
                  <button onClick={() => removeIndexedDir(d.path)} className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors" style={dangerButton}>Remove</button>
                </div>
              ))}
            </div>
          )}
          <p className="text-xs" style={{ color: 'var(--muted)' }}>These directories are indexed for local file search.</p>
        </Section>

        {/* 5. Hotkey */}
        <Section title="Hotkey">
          <div className="flex items-center justify-between px-3 py-2" style={itemCard}>
            <p className="text-xs" style={{ color: 'var(--text)', fontFamily: 'monospace' }}>
              {recording ? 'Press any key...' : prettyHotkey(hotkey)}
            </p>
            <button
              onClick={() => setRecording(r => !r)}
              className="rounded px-2 py-1 text-xs shrink-0 ml-2 transition-colors"
              style={recording ? accentButton : subtleButton}
            >
              {recording ? 'Press any key...' : 'Change'}
            </button>
          </div>
        </Section>

        {/* 6. Preferences */}
        <Section title="Preferences">
          <div className="flex flex-col gap-3 px-3 py-2 rounded" style={itemCard}>
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs" style={{ color: 'var(--text)' }}>Download directory</span>
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-xs truncate" style={{ color: 'var(--muted)', fontFamily: 'monospace', maxWidth: '18rem' }} title={downloadDir || 'Default'}>
                  {downloadDir || 'Default (Downloads/Synapt)'}
                </span>
                <button onClick={changeDownloadDir} className="rounded px-2 py-1 text-xs shrink-0 transition-colors" style={subtleButton}>Change</button>
              </div>
            </div>

            <label className="flex items-center justify-between">
              <span className="text-xs" style={{ color: 'var(--text)' }}>Max results</span>
              <input
                type="number"
                min={10}
                max={200}
                step={5}
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

            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={notifications}
                onChange={e => toggleNotifications(e.target.checked)}
                style={{ accentColor: 'var(--accent)' }}
              />
              <span className="text-xs" style={{ color: 'var(--text)' }}>System notifications</span>
            </label>

            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={autostart}
                onChange={e => toggleAutostart(e.target.checked)}
                style={{ accentColor: 'var(--accent)' }}
              />
              <span className="text-xs" style={{ color: 'var(--text)' }}>Start on login</span>
            </label>

            <label className="flex items-center justify-between">
              <span className="text-xs" style={{ color: 'var(--text)' }}>Theme</span>
              <select
                className="rounded px-2 py-1 text-xs w-28"
                style={inputStyle}
                value={theme}
                onChange={e => changeTheme(e.target.value)}
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">System</option>
              </select>
            </label>
          </div>
        </Section>

        {/* 7. Transfer History */}
        <Section title="Transfer History">
          {history.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>No transfers yet.</p>
          ) : (
            <div className="flex flex-col gap-2">
              {history.map((t, i) => (
                <div key={`${t.filename}-${t.started_at}-${i}`} className="flex items-center justify-between px-3 py-2" style={itemCard}>
                  <div className="min-w-0">
                    <p className="text-[13px]" style={{ color: 'var(--text)' }}>{t.filename}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)' }}>{peerName(t.peer_device_id)}</p>
                  </div>
                  <div className="text-right shrink-0 ml-2">
                    <span className="text-[10px] px-1.5 py-0.5 rounded" style={statusBadgeStyle(t.status)}>{t.status}</span>
                    <p className="text-xs mt-1" style={{ color: 'var(--muted)' }}>{formatBytes(t.size)} · {relativeDate(t.started_at)}</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Section>

        {/* 8. About */}
        <Section title="About">
          <div className="flex flex-col gap-1 px-3 py-2 rounded" style={itemCard}>
            <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>Synapt</p>
            <p className="text-xs" style={{ color: 'var(--muted)' }}>Version {APP_VERSION}</p>
            <p className="text-xs" style={{ color: 'var(--muted)' }}>Apache License 2.0</p>
            <a href={REPO_URL} target="_blank" rel="noreferrer" className="text-xs" style={{ color: 'var(--accent)' }}>
              {REPO_URL}
            </a>
          </div>
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
