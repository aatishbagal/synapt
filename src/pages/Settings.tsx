import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft } from 'lucide-react';
import { IndexProgress, InviteCodeInfo, IpcStatus, TrustedPeer } from '../types';
import { PairingDialog } from '../components/PairingDialog';
import { useTheme } from '../hooks/useTheme';
import { Select } from '../components/Select';
import { UnderlineLoader } from '../components/UnderlineLoader';

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

interface IncomingPair {
  device_id: string;
  device_name: string;
  verify_code: string;
}

const REPO_URL = 'https://github.com/aatishbagal/synapt';

/// Milliseconds to wait for the other device before giving up on a manual pair.
const PAIR_TIMEOUT_MS = 10000;

const IPV4_RE = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;

/// True when the text is a syntactically valid IPv4 address, so the entry field
/// can route it to pair_by_ip rather than treating it as an invite code.
function looksLikeIpv4(text: string): boolean {
  const m = IPV4_RE.exec(text.trim());
  return m !== null && m.slice(1).every(part => Number(part) <= 255);
}

/// Strip formatting from a typed invite code, leaving the alphabet characters.
function normalizeInviteCode(text: string): string {
  return text.toUpperCase().replace(/[^A-Z0-9]/g, '');
}

/// True for the RFC 1918 ranges a home or office LAN normally uses. Anything
/// else is worth warning about, since the two devices are likely not on one
/// subnet and the pairing connection will not get through.
function isPrivateLanIp(ip: string): boolean {
  const m = IPV4_RE.exec(ip.trim());
  if (!m) return false;
  const [a, b] = [Number(m[1]), Number(m[2])];
  if (a === 192 && b === 168) return true;
  if (a === 10) return true;
  if (a === 172 && b >= 16 && b <= 31) return true;
  return false;
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

/// An update offered by the endpoint, as returned by the check_for_update command.
interface UpdateInfo {
  version: string;
  current: string;
  notes: string;
}

/// Where the About section's update check has got to.
type UpdateState =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'available'; info: UpdateInfo }
  | { kind: 'none' }
  | { kind: 'installing' }
  | { kind: 'error'; message: string };

