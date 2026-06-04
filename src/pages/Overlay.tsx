import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Settings as SettingsIcon } from 'lucide-react';
import { PairingDialog } from '../components/PairingDialog';
import { SearchBar } from '../components/SearchBar';
import { ResultList } from '../components/ResultList';
import { DevicePicker } from '../components/DevicePicker';
import { IndexingBanner } from '../components/IndexingBanner';
import { Peer, SearchResult, TrustedPeer, DeviceOption } from '../types';
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

  const [availableDevices, setAvailableDevices] = useState<DeviceOption[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<DeviceOption | null>(null);
  const [showDevicePicker, setShowDevicePicker] = useState(false);
  const [devicePickerIndex, setDevicePickerIndex] = useState(0);

  const searchInputRef = useRef<HTMLInputElement>(null);

  const { apply: applyTheme } = useTheme();
  const nav = useNavigate();

  const parsedInput = useMemo(
    () => parseInput(inputValue, selectedDevice !== null),
    [inputValue, selectedDevice],
  );

  const deviceFilter = inputValue.startsWith('@') ? inputValue.slice(1).toLowerCase() : '';
  const filteredDevices = useMemo(
    () => availableDevices.filter(d => d.device_name.toLowerCase().startsWith(deviceFilter)),
    [availableDevices, deviceFilter],
  );

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
    if (selectedIndex >= 0 && result) openSelected(result);
  };

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
          onArrowDown={handleArrowDown}
          onArrowUp={handleArrowUp}
          onEnter={handleEnter}
          onEscape={() => invoke('hide_window').catch(() => {})}
          onPickerArrowDown={handlePickerArrowDown}
          onPickerArrowUp={handlePickerArrowUp}
          onPickerSelect={handlePickerSelect}
          onPickerClose={handlePickerClose}
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

      <div className="flex-1 overflow-y-auto">
        {error && (
          <p className="text-xs px-3 py-2" style={{ color: 'var(--danger)' }}>
            {error}
          </p>
        )}
        {(loading || remoteSearchLoading) && (
          <p className="text-sm text-center py-6" style={{ color: 'var(--muted)' }}>
            Searching...
          </p>
        )}
        {!loading && !remoteSearchLoading && !error && results.length === 0 && hasQuery && parsedInput.mode !== 'calc' && (
          noIndexedDirs && !selectedDevice ? (
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
        {results.length > 0 && (
          <ResultList
            results={results}
            selectedIndex={selectedIndex}
            onSelect={openSelected}
            onHover={setSelectedIndex}
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
