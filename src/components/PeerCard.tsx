import React from 'react';
import { Peer } from '../types';

interface Props {
  peer: Peer;
  onPair: (peer: Peer) => void;
  onSendFile: (peer: Peer) => void;
}

export const PeerCard: React.FC<Props> = ({ peer, onPair, onSendFile }) => {
  const trusted = peer.status === 'Trusted';
  return (
    <div className="flex items-center justify-between px-4 py-3 bg-surface rounded-card border border-border">
      <div>
        <p className="text-text-primary text-sm font-medium">{peer.device_name}</p>
        <p className="text-text-muted text-xs">{peer.ip}</p>
      </div>
      <div className="flex items-center gap-2">
        <span className={`text-xs px-2 py-0.5 rounded ${
          trusted ? 'text-accent bg-accent/10' : 'text-text-muted bg-border'
        }`}>{trusted ? 'Trusted' : 'Discovered'}</span>
        {trusted
          ? <button onClick={() => onSendFile(peer)} className="text-xs px-3 py-1 bg-accent text-bg rounded-btn hover:opacity-80 transition-opacity">Send file</button>
          : <button onClick={() => onPair(peer)}     className="text-xs px-3 py-1 border border-border text-text-primary rounded-btn hover:bg-border transition-colors">Pair</button>
        }
      </div>
    </div>
  );
};
