import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Settings as SettingsIcon } from 'lucide-react';
import { PairingDialog } from '../components/PairingDialog';
import { SearchBar } from '../components/SearchBar';
import { ResultList } from '../components/ResultList';
import { DevicePicker } from '../components/DevicePicker';
import { SearchLoadingBar } from '../components/SearchLoadingBar';
import { TransferCard } from '../components/TransferCard';
import { IndexingBanner } from '../components/IndexingBanner';
import { Peer, SearchResult, TrustedPeer, DeviceOption, IndexProgress, IpcStatus } from '../types';
import { useTheme } from '../hooks/useTheme';
import { parseInput } from '../utils/parseInput';
import { stepDown, stepUp } from '../utils/navigation';

interface IncomingPair {
  device_id: string;
  device_name: string;
  verify_code: string;
}

export const Overlay: React.FC = () => {
  const [incomingPair, setIncomingPair] = useState<IncomingPair | null>(null);

  const [inputValue, setInputValue] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  // -1 means nothing is highlighted yet; navigation begins highlighting at 0.
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [loading, setLoading] = useState(false);
  const [remoteSearchLoading, setRemoteSearchLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [noIndexedDirs, setNoIndexedDirs] = useState(false);
  const [foldersIndexed, setFoldersIndexed] = useState<boolean | null>(null);
  const [rescanning, setRescanning] = useState(false);

  const [availableDevices, setAvailableDevices] = useState<DeviceOption[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<DeviceOption | null>(null);
  const [showDevicePicker, setShowDevicePicker] = useState(false);
  const [devicePickerIndex, setDevicePickerIndex] = useState(0);
  const [confirmingTransfer, setConfirmingTransfer] = useState<SearchResult | null>(null);
  const [expandedResultPath, setExpandedResultPath] = useState<string | null>(null);
  const [expandedActionIndex, setExpandedActionIndex] = useState(0);
  const [sendToDevicesPath, setSendToDevicesPath] = useState<string | null>(null);

  const [ipcStatus, setIpcStatus] = useState<IpcStatus | null>(null);

  const searchInputRef = useRef<HTMLInputElement>(null);

  const { apply: applyTheme } = useTheme();
  const nav = useNavigate();

  // Subtle SynaptClip presence indicator; refreshed every 30 seconds.
  useEffect(() => {
    const load = () => {
      invoke<IpcStatus>('get_ipc_status').then(setIpcStatus).catch(() => undefined);
    };
    load();
    const id = setInterval(load, 30000);
    return () => clearInterval(id);
  }, []);

  const parsedInput = useMemo(
    () => parseInput(inputValue, selectedDevice !== null),
    [inputValue, selectedDevice],
  );

  const deviceFilter = inputValue.startsWith('@') ? inputValue.slice(1).toLowerCase() : '';
  const filteredDevices = useMemo(
    () => availableDevices.filter(d => d.device_name.toLowerCase().startsWith(deviceFilter)),
    [availableDevices, deviceFilter],
  );

  // A leading '/' means folder search, both locally and on a tagged remote
  // device (the remote engine routes '/' queries to a directory-only search).
  const isFolderQuery =
    parsedInput.mode === 'folder' ||
    (selectedDevice !== null && parsedInput.query.startsWith('/'));

  // Reconcile the theme against the persisted setting on mount (the theme is
  // changed from Settings; the overlay only applies it).
  useEffect(() => {
    invoke<string | null>('get_setting', { key: 'theme' })
      .then(t => applyTheme(t ?? 'dark'))
      .catch(() => undefined);
    // applyTheme is stable for our purposes; run once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Refresh the @ picker's device list. Presence does not need to be real-time
  // here, so a 10s poll is sufficient.
  useEffect(() => {
    const fetchDevices = async () => {
      const trusted = await invoke<TrustedPeer[]>('get_trusted_peers').catch(() => []);
      const peers = await invoke<Peer[]>('get_peers').catch(() => []);
      const online = new Set(peers.map(p => p.device_id));
      setAvailableDevices(
        trusted.map(t => ({
          device_id: t.device_id,
          device_name: t.device_name,
          ip: peers.find(p => p.device_id === t.device_id)?.ip ?? '',
          online: online.has(t.device_id),
        })),
      );
    };
    fetchDevices();
    const id = setInterval(fetchDevices, 10000);
    return () => clearInterval(id);
  }, []);

  // Responder side of pairing: an incoming request raises the pairing dialog.
  // Pairing is initiated from Settings; the overlay only responds.
  useEffect(() => {
    const unlisten = listen<IncomingPair>('pair-request', e => setIncomingPair(e.payload));
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // Surface a prompt when no directories are configured for indexing, so an
  // empty result list is not mistaken for a broken search.
  useEffect(() => {
    interface IndexStatus {
      indexed_dirs_count: number;
    }
    invoke<IndexStatus>('get_index_status')
      .then(s => setNoIndexedDirs(s.indexed_dirs_count === 0))
      .catch(() => undefined);
    const unlisten = listen('no-indexed-dirs', () => setNoIndexedDirs(true));
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // Reset to -1 (nothing highlighted) whenever the result set changes.
  useEffect(() => {
    setSelectedIndex(-1);
  }, [results]);

  // Check folder-search readiness when entering folder mode, and refresh it when
  // an index scan completes (so the rescan nudge clears itself).
  useEffect(() => {
    if (parsedInput.mode === 'folder') {
      invoke<boolean>('dirs_indexed').then(setFoldersIndexed).catch(() => setFoldersIndexed(null));
    }
  }, [parsedInput.mode]);

  useEffect(() => {
    const unlisten = listen<IndexProgress>('index-progress', e => {
      if (e.payload.phase.type === 'Complete') {
        setRescanning(false);
        invoke<boolean>('dirs_indexed').then(setFoldersIndexed).catch(() => undefined);
      }
    });
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const triggerFolderRescan = () => {
    setRescanning(true);
    invoke('trigger_reindex').catch(() => setRescanning(false));
  };

  // Open the @ device picker while the input begins with @ and no device is yet
  // selected. The @settings shortcut still routes to Settings, so it is excluded.
  useEffect(() => {
    if (selectedDevice) {
      setShowDevicePicker(false);
      return;
    }
    if (inputValue.startsWith('@') && inputValue.toLowerCase() !== '@settings') {
      setShowDevicePicker(true);
      setDevicePickerIndex(0);
    } else {
      setShowDevicePicker(false);
    }
  }, [inputValue, selectedDevice]);

  useEffect(() => {
    // The picker is open: defer searching until a device is chosen.
    if (showDevicePicker) {
      return;
    }

    if (selectedDevice) {
      const query = parsedInput.query;
      if (query.length === 0) {
        setResults([]);
        setError(null);
        return;
      }
      setError(null);
      const handle = setTimeout(() => {
        setRemoteSearchLoading(true);
        invoke<SearchResult[]>('search_remote', { deviceId: selectedDevice.device_id, query })
          .then(r => {
            setResults(r);
            setRemoteSearchLoading(false);
          })
          .catch(e => {
            setError(String(e));
            setResults([]);
            setRemoteSearchLoading(false);
          });
      }, 150);
      return () => clearTimeout(handle);
    }

    const { mode, query } = parsedInput;
    if (mode === 'settings') {
      nav('/settings');
      return;
    }
    if (mode === 'calc' || mode === 'remote' || query.length === 0) {
      setResults([]);
      setError(null);
      return;
    }
    setError(null);
    const handle = setTimeout(async () => {
      setLoading(true);
      try {
        if (mode === 'local') {
          setResults(await invoke<SearchResult[]>('search_local', { query }));
        } else if (mode === 'folder') {
          setResults(await invoke<SearchResult[]>('search_local', { query: '/' + query }));
        }
      } catch (e) {
        setError(String(e));
        setResults([]);
      } finally {
        setLoading(false);
      }
    }, 150);
    return () => clearTimeout(handle);
  }, [parsedInput, selectedDevice, showDevicePicker, nav]);

  const openSelected = (result: SearchResult) => {
    if (result.result_type === 'App') {
      invoke('launch_app', { exec: result.exec }).catch(() => {});
    } else {
      invoke('open_file_path', { path: result.path }).catch(() => {});
    }
    invoke('hide_window').catch(() => {});
  };

  // Keyboard navigation is driven from the search input (which retains focus
  // throughout) so the user can keep refining the query while browsing results.
  const handleArrowDown = () => {
    setSelectedIndex(i => stepDown(i, results.length));
  };

  const handleArrowUp = () => {
    setSelectedIndex(i => stepUp(i));
    searchInputRef.current?.focus();
  };

  const handleEnter = () => {
    const result = results[selectedIndex];
    if (selectedIndex < 0 || !result) return;
    // Remote directory results (folder search) have no transfer/launch action;
    // a directory cannot be downloaded as a file, so do nothing on Enter.
    if (selectedDevice && isFolderQuery && result.result_type === 'File') {
      return;
    }
    // Other remote results require an explicit confirmation: a download for
    // files, a launch for apps, rather than acting on a non-local path directly.
    if (selectedDevice && (result.result_type === 'File' || result.result_type === 'App')) {
      setConfirmingTransfer(result);
      return;
    }
    openSelected(result);
  };

  const confirmTransfer = () => {
    const result = confirmingTransfer;
    setConfirmingTransfer(null);
    searchInputRef.current?.focus();
    if (!result || !selectedDevice) return;
    if (result.result_type === 'App') {
      invoke('remote_launch_app', {
        deviceId: selectedDevice.device_id,
        appSourcePath: result.path,
      }).catch(e => setError(String(e)));
    } else {
      invoke('request_file_cmd', {
        deviceId: selectedDevice.device_id,
        remotePath: result.path,
      }).catch(e => setError(String(e)));
    }
  };

  const cancelTransfer = () => {
    setConfirmingTransfer(null);
    searchInputRef.current?.focus();
  };

  // Right-arrow file action expansion (local files only).
  const expandActions = () => {
    const result = results[selectedIndex];
    if (selectedIndex >= 0 && result && result.result_type === 'File' && !selectedDevice) {
      setExpandedResultPath(result.path);
      setExpandedActionIndex(0);
    }
  };

  const executeExpandedAction = (path: string, actionIndex: number) => {
    if (actionIndex === 0) {
      invoke('open_file_path', { path }).catch(() => {});
      invoke('hide_window').catch(() => {});
      setExpandedResultPath(null);
    } else if (actionIndex === 1) {
      invoke('reveal_in_files', { path }).catch(() => {});
      invoke('hide_window').catch(() => {});
      setExpandedResultPath(null);
    } else {
      setExpandedResultPath(null);
      setSendToDevicesPath(path);
    }
  };

  const closeSendToDevices = () => {
    setSendToDevicesPath(null);
    searchInputRef.current?.focus();
  };

  // Collapse the action expansion when the query or the highlighted row changes.
  useEffect(() => {
    setExpandedResultPath(null);
    setSendToDevicesPath(null);
  }, [inputValue]);

  useEffect(() => {
    setExpandedResultPath(null);
  }, [selectedIndex]);

  const selectDevice = (device: DeviceOption) => {
    setSelectedDevice(device);
    setShowDevicePicker(false);
    setInputValue('');
    setResults([]);
    searchInputRef.current?.focus();
  };

  const clearDevice = (reopenPicker: boolean) => {
    setSelectedDevice(null);
    setResults([]);
    setError(null);
    setInputValue(reopenPicker ? '@' : '');
    searchInputRef.current?.focus();
  };

  const handlePickerArrowDown = () => {
    setDevicePickerIndex(i => Math.min(i + 1, filteredDevices.length - 1));
  };

  const handlePickerArrowUp = () => {
    setDevicePickerIndex(i => Math.max(i - 1, 0));
  };

  const handlePickerSelect = () => {
    const device = filteredDevices[devicePickerIndex];
    if (device) selectDevice(device);
  };

  const handlePickerClose = () => {
    setShowDevicePicker(false);
    setInputValue('');
  };

  const handleRootKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      invoke('hide_window').catch(() => {});
    }
  };

  const hasQuery = parsedInput.query.length > 0;
  const currentResult = selectedIndex >= 0 ? results[selectedIndex] : undefined;
  const canExpand =
    !!currentResult &&
    currentResult.result_type === 'File' &&
    !selectedDevice &&
    expandedResultPath === null &&
    sendToDevicesPath === null;

  return (
    <div
      tabIndex={0}
      onKeyDown={handleRootKeyDown}
      className="flex flex-col w-screen h-screen overflow-hidden focus:outline-none relative"
      style={{
        backgroundColor: 'var(--bg)',
        color: 'var(--text)',
        borderRadius: '8px',
      }}
    >
      <div
        className="flex items-center justify-between px-3 shrink-0"
        style={{
          height: '40px',
          borderBottom: '1px solid var(--border)',
          backgroundColor: 'var(--bg)',
        }}
      >
        <div className="flex items-center gap-2">
          <img
            src="/assets/images/logo/png/SynaptV2_White_PNG.png"
            alt="Synapt"
            className="h-4 w-4"
          />
          <span className="text-xs font-medium">Synapt</span>
          {ipcStatus?.synaptclip_present && (
            <span className="text-xs" style={{ color: 'var(--muted)' }}>
              | SynaptClip <span style={{ fontSize: '10px' }}>connected</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => nav('/settings')}
            title="Settings"
            className="flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors"
            style={{ color: 'var(--muted)' }}
          >
            <SettingsIcon size={12} />
          </button>
        </div>
      </div>

      <div className="relative shrink-0">
        <SearchBar
          value={inputValue}
          onValueChange={setInputValue}
          inputRef={searchInputRef}
          selectedDevice={selectedDevice}
          onClearDevice={clearDevice}
          showDevicePicker={showDevicePicker}
          canExpand={canExpand}
          expanded={expandedResultPath !== null}
          onArrowDown={handleArrowDown}
          onArrowUp={handleArrowUp}
          onArrowRight={expandActions}
          onEnter={handleEnter}
          onEscape={() => invoke('hide_window').catch(() => {})}
          onPickerArrowDown={handlePickerArrowDown}
          onPickerArrowUp={handlePickerArrowUp}
          onPickerSelect={handlePickerSelect}
          onPickerClose={handlePickerClose}
          onExpandArrowDown={() => setExpandedActionIndex(i => (i + 1) % 3)}
          onExpandArrowUp={() => setExpandedActionIndex(i => (i + 2) % 3)}
          onExpandEnter={() => {
            if (expandedResultPath) executeExpandedAction(expandedResultPath, expandedActionIndex);
          }}
          onExpandCollapse={() => setExpandedResultPath(null)}
        />
        {showDevicePicker && (
          <DevicePicker
            devices={filteredDevices}
            selectedIndex={devicePickerIndex}
            onSelect={selectDevice}
            onClose={handlePickerClose}
          />
        )}
      </div>

      <SearchLoadingBar visible={remoteSearchLoading} />
      <TransferCard />

      <div className="flex-1 overflow-y-auto">
        {error && (
          <p className="text-xs px-3 py-2" style={{ color: 'var(--danger)' }}>
            {error}
          </p>
        )}
        {loading && (
          <p className="text-sm text-center py-6" style={{ color: 'var(--muted)' }}>
            Searching...
          </p>
        )}
        {!loading && !remoteSearchLoading && !error && results.length === 0 && hasQuery && parsedInput.mode !== 'calc' && (
          selectedDevice ? (
            <div className="flex flex-col items-center justify-center py-8 gap-1 px-6 text-center">
              <p className="text-sm" style={{ color: 'var(--muted)' }}>
                No shared results from {selectedDevice.device_name}
              </p>
              <p className="text-xs" style={{ color: 'var(--muted)' }}>
                Remote search only returns files inside that device&rsquo;s shared
                folders. Add a shared folder in Settings on {selectedDevice.device_name},
                or it may still be indexing.
              </p>
            </div>
          ) : parsedInput.mode === 'folder' ? (
            foldersIndexed === false ? (
              <div className="flex flex-col items-center justify-center py-8 gap-2 px-6 text-center">
                <p className="text-sm" style={{ color: 'var(--muted)' }}>
                  {rescanning
                    ? 'Reindexing - folders will appear when it finishes.'
                    : 'Folders are not indexed yet.'}
                </p>
                {!rescanning && (
                  <button
                    onClick={triggerFolderRescan}
                    className="text-xs px-3 py-1 rounded transition-colors"
                    style={{ backgroundColor: 'var(--accent)', color: '#fff' }}
                  >
                    Rescan to index folders
                  </button>
                )}
              </div>
            ) : (
              <div className="flex items-center justify-center py-8">
                <p className="text-sm" style={{ color: 'var(--muted)' }}>
                  No folders match &ldquo;{parsedInput.query}&rdquo;
                </p>
              </div>
            )
          ) : noIndexedDirs ? (
            <div className="flex-1 flex items-center justify-center py-8">
              <button
                onClick={() => nav('/settings')}
                className="text-sm transition-colors"
                style={{ color: 'var(--muted)' }}
              >
                Add directories to index in Settings to start searching
              </button>
            </div>
          ) : (
            <div className="flex items-center justify-center py-8">
              <p className="text-sm" style={{ color: 'var(--muted)' }}>
                No results for &ldquo;{parsedInput.query}&rdquo;
              </p>
            </div>
          )
        )}
        {!hasQuery &&
          !showDevicePicker &&
          !loading &&
          !remoteSearchLoading &&
          !error &&
          results.length === 0 && (
            <div className="flex flex-col items-center justify-center h-full px-6 text-center">
              <p className="text-sm font-medium mb-3" style={{ color: 'var(--text)' }}>
                Try searching
              </p>
              <div className="flex flex-col gap-1.5">
                <p className="text-xs" style={{ color: 'var(--muted)' }}>
                  Type to search files and apps
                </p>
                <p className="text-xs" style={{ color: 'var(--muted)' }}>
                  <span style={{ color: 'var(--accent)' }}>/</span> to find folders
                </p>
                <p className="text-xs" style={{ color: 'var(--muted)' }}>
                  <span style={{ color: 'var(--accent)' }}>@</span> to search a paired device
                </p>
                <p className="text-xs" style={{ color: 'var(--muted)' }}>
                  <span style={{ color: 'var(--accent)' }}>Right arrow</span> on a file for actions
                </p>
                <p className="text-xs" style={{ color: 'var(--muted)' }}>
                  Type a calculation like 45 * 12
                </p>
              </div>
            </div>
          )}
        {results.length > 0 && (
          <ResultList
            results={results}
            selectedIndex={selectedIndex}
            onSelect={openSelected}
            onHover={setSelectedIndex}
            confirmingPath={confirmingTransfer?.path ?? null}
            confirmMessage={
              confirmingTransfer && selectedDevice
                ? confirmingTransfer.result_type === 'App'
                  ? `Launch ${confirmingTransfer.name} on ${selectedDevice.device_name}?`
                  : `Download ${confirmingTransfer.name} from ${selectedDevice.device_name}?`
                : ''
            }
            confirmLabel={confirmingTransfer?.result_type === 'App' ? 'Launch' : 'Download'}
            onConfirmTransfer={confirmTransfer}
            onCancelTransfer={cancelTransfer}
            expandedPath={expandedResultPath}
            expandedActionIndex={expandedActionIndex}
            onActionHover={setExpandedActionIndex}
            onActionExecute={executeExpandedAction}
            sendToDevicesPath={sendToDevicesPath}
            devices={availableDevices}
            onCloseSendToDevices={closeSendToDevices}
            resultsAreFolders={isFolderQuery}
          />
        )}
      </div>

      {incomingPair && (
        <PairingDialog
          mode="responder"
          incomingPair={incomingPair}
          onClose={() => setIncomingPair(null)}
        />
      )}

      <IndexingBanner />
    </div>
  );
};
