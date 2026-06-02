import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { PeerCard } from '../components/PeerCard';
import { PairingDialog } from '../components/PairingDialog';
import { SearchBar } from '../components/SearchBar';
import { ResultList } from '../components/ResultList';
import { Peer, ParsedInput, SearchResult, TrustedPeer } from '../types';

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

  const nav = useNavigate();

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
    invoke('open_file_path', { path: result.path }).catch(() => {});
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
      className={`w-full h-screen bg-bg flex flex-col focus:outline-none ${
        dragOver ? 'ring-2 ring-accent' : ''
      }`}
    >
      <div className="flex items-center justify-between px-4 pt-3 pb-2">
        <span className="text-text-primary text-sm font-medium">Synapt</span>
        <button
          onClick={() => nav('/settings')}
          className="text-text-muted hover:text-text-primary transition-colors text-xs"
        >
          Settings
        </button>
      </div>

      <div className="px-4 pb-2">
        <SearchBar
          onInput={setParsedInput}
          onArrowDown={() => results.length > 0 && setSelectedIndex(0)}
          onEscape={() => invoke('hide_window').catch(() => {})}
        />
      </div>

      <div className="flex-1 overflow-y-auto px-4 max-h-[18rem]">
        {error && <p className="text-red-400 text-xs px-4 py-2">{error}</p>}
        {loading && <p className="text-text-muted text-xs text-center py-4">Searching...</p>}
        {!loading && !error && results.length === 0 && hasQuery && parsedInput.mode !== 'calc' && (
          <p className="text-text-muted text-xs text-center py-6">
            No results for "{parsedInput.query}"
          </p>
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

      <div className="border-t border-border px-4 py-3 flex flex-col gap-2 max-h-[14rem] overflow-y-auto">
        <span className="text-text-muted text-xs">Devices</span>
        {peers.length === 0 ? (
          <p className="text-text-muted text-xs text-center py-2">
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
        {!selectedPeerForSend ? (
          <p className="text-text-muted text-xs">
            Select a trusted device to send files via drag-and-drop
          </p>
        ) : (
          <p className="text-accent text-xs">
            Drop files to send to {selectedPeerForSend.device_name}
          </p>
        )}
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
    </div>
  );
};
