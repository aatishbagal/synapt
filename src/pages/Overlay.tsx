import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { Settings as SettingsIcon } from 'lucide-react';
import { PeerCard } from '../components/PeerCard';
import { PairingDialog } from '../components/PairingDialog';
import { SearchBar } from '../components/SearchBar';
import { ResultList } from '../components/ResultList';
import { IndexingBanner } from '../components/IndexingBanner';
import { Peer, ParsedInput, SearchResult, TrustedPeer } from '../types';
import { useTheme } from '../hooks/useTheme';

const THEME_ORDER = ['dark', 'light', 'system'] as const;

/** Inline 16x16 icon for the active theme: sun, moon, or monitor. */
function ThemeIcon({ theme }: { theme: string }) {
  const common = {
    width: 16,
    height: 16,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 2,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
  };
  if (theme === 'light') {
    return (
      <svg {...common}>
        <circle cx="12" cy="12" r="4" />
        <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
      </svg>
    );
  }
  if (theme === 'system') {
    return (
      <svg {...common}>
        <rect x="2" y="3" width="20" height="14" rx="2" />
        <path d="M8 21h8M12 17v4" />
      </svg>
    );
  }
  return (
    <svg {...common}>
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  );
}

interface IncomingPair {
  device_id: string;
  device_name: string;
  verify_code: string;
}

const EMPTY_INPUT: ParsedInput = { raw: '', mode: 'local', query: '', deviceName: null };

