import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PeerCard } from '../components/PeerCard';
import { PairingDialog } from '../components/PairingDialog';
import { Peer } from '../types';

interface IncomingPair {
  device_id: string;
  device_name: string;
  verify_code: string;
}

export const Overlay: React.FC = () => {
  const [peers, setPeers] = useState<Peer[]>([]);
  const [pairingPeer, setPairingPeer] = useState<Peer | null>(null);
  const [incomingPair, setIncomingPair] = useState<IncomingPair | null>(null);
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

  return (
    <div className="w-full h-screen bg-bg flex flex-col">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <span className="text-text-primary text-sm font-medium">Synapt</span>
        <button onClick={() => nav('/settings')} className="text-text-muted hover:text-text-primary transition-colors text-xs">Settings</button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-2">
        {peers.length === 0
          ? <p className="text-text-muted text-xs text-center mt-8">No devices found on local network</p>
          : peers.map(p => (
              <PeerCard
                key={p.device_id}
                peer={p}
                onPair={setPairingPeer}
                onSendFile={() => { /* file picker - v0.2 */ }}
              />
            ))
        }
      </div>

      {pairingPeer && (
        <PairingDialog
          mode="initiator"
          peer={pairingPeer}
          onClose={() => setPairingPeer(null)}
        />
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
