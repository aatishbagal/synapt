import { useCoreUpdate } from '../hooks/useCoreUpdate';
import { CoreUpdateNotice } from '../components/CoreUpdateNotice';
import React, { useCallback, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft } from 'lucide-react';
import { IndexProgress, InviteCodeInfo, IpcStatus, TrustedPeer } from '../types';
import { PairingDialog } from '../components/PairingDialog';
import { useTheme } from '../hooks/useTheme';
import { useHotkeyInfo } from '../hooks/useHotkeyInfo';
import { Select } from '../components/Select';
import { UnderlineLoader } from '../components/UnderlineLoader';
import { InlineConfirm } from '../components/InlineConfirm';
import { ExperimentalSettings } from '../components/ExperimentalSettings';
import { EXPERIMENTAL_UI_AVAILABLE } from '../experimental';
import { WhatsNewModal, WhatsNewEntry } from '../components/WhatsNewModal';
import { AnimatePresence } from 'motion/react';
import { describeError } from '../utils/errors';
import { prettyHotkey } from '../utils/hotkey';
import { useHotkeyRecorder } from '../hooks/useHotkeyRecorder';

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

/// Active state of a toggle button (segmented tabs, hotkey recorder), per the
/// DESIGN.md interactive states: --surface-hover fill with an --accent label.
const activeToggle: React.CSSProperties = {
  backgroundColor: 'var(--surface-hover)',
  color: 'var(--accent)',
};