/// One-line status shown beside the update button.
function updateStatusLabel(state: UpdateState): string {
  switch (state.kind) {
    case 'idle':
      return 'Updates not checked yet';
    case 'checking':
      return 'Checking for updates...';
    case 'none':
      return 'Up to date';
    case 'available':
      return `Version ${state.info.version} available`;
    case 'installing':
      return 'Downloading, Synapt will restart when it finishes';
    case 'error':
      return `Could not check: ${state.message}`;
  }
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
  const [indexStatus, setIndexStatus] = useState<string | null>(null);
  const [indexActive, setIndexActive] = useState(false);

  const [hotkey, setHotkey] = useState('ctrl+space');
  const [recording, setRecording] = useState(false);

  const [downloadDir, setDownloadDir] = useState('');
  const [maxResults, setMaxResults] = useState('50');
  const [includeHidden, setIncludeHidden] = useState(false);
  const [notifications, setNotifications] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [theme, setTheme] = useState('dark');

  const [history, setHistory] = useState<TransferHistory[]>([]);
  const [ipcStatus, setIpcStatus] = useState<IpcStatus>({
    api_active: false,
    synaptclip_present: false,
    peer_count: 0,
  });
  // Null until a crash has actually been recorded, so the row stays hidden on
  // a healthy install.
  const [crashLogPath, setCrashLogPath] = useState<string | null>(null);

  // --- Add Device (manual pairing) ---
  const [pairTab, setPairTab] = useState<'enter' | 'show'>('enter');
  const [pairInput, setPairInput] = useState('');
  const [pairBusy, setPairBusy] = useState(false);
  const [pairError, setPairError] = useState('');
  const [localIp, setLocalIp] = useState<string | null>(null);
  const [inviteInfo, setInviteInfo] = useState<InviteCodeInfo | null>(null);
  const [inviteError, setInviteError] = useState('');
  const [manualVerify, setManualVerify] = useState<string | null>(null);
  const [incomingPair, setIncomingPair] = useState<IncomingPair | null>(null);

  const [appVersion, setAppVersion] = useState('');
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: 'idle' });
  const [autoUpdate, setAutoUpdate] = useState(true);

  const checkForUpdate = useCallback(async () => {
    setUpdateState({ kind: 'checking' });
    try {
      const info = await invoke<UpdateInfo | null>('check_for_update');
      setUpdateState(info ? { kind: 'available', info } : { kind: 'none' });
    } catch (e) {
      setUpdateState({ kind: 'error', message: String(e) });
    }
  }, []);

  const busy = updateState.kind === 'checking' || updateState.kind === 'installing';
  const statusTone =
    updateState.kind === 'available'
      ? 'var(--text)'
      : updateState.kind === 'error'
        ? 'var(--danger)'
        : 'var(--muted)';

  const toggleAutoUpdate = useCallback((enabled: boolean) => {
    setAutoUpdate(enabled);
    invoke('set_setting', { key: 'auto_update_check', value: String(enabled) }).catch(() => {});
  }, []);

  const installUpdate = useCallback(async () => {
    setUpdateState({ kind: 'installing' });
    try {
      // Succeeds by restarting into the new version, so nothing follows it.
      await invoke('install_update');
    } catch (e) {
      setUpdateState({ kind: 'error', message: `Install failed: ${String(e)}` });
    }
  }, []);

  const loadInviteCode = useCallback(async () => {
    try {
      setInviteInfo(await invoke<InviteCodeInfo>('generate_invite_code'));
      setInviteError('');
    } catch (e) {
      setInviteInfo(null);
      setInviteError(String(e));
    }
  }, []);

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
      setAutoUpdate(settings.auto_update_check !== 'false');
      setDownloadDir(settings.download_dir ?? '');
      setHotkey(settings.hotkey ?? 'ctrl+space');
      const t = settings.theme ?? 'dark';
      setTheme(t);
      applyTheme(t);

      setAutostart(await invoke<boolean>('get_autostart').catch(() => false));
      setCrashLogPath(await invoke<string | null>('get_crash_log_path').catch(() => null));
      setAppVersion(await invoke<string>('get_app_version').catch(() => ''));

      // Check on open when the preference allows it, so the section shows real
      // status rather than an inert button waiting to be clicked.
      if (settings.auto_update_check !== 'false') {
        void checkForUpdate();
      }
      setIpcStatus(await invoke<IpcStatus>('get_ipc_status').catch(() => ({ api_active: false, synaptclip_present: false, peer_count: 0 })));

      await loadTrusted();
      await loadSharedDirs();
      await loadIndexedDirs();
      const running = await invoke<boolean>('get_is_indexing').catch(() => false);
      setIndexActive(running);
      setIndexStatus(running ? 'Index scan running...' : null);
      setHistory(await invoke<TransferHistory[]>('get_transfer_history').catch(() => []));
      setLocalIp(await invoke<string | null>('get_local_ip').catch(() => null));
    })();
  }, [loadTrusted, loadSharedDirs, loadIndexedDirs, checkForUpdate]);

  // The code is generated lazily so a user who never opens the tab does not pay
  // for an interface lookup, and so returning to the tab reflects a changed IP.
  useEffect(() => {
    if (pairTab === 'show' && inviteInfo === null) {
      void loadInviteCode();
    }
  }, [pairTab, inviteInfo, loadInviteCode]);

  // Responder side. The overlay carries this listener too, but it is unmounted
  // while Settings is open, which is exactly where the user sits waiting for
  // someone to redeem their code.
  useEffect(() => {
    const unlisten = listen<IncomingPair>('pair-request', e => setIncomingPair(e.payload));
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // The startup check runs in the background, so surface its result if Settings
  // happens to be open when it lands.
  useEffect(() => {
    const unlisten = listen<UpdateInfo>('update-available', event => {
      setUpdateState({ kind: 'available', info: event.payload });
    });
    return () => {
      unlisten.then(fn => fn()).catch(() => undefined);
    };
  }, []);

  // Reflect live indexing progress in the Indexed Directories status line.
  useEffect(() => {
    let clearTimer: ReturnType<typeof setTimeout> | null = null;
    const truncate = (s: string, n: number) => (s.length > n ? `...${s.slice(-(n - 3))}` : s);
    const unlisten = listen<IndexProgress>('index-progress', event => {
      const { phase, current_dir } = event.payload;
      if (clearTimer) {
        clearTimeout(clearTimer);
        clearTimer = null;
      }
      switch (phase.type) {
        case 'Starting':
        case 'BuildingIndex':
          setIndexActive(true);
          setIndexStatus('Index scan running...');
          break;
        case 'Scanning':
          setIndexActive(true);
          setIndexStatus(`Scanning: ${truncate(current_dir, 40)}`);
          break;
        case 'Complete':
          setIndexActive(false);
          setIndexStatus(`Last scan: ${phase.total_files.toLocaleString()} files`);
          void loadIndexedDirs();
          clearTimer = setTimeout(() => setIndexStatus(null), 5000);
          break;
        case 'Failed':
          setIndexActive(false);
          setIndexStatus(`Index failed: ${phase.reason}`);
          clearTimer = setTimeout(() => setIndexStatus(null), 5000);
          break;
      }
    });
    return () => {
      unlisten.then(fn => fn()).catch(() => undefined);
      if (clearTimer) clearTimeout(clearTimer);
    };
  }, [loadIndexedDirs]);

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

  // --- Add Device (manual pairing) ---
  // The joining side dials the other device. The address may be typed as a raw
  // IPv4 address or as an invite code, so the field accepts either.
  const startManualPair = async () => {
    const entry = pairInput.trim();
    if (entry.length === 0 || pairBusy) return;

    setPairBusy(true);
    setPairError('');

    const request = looksLikeIpv4(entry)
      ? invoke<string>('pair_by_ip', { ip: entry })
      : invoke<string>('redeem_invite_code', { code: normalizeInviteCode(entry) });

    // begin_pairing has its own connect timeout, but a host that accepts the
    // TCP connection and then stalls would otherwise leave the button spinning.
    const timeout = new Promise<never>((_, reject) =>
      setTimeout(
        () =>
          reject(
            new Error(
              'Could not reach that device. Check the address and make sure Synapt is running on the other device.',
            ),
          ),
        PAIR_TIMEOUT_MS,
      ),
    );

    try {
      const verify = await Promise.race([request, timeout]);
      setManualVerify(verify);
      setPairInput('');
    } catch (e) {
      setPairError(e instanceof Error ? e.message : String(e));
    } finally {
      setPairBusy(false);
    }
  };

  const closeManualPair = () => {
    setManualVerify(null);
    void loadTrusted();
  };

  // --- Trusted Devices ---
  const removePeer = async (deviceId: string) => {
    await invoke('revoke_peer_cmd', { deviceId }).catch(() => undefined);
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

        {/* 3. Add Device */}
        <Section title="Add Device">
          <div className="flex gap-1">
            {(['enter', 'show'] as const).map(tab => (
              <button
                key={tab}
                onClick={() => setPairTab(tab)}
                className="rounded px-2 py-1 text-xs transition-colors"
                style={pairTab === tab ? accentButton : subtleButton}
              >
                {tab === 'enter' ? 'Enter IP or code' : 'Show my code'}
              </button>
            ))}
          </div>

          {pairTab === 'enter' ? (
            <div className="flex flex-col gap-2">
              <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
                This device IP: {localIp ?? 'unavailable'}
              </p>
              <div className="flex gap-2">
                <input
                  value={pairInput}
                  onChange={e => {
                    setPairInput(e.target.value);
                    setPairError('');
                  }}
                  onKeyDown={e => {
                    if (e.key === 'Enter') void startManualPair();
                  }}
                  placeholder="IP address or invite code"
                  spellCheck={false}
                  autoCapitalize="characters"
                  className="flex-1 rounded px-2 py-1 text-xs outline-none"
                  style={inputStyle}
                />
                <button
                  onClick={() => void startManualPair()}
                  disabled={pairBusy || pairInput.trim().length === 0}
                  className="rounded px-2 py-1 text-xs shrink-0 transition-colors disabled:opacity-50"
                  style={accentButton}
                >
                  {pairBusy ? 'Connecting...' : 'Connect and Pair'}
                </button>
              </div>
              {pairError && (
                <p className="text-xs" style={{ color: 'var(--danger)' }}>{pairError}</p>
              )}
              <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
                Enter the IP address shown on the other device, or paste their invite code.
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              {inviteError ? (
                <p className="text-xs" style={{ color: 'var(--danger)' }}>{inviteError}</p>
              ) : (
                <p
                  className="text-center"
                  style={{
                    fontFamily: 'monospace',
                    fontSize: '28px',
                    letterSpacing: '0.15em',
                    color: 'var(--text)',
                    backgroundColor: 'var(--surface)',
                    border: '1px solid var(--border)',
                    borderRadius: '8px',
                    padding: '16px 24px',
                  }}
                >
                  {inviteInfo?.code ?? '...'}
                </p>
              )}
              {inviteInfo && (
                <>
                  <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>
                    This device IP: {inviteInfo.local_ip}
                  </p>
                  <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
                    Share this code or your IP with the other device to start pairing.
                  </p>
                  {!isPrivateLanIp(inviteInfo.local_ip) && (
                    <p className="text-[11px]" style={{ color: 'var(--warning)' }}>
                      Your device may be on an unusual network. If pairing fails, check that both
                      devices are on the same Wi-Fi network.
                    </p>
                  )}
                </>
              )}
              <button
                onClick={() => void loadInviteCode()}
                className="text-[11px] self-start transition-colors"
                style={{ color: 'var(--muted)', background: 'none', border: 'none', padding: 0 }}
              >
                Generate new code
              </button>
            </div>
          )}
        </Section>

        {/* 4. Shared Directories */}
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

        {/* 5. Indexed Directories */}
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
          {indexStatus && (
            <div className="flex items-center gap-2" style={{ marginTop: 6 }}>
              {indexActive && <UnderlineLoader state="active" width={20} />}
              <p className="text-xs" style={{ color: 'var(--muted)' }}>{indexStatus}</p>
            </div>
          )}
          <p className="text-xs" style={{ color: 'var(--muted)' }}>These directories are indexed for local file search.</p>
        </Section>

        {/* 6. Hotkey */}
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

        {/* 7. Preferences */}
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
              <Select
                value={theme}
                options={[
                  { value: 'dark', label: 'Dark' },
                  { value: 'light', label: 'Light' },
                  { value: 'system', label: 'System' },
                ]}
                onChange={changeTheme}
              />
            </label>
          </div>
        </Section>

        {/* 8. Transfer History */}
        <Section title="Transfers">
          <button
            type="button"
            onClick={() => nav('/transfers')}
            className="flex items-center justify-between px-3 py-2 w-full text-left"
            style={itemCard}
          >
            <span className="text-[13px]" style={{ color: 'var(--text)' }}>Transfer history</span>
            <span className="text-xs" style={{ color: 'var(--muted)' }}>
              {history.length} {history.length === 1 ? 'transfer' : 'transfers'} &rsaquo;
            </span>
          </button>
        </Section>

        {/* 9. About */}
        <Section title="About">
          <div className="flex flex-col gap-1 px-3 py-2 rounded" style={itemCard}>
            <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>Synapt</p>
            <p className="text-xs" style={{ color: 'var(--muted)' }}>Version {appVersion}</p>
            <p className="text-xs" style={{ color: 'var(--muted)' }}>Apache License 2.0</p>
            <a href={REPO_URL} target="_blank" rel="noreferrer" className="text-xs" style={{ color: 'var(--accent)' }}>
              {REPO_URL}
            </a>
            <div className="flex flex-col gap-1.5 mt-1">
              {/* Integration API */}
              <div className="flex items-center justify-between">
                <span className="text-xs" style={{ color: 'var(--muted)' }}>Integration API</span>
                <span className="flex items-center gap-1.5">
                  <span
                    className="inline-block rounded-full"
                    style={{ width: 7, height: 7, backgroundColor: ipcStatus.api_active ? 'var(--accent)' : 'var(--border)' }}
                  />
                  <span className="text-xs" style={{ color: ipcStatus.api_active ? 'var(--accent)' : 'var(--muted)' }}>
                    {ipcStatus.api_active ? 'Active' : 'Inactive'}
                  </span>
                </span>
              </div>
              {/* SynaptClip */}
              <div className="flex items-center justify-between">
                <span className="text-xs" style={{ color: 'var(--muted)' }}>SynaptClip</span>
                <span className="flex items-center gap-1.5">
                  <span
                    className="inline-block rounded-full"
                    style={{ width: 7, height: 7, backgroundColor: ipcStatus.synaptclip_present ? 'var(--accent)' : 'var(--border)' }}
                  />
                  <span className="text-xs" style={{ color: ipcStatus.synaptclip_present ? 'var(--accent)' : 'var(--muted)' }}>
                    {ipcStatus.synaptclip_present ? 'Detected' : 'Not running'}
                  </span>
                </span>
              </div>
              <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
                {ipcStatus.synaptclip_present
                  ? `${ipcStatus.peer_count} peer(s) available for clipboard sync`
                  : 'Install SynaptClip to enable cross-device clipboard sync'}
              </p>
              {/* Updates */}
              <div
                className="flex flex-col gap-2 mt-1 pt-2"
                style={{ borderTop: '1px solid var(--border)' }}
              >
                <div className="flex items-center justify-between gap-3">
                  <span className="flex items-center gap-2 min-w-0">
                    {busy && <UnderlineLoader state="active" width={16} />}
                    <span className="text-xs truncate" style={{ color: statusTone }}>
                      {updateStatusLabel(updateState)}
                    </span>
                  </span>
                  {updateState.kind === 'available' ? (
                    <button
                      type="button"
                      onClick={installUpdate}
                      className="rounded px-2 py-1 text-xs shrink-0 transition-colors"
                      style={accentButton}
                    >
                      Install and restart
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={checkForUpdate}
                      disabled={busy}
                      className="rounded px-2 py-1 text-xs shrink-0 transition-colors"
                      style={{ ...subtleButton, opacity: busy ? 0.5 : 1 }}
                    >
                      {updateState.kind === 'idle' ? 'Check for updates' : 'Check again'}
                    </button>
                  )}
                </div>

                {updateState.kind === 'available' && updateState.info.notes && (
                  <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
                    {updateState.info.notes}
                  </p>
                )}

                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={autoUpdate}
                    onChange={e => toggleAutoUpdate(e.target.checked)}
                    style={{ accentColor: 'var(--accent)' }}
                  />
                  <span className="text-[11px]" style={{ color: 'var(--muted)' }}>
                    Check for updates automatically
                  </span>
                </label>
              </div>
              {crashLogPath && (
                <div className="flex flex-col gap-0.5 mt-1">
                  <p className="text-[11px] break-all" style={{ color: 'var(--muted)' }}>
                    Crash log: {crashLogPath}
                  </p>
                  <button
                    type="button"
                    className="text-[11px] text-left"
                    style={{ color: 'var(--accent)' }}
                    onClick={() => {
                      invoke('reveal_in_files', { path: crashLogPath }).catch(() => {});
                    }}
                  >
                    Open in file manager
                  </button>
                </div>
              )}
            </div>
          </div>
        </Section>
      </div>

      {manualVerify !== null && (
        <PairingDialog mode="manual" verifyCode={manualVerify} onClose={closeManualPair} />
      )}

      {incomingPair && (
        <PairingDialog
          mode="responder"
          incomingPair={incomingPair}
          onClose={() => {
            setIncomingPair(null);
            void loadTrusted();
          }}
        />
      )}
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