export const Overlay: React.FC = () => {
  const [peers, setPeers] = useState<Peer[]>([]);
  const [pairingPeer, setPairingPeer] = useState<Peer | null>(null);
  const [incomingPair, setIncomingPair] = useState<IncomingPair | null>(null);

  const [parsedInput, setParsedInput] = useState<ParsedInput>(EMPTY_INPUT);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [selectedPeerForSend, setSelectedPeerForSend] = useState<Peer | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [noIndexedDirs, setNoIndexedDirs] = useState(false);

  const [theme, setTheme] = useState<string>(() => localStorage.getItem('synapt-theme') ?? 'dark');
  const [themeHover, setThemeHover] = useState(false);
  const { apply: applyTheme } = useTheme();

  const nav = useNavigate();

  useEffect(() => {
    invoke<string | null>('get_setting', { key: 'theme' })
      .then(t => setTheme(t ?? 'dark'))
      .catch(() => undefined);
  }, []);

  const cycleTheme = async () => {
    const next = THEME_ORDER[(THEME_ORDER.indexOf(theme as (typeof THEME_ORDER)[number]) + 1) % THEME_ORDER.length];
    setTheme(next);
    applyTheme(next);
    await invoke('set_setting', { key: 'theme', value: next }).catch(() => undefined);
  };

  useEffect(() => {
    const poll = async () => {
      const result = await invoke<Peer[]>('get_peers').catch(() => []);
      setPeers(result);
    };
    poll();
    const id = setInterval(poll, 2000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    const unlisten = listen<IncomingPair>('pair-request', e => setIncomingPair(e.payload));
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // Surface a prompt when no directories are configured for indexing, so an
  // empty result list is not mistaken for a broken search.
  useEffect(() => {
    interface IndexStatus { indexed_dirs_count: number }
    invoke<IndexStatus>('get_index_status')
      .then(s => setNoIndexedDirs(s.indexed_dirs_count === 0))
      .catch(() => undefined);
    const unlisten = listen('no-indexed-dirs', () => setNoIndexedDirs(true));
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  useEffect(() => {
    setSelectedIndex(0);
  }, [results]);

  useEffect(() => {
    const { mode, query, deviceName } = parsedInput;

    if (mode === 'settings') {
      nav('/settings');
      return;
    }

    if (mode === 'calc' || query.length === 0) {
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
        } else if (mode === 'remote') {
          const trusted = await invoke<TrustedPeer[]>('get_trusted_peers').catch(() => []);
          const match = trusted.find(
            p => p.device_name.toLowerCase() === (deviceName ?? '').toLowerCase(),
          );
          if (!match) {
            setResults([]);
            setError('Device not paired — check Settings');
          } else {
            setResults(
              await invoke<SearchResult[]>('search_remote', { deviceId: match.device_id, query }),
            );
          }
        }
      } catch (e) {
        setError(String(e));
        setResults([]);
      } finally {
        setLoading(false);
      }
    }, 150);

    return () => clearTimeout(handle);
  }, [parsedInput, nav]);

  const openSelected = (result: SearchResult) => {
    if (result.result_type === 'App') {
      invoke('launch_app', { exec: result.exec }).catch(() => {});
    } else {
      invoke('open_file_path', { path: result.path }).catch(() => {});
    }
    invoke('hide_window').catch(() => {});
  };

  const sendPaths = (paths: string[]) => {
    if (!selectedPeerForSend) {
      setError('Select a trusted device first');
      return;
    }
    if (paths.length === 0) return;
    setError(null);
    invoke('send_files_cmd', {
      deviceId: selectedPeerForSend.device_id,
      localPaths: paths,
    }).catch(err => setError(String(err)));
  };

  // Tauri v2 delivers dropped-file paths through the webview drag-drop event
  // (the native handler intercepts before the HTML layer sees them).
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(event => {
      const p = event.payload;
      if (p.type === 'enter' || p.type === 'over') {
        setDragOver(true);
      } else if (p.type === 'leave') {
        setDragOver(false);
      } else if (p.type === 'drop') {
        setDragOver(false);
        sendPaths(p.paths);
      }
    });
    return () => {
      unlisten.then(fn => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedPeerForSend]);

  const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragOver(false);
    // Fallback for environments where HTML drop exposes file paths directly.
    const paths: string[] = [];
    for (let i = 0; i < e.dataTransfer.files.length; i++) {
      const file = e.dataTransfer.files[i] as unknown as { path?: string };
      if (file.path) paths.push(file.path);
    }
    if (paths.length > 0) sendPaths(paths);
  };

  const handleRootKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(i => Math.min(i + 1, results.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(i => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const result = results[selectedIndex];
      if (result) openSelected(result);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      if (selectedPeerForSend) {
        setSelectedPeerForSend(null);
      } else {
        invoke('hide_window').catch(() => {});
      }
    }
  };

  const hasQuery = parsedInput.query.length > 0;

  return (
    <div
      tabIndex={0}
      onKeyDown={handleRootKeyDown}
      onDragOver={e => {
        e.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={handleDrop}
      className="flex flex-col w-screen h-screen overflow-hidden focus:outline-none relative"
      style={{
        backgroundColor: 'var(--bg)',
        color: 'var(--text)',
        borderRadius: '8px',
        boxShadow: dragOver ? 'inset 0 0 0 2px var(--accent)' : 'none',
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
          <button
            type="button"
            onClick={cycleTheme}
            title="Toggle theme"
            onMouseEnter={() => setThemeHover(true)}
            onMouseLeave={() => setThemeHover(false)}
            className="flex items-center px-2 py-1 rounded transition-colors"
            style={{ color: themeHover ? 'var(--text)' : 'var(--muted)' }}
          >
            <ThemeIcon theme={theme} />
          </button>
        </div>
      </div>

      <SearchBar
        onInput={setParsedInput}
        onArrowDown={() => results.length > 0 && setSelectedIndex(0)}
        onEscape={() => invoke('hide_window').catch(() => {})}
      />

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
        {!loading && !error && results.length === 0 && hasQuery && parsedInput.mode !== 'calc' && (
          noIndexedDirs ? (
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

      <div
        className="shrink-0 overflow-y-auto"
        style={{
          borderTop: '1px solid var(--border)',
          backgroundColor: 'var(--bg)',
          maxHeight: '14rem',
        }}
      >
        <div className="px-3 pt-2 pb-1">
          <h2 className="text-[11px] uppercase tracking-wider" style={{ color: 'var(--muted)' }}>
            Devices
          </h2>
        </div>
        {peers.length === 0 ? (
          <p className="text-sm text-center py-4" style={{ color: 'var(--muted)' }}>
            No devices found on local network
          </p>
        ) : (
          peers.map(p => (
            <PeerCard
              key={p.device_id}
              peer={p}
              onPair={setPairingPeer}
              onSendFile={setSelectedPeerForSend}
              selected={selectedPeerForSend?.device_id === p.device_id}
              onSelect={setSelectedPeerForSend}
            />
          ))
        )}
        <div className="px-3 py-2">
          {!selectedPeerForSend ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              Select a trusted device to send files via drag-and-drop
            </p>
          ) : (
            <p className="text-xs" style={{ color: 'var(--accent)' }}>
              Drop files to send to {selectedPeerForSend.device_name}
            </p>
          )}
        </div>
      </div>

      {pairingPeer && (
        <PairingDialog mode="initiator" peer={pairingPeer} onClose={() => setPairingPeer(null)} />
      )}

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
