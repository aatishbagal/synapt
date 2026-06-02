import React from 'react';
import { Peer } from '../types';

interface Props {
  peer: Peer;
  onPair: (peer: Peer) => void;
  onSendFile: (peer: Peer) => void;
  selected?: boolean;
  onSelect?: (peer: Peer) => void;
}

export const PeerCard: React.FC<Props> = ({ peer, onPair, onSendFile, selected, onSelect }) => {
  const trusted = peer.status === 'Trusted';
  return (
    <div
      onClick={trusted && onSelect ? () => onSelect(peer) : undefined}
      className={`flex items-center justify-between px-4 py-3 bg-surface rounded-card border ${
        selected ? 'border-accent ring-1 ring-accent' : 'border-border'
      } ${trusted ? 'cursor-pointer' : ''}`}
    >
      <div>
        <p className="text-text-primary text-sm font-medium">{peer.device_name}</p>
        <p className="text-text-muted text-xs">{peer.ip}</p>
      </div>
      <div className="flex items-center gap-2">
        <span className={`text-xs px-2 py-0.5 rounded ${
          trusted ? 'text-accent bg-accent/10' : 'text-text-muted bg-border'
        }`}>{trusted ? 'Trusted' : 'Discovered'}</span>
        {trusted
          ? <button onClick={(e) => { e.stopPropagation(); onSendFile(peer); }} className="text-xs px-3 py-1 bg-accent text-bg rounded-btn hover:opacity-80 transition-opacity">Send file</button>
          : <button onClick={(e) => { e.stopPropagation(); onPair(peer); }}     className="text-xs px-3 py-1 border border-border text-text-primary rounded-btn hover:bg-border transition-colors">Pair</button>
        }
      </div>
    </div>
  );
};