/// Settings sections that can report a failed action next to where it happened.
type SectionKey = 'device' | 'trusted' | 'shared' | 'indexed' | 'preferences' | 'about';

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
  const shortcut = useHotkeyInfo();

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

  // A failed action is reported inside the section it came from, in plain
  // language, instead of being swallowed.
  const [sectionErrors, setSectionErrors] = useState<Partial<Record<SectionKey, string>>>({});
  const fail = useCallback((section: SectionKey, error: unknown, fallback: string) => {
    setSectionErrors(prev => ({ ...prev, [section]: describeError(error, fallback) }));
  }, []);
  const clearError = useCallback((section: SectionKey) => {
    setSectionErrors(prev => (prev[section] ? { ...prev, [section]: undefined } : prev));
  }, []);
  const [confirmRemovePeer, setConfirmRemovePeer] = useState<string | null>(null);

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

  const coreUpdate = useCoreUpdate();
  const checkCoreUpdate = coreUpdate.check;
  const [appVersion, setAppVersion] = useState('');
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: 'idle' });
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [crashReports, setCrashReports] = useState(true);
  const [releaseNotes, setReleaseNotes] = useState<WhatsNewEntry | null>(null);

  const checkForUpdate = useCallback(async () => {
    setUpdateState({ kind: 'checking' });
    const coreCheck = checkCoreUpdate();
    try {
      const info = await invoke<UpdateInfo | null>('check_for_update');
      setUpdateState(info ? { kind: 'available', info } : { kind: 'none' });
    } catch (e) {
      setUpdateState({ kind: 'error', message: describeError(e, 'Check your internet connection and try again.') });
    } finally {
      await coreCheck;
    }
  }, [checkCoreUpdate]);

  const busy = coreUpdate.checking || updateState.kind === 'checking' || updateState.kind === 'installing';
  const statusTone =
    updateState.kind === 'available'
      ? 'var(--text)'
      : updateState.kind === 'error'
        ? 'var(--danger)'
        : 'var(--muted)';

  const toggleAutoUpdate = useCallback((enabled: boolean) => {
    setAutoUpdate(enabled);
    invoke('set_setting', { key: 'auto_update_check', value: String(enabled) }).catch(e => {
      setAutoUpdate(!enabled);
      fail('about', e, 'Could not save this setting. Try again.');
    });
  }, [fail]);

  const showReleaseNotes = useCallback(() => {
    invoke<WhatsNewEntry[]>('get_whats_new')
      .then(versions => setReleaseNotes(versions[0] ?? null))
      .catch(e => fail('about', e, 'Could not load the release notes.'));
  }, [fail]);

  const openChangelog = useCallback(() => {
    invoke('open_changelog').catch(e => fail('about', e, 'Could not open the changelog in your browser.'));
  }, [fail]);

  const toggleCrashReports = useCallback((enabled: boolean) => {
    setCrashReports(enabled);
    invoke('set_setting', { key: 'crash_reports_enabled', value: String(enabled) }).catch(e => {
      setCrashReports(!enabled);
      fail('about', e, 'Could not save this setting. Try again.');
    });
  }, [fail]);

  const installUpdate = useCallback(async () => {
    setUpdateState({ kind: 'installing' });
    try {
      // Succeeds by restarting into the new version, so nothing follows it.
      await invoke('install_update');
    } catch (e) {
      setUpdateState({ kind: 'error', message: describeError(e, 'The update could not be installed. Try again later.') });
    }
  }, []);

  const loadInviteCode = useCallback(async () => {
    try {
      setInviteInfo(await invoke<InviteCodeInfo>('generate_invite_code'));
      setInviteError('');
    } catch (e) {
      setInviteInfo(null);
      setInviteError(describeError(e, 'Could not create an invite code. Check your network connection and try again.'));
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
      setCrashReports(settings.crash_reports_enabled !== 'false');
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
      setDeviceNameError(describeError(e, 'Could not save the device name. Try again.'));
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
      setPairError(describeError(e, 'Pairing failed. Check the address or code and try again.'));
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
    setConfirmRemovePeer(null);
    clearError('trusted');
    await invoke('revoke_peer_cmd', { deviceId }).catch(e =>
      fail('trusted', e, 'Could not remove the device. Try again.'),
    );
    await loadTrusted();
  };

  // --- Shared Directories ---
  const addSharedDir = async () => {
    const path = await invoke<string | null>('open_dir_picker').catch(() => null);
    if (!path) return;
    clearError('shared');
    await invoke('add_shared_dir', { path }).catch(e =>
      fail('shared', e, 'Could not share that folder. Try again.'),
    );
    await loadSharedDirs();
  };
  const removeSharedDir = async (path: string) => {
    clearError('shared');
    await invoke('remove_shared_dir', { path }).catch(e =>
      fail('shared', e, 'Could not stop sharing that folder. Try again.'),
    );
    await loadSharedDirs();
  };

  // --- Indexed Directories ---
  const addIndexedDir = async () => {
    const path = await invoke<string | null>('open_dir_picker').catch(() => null);
    if (!path) return;
    setRescanning(true);
    clearError('indexed');
    await invoke('add_indexed_dir', { path }).catch(e =>
      fail('indexed', e, 'Could not add that folder to the index. Try again.'),
    );
    await loadIndexedDirs();
    setRescanning(false);
  };
  const removeIndexedDir = async (path: string) => {
    clearError('indexed');
    await invoke('remove_indexed_dir', { path }).catch(e =>
      fail('indexed', e, 'Could not remove that folder from the index. Try again.'),
    );
    await loadIndexedDirs();
  };
  const rescanAll = async () => {
    setRescanning(true);
    clearError('indexed');
    await invoke('trigger_reindex').catch(e => fail('indexed', e, 'Could not start the rescan. Try again.'));
    await loadIndexedDirs();
    setRescanning(false);
  };

  // --- Hotkey recording ---
  const { recording, setRecording, error: hotkeyError } = useHotkeyRecorder(setHotkey);

  // --- Preferences ---
  const changeDownloadDir = async () => {
    const path = await invoke<string | null>('open_dir_picker').catch(() => null);
    if (!path) return;
    const previous = downloadDir;
    setDownloadDir(path);
    await savePreference('download_dir', path, () => setDownloadDir(previous));
  };
  /// Persist one preference; on failure undo the optimistic UI change and say so.
  const savePreference = async (key: string, value: string, revert: () => void) => {
    clearError('preferences');
    try {
      await invoke('set_setting', { key, value });
    } catch (e) {
      revert();
      fail('preferences', e, 'Could not save this setting. Try again.');
    }
  };
  const saveMaxResults = async () => {
    await savePreference('max_results', String(maxResults), () => undefined);
  };
  const toggleIncludeHidden = async (checked: boolean) => {
    setIncludeHidden(checked);
    await savePreference('include_hidden', String(checked), () => setIncludeHidden(!checked));
  };
  const toggleNotifications = async (checked: boolean) => {
    setNotifications(checked);
    await savePreference('notifications_enabled', String(checked), () => setNotifications(!checked));
  };
  const toggleAutostart = async (checked: boolean) => {
    setAutostart(checked);
    clearError('preferences');
    try {
      await invoke('set_autostart', { enabled: checked });
    } catch (e) {
      setAutostart(!checked);
      fail('preferences', e, 'Could not change the start-on-login setting. Try again.');
    }
  };
  const changeTheme = async (value: string) => {
    const previous = theme;
    setTheme(value);
    applyTheme(value);
    await savePreference('theme', value, () => {
      setTheme(previous);
      applyTheme(previous);
    });
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
          title="Back to search"
          className="icon-button icon-button-labeled"
        >
          <ArrowLeft size={12} />
          <span>Back</span>
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3 flex flex-col gap-6">
        {/* 1. Device */}
        <Section title="Device" error={sectionErrors.device}>
          <div className="flex flex-col gap-3 px-3 py-2 rounded" style={itemCard}>
            <div>
              <input
                className="input w-full"
                value={deviceName}
                onChange={e => setDeviceName(e.target.value)}
                onBlur={saveDeviceName}
                placeholder="Device name"
                maxLength={64}
              />
              {deviceNameError && (
                <p role="alert" className="error-text mt-1">{deviceNameError}</p>
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
        <Section title="Trusted Devices" error={sectionErrors.trusted}>
          {trusted.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              No paired devices yet. Use Add Device below to pair one.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              {trusted.map(p => (
                <div
                  key={p.device_id}
                  className="flex items-center justify-between px-3 py-2"
                  style={{ ...itemCard, position: 'relative' }}
                >
                  <AnimatePresence>
                    {confirmRemovePeer === p.device_id && (
                      <InlineConfirm
                        key="remove"
                        tone="danger"
                        message={`Remove ${p.device_name}? You will need to pair it again to share files.`}
                        confirmLabel="Remove"
                        onConfirm={() => void removePeer(p.device_id)}
                        onCancel={() => setConfirmRemovePeer(null)}
                      />
                    )}
                  </AnimatePresence>
                  <div className="min-w-0">
                    <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>{p.device_name}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)', fontFamily: 'monospace' }}>{p.fingerprint.slice(0, 16)}</p>
                    <p className="text-xs" style={{ color: 'var(--muted)' }}>
                      paired {relativeDate(p.paired_at)} | last seen {relativeDate(p.last_seen)}
                    </p>
                  </div>
                  <button onClick={() => setConfirmRemovePeer(p.device_id)} className="btn btn-destructive shrink-0 ml-2">Remove</button>
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
                aria-pressed={pairTab === tab}
                className="btn btn-secondary"
                style={pairTab === tab ? activeToggle : undefined}
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
                  className="input flex-1"
                />
                <button
                  onClick={() => void startManualPair()}
                  disabled={pairBusy || pairInput.trim().length === 0}
                  className="btn btn-primary shrink-0"
                >
                  {pairBusy ? 'Connecting...' : 'Connect and Pair'}
                </button>
              </div>
              {pairError && (
                <p role="alert" className="error-text">{pairError}</p>
              )}
              <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
                Enter the IP address shown on the other device, or paste their invite code.
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              {inviteError ? (
                <p role="alert" className="error-text">{inviteError}</p>
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
              <button onClick={() => void loadInviteCode()} className="btn btn-ghost self-start">
                Generate new code
              </button>
            </div>
          )}
        </Section>

        {/* 4. Shared Directories */}
        <Section
          title="Shared Directories"
          error={sectionErrors.shared}
          action={<button onClick={addSharedDir} className="btn btn-secondary">Add</button>}
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
                  <button onClick={() => removeSharedDir(d)} className="btn btn-destructive shrink-0 ml-2">Remove</button>
                </div>
              ))}
            </div>
          )}
          <p className="text-xs" style={{ color: 'var(--muted)' }}>Trusted peers can only access files in these directories.</p>
        </Section>

        {/* 5. Indexed Directories */}
        <Section
          title="Indexed Directories"
          error={sectionErrors.indexed}
          action={
            <div className="flex gap-2">
              <button onClick={rescanAll} disabled={rescanning} className="btn btn-secondary">
                {rescanning ? 'Scanning...' : 'Rescan All'}
              </button>
              <button onClick={addIndexedDir} disabled={rescanning} className="btn btn-primary">Add</button>
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
                      {d.file_count.toLocaleString()} files | scanned {relativeDate(d.last_indexed)}
                    </p>
                  </div>
                  <button onClick={() => removeIndexedDir(d.path)} className="btn btn-destructive shrink-0 ml-2">Remove</button>
                </div>
              ))}
            </div>
          )}
          {indexStatus && (
            <div role="status" className="flex items-center gap-2" style={{ marginTop: 8 }}>
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
              {shortcut.info?.wayland
                ? shortcut.info.trigger ?? 'Shortcut not active'
                : recording ? 'Press any key...' : prettyHotkey(hotkey)}
            </p>
            <button
              disabled={shortcut.busy || shortcut.info === null}
              onClick={() => {
                if (shortcut.info?.wayland) void shortcut.configure();
                else setRecording(r => !r);
              }}
              aria-pressed={recording}
              className="btn btn-secondary shrink-0 ml-2"
              style={recording ? activeToggle : undefined}
            >
              {shortcut.busy ? 'Waiting for desktop...' : shortcut.info?.wayland ? 'Configure shortcut' : recording ? 'Press any key...' : 'Change'}
            </button>
          </div>
          {shortcut.info?.wayland && (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              Your desktop manages this shortcut and may ask for permission.
            </p>
          )}
          {(shortcut.error || shortcut.info?.error || hotkeyError) && (
            <p role="alert" className="error-text">
              {shortcut.error || shortcut.info?.error || hotkeyError}
            </p>
          )}
        </Section>

        {/* 7. Preferences */}
        <Section title="Preferences" error={sectionErrors.preferences}>
          <div className="flex flex-col gap-3 px-3 py-2 rounded" style={itemCard}>
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs" style={{ color: 'var(--text)' }}>Download directory</span>
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-xs truncate" style={{ color: 'var(--muted)', fontFamily: 'monospace', maxWidth: '18rem' }} title={downloadDir || 'Default'}>
                  {downloadDir || 'Default (Downloads/Synapt)'}
                </span>
                <button onClick={changeDownloadDir} className="btn btn-secondary shrink-0">Change</button>
              </div>
            </div>

            <label className="flex items-center justify-between">
              <span className="text-xs" style={{ color: 'var(--text)' }}>Max results</span>
              <input
                type="number"
                min={10}
                max={200}
                step={5}
                className="input w-24"
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
        <Section title="About" error={sectionErrors.about}>
          <div className="flex flex-col gap-1 px-3 py-2 rounded" style={itemCard}>
            <p className="text-[13px] font-medium" style={{ color: 'var(--text)' }}>Synapt</p>
            <p className="text-xs" style={{ color: 'var(--muted)' }}>Version {appVersion}</p>
            <p className="text-xs" style={{ color: 'var(--muted)' }}>Apache License 2.0</p>
            <a href={REPO_URL} target="_blank" rel="noreferrer" className="text-xs" style={{ color: 'var(--accent)' }}>
              {REPO_URL}
            </a>
            <button type="button" onClick={showReleaseNotes} className="btn btn-secondary self-start mt-1">
              See release notes
            </button>
            <div className="flex flex-col gap-2 mt-1">
              {/* Integration API */}
              <div className="flex items-center justify-between">
                <span className="text-xs" style={{ color: 'var(--muted)' }}>Integration API</span>
                <span className="text-xs" style={{ color: ipcStatus.api_active ? 'var(--text)' : 'var(--muted)' }}>
                  {ipcStatus.api_active ? 'Active' : 'Inactive'}
                </span>
              </div>
              {/* SynaptClip */}
              <div className="flex items-center justify-between">
                <span className="text-xs" style={{ color: 'var(--muted)' }}>SynaptClip</span>
                <span className="text-xs" style={{ color: ipcStatus.synaptclip_present ? 'var(--text)' : 'var(--muted)' }}>
                  {ipcStatus.synaptclip_present ? 'Detected' : 'Not running'}
                </span>
              </div>
              <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
                {ipcStatus.synaptclip_present
                  ? `${ipcStatus.peer_count} ${ipcStatus.peer_count === 1 ? 'device' : 'devices'} available for clipboard sync`
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
                      className="btn btn-primary shrink-0"
                    >
                      Install and restart
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={checkForUpdate}
                      disabled={busy}
                      className="btn btn-secondary shrink-0"
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

                <CoreUpdateNotice status={coreUpdate.status} checking={coreUpdate.checking} error={coreUpdate.error} />

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
              <label className="flex items-center gap-2 cursor-pointer mt-1">
                <input
                  type="checkbox"
                  checked={crashReports}
                  onChange={e => toggleCrashReports(e.target.checked)}
                  style={{ accentColor: 'var(--accent)' }}
                />
                <span className="text-[11px]" style={{ color: 'var(--muted)' }}>
                  Offer to email a crash report after a crash
                </span>
              </label>
              {crashLogPath && (
                <div className="flex flex-col mt-1">
                  <p className="text-[11px] break-all" style={{ color: 'var(--muted)' }}>
                    Crash log: {crashLogPath}
                  </p>
                  <button
                    type="button"
                    className="text-[11px] text-left"
                    style={{ color: 'var(--accent)' }}
                    onClick={() => {
                      invoke('reveal_in_files', { path: crashLogPath }).catch(e =>
                        fail('about', e, 'Could not open the file manager.'),
                      );
                    }}
                  >
                    Open in file manager
                  </button>
                </div>
              )}
            </div>
          </div>
        </Section>

        {/* 10. Experimental */}
        {EXPERIMENTAL_UI_AVAILABLE && (
          <Section title="Experimental">
            <div className="px-3 py-2" style={itemCard}>
              <ExperimentalSettings />
            </div>
          </Section>
        )}
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

      <AnimatePresence>
        {releaseNotes && (
          <WhatsNewModal
            key="whats-new"
            title="What's new in Synapt"
            data={releaseNotes}
            onDismiss={() => setReleaseNotes(null)}
            onSeeAll={openChangelog}
          />
        )}
      </AnimatePresence>
    </div>
  );
};

interface SectionProps {
  title: string;
  action?: React.ReactNode;
  /** Plain-language message for the last failed action in this section. */
  error?: string;
  children: React.ReactNode;
}

function Section({ title, action, error, children }: SectionProps) {
  // gap-2 supplies the heading's 8px bottom margin, so it is zeroed here.
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="section-heading mb-0">{title}</h2>
        {action}
      </div>
      {children}
      {error && (
        <p role="alert" className="error-text">
          {error}
        </p>
      )}
    </div>
  );
}
