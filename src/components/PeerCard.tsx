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
      className={`flex items-center justify-between gap-2 px-3 py-2.5 transition-colors ${
        trusted ? 'cursor-pointer' : ''
      }`}
      style={{
        borderBottom: `1px solid ${selected ? 'var(--accent)' : 'var(--border)'}`,
        backgroundColor: selected ? 'var(--surface-hover)' : 'transparent',
      }}
      onMouseEnter={e => {
        if (!selected) (e.currentTarget as HTMLDivElement).style.backgroundColor = 'var(--surface-hover)';
      }}
      onMouseLeave={e => {
        if (!selected) (e.currentTarget as HTMLDivElement).style.backgroundColor = 'transparent';
      }}
    >
      <div className="flex-1 min-w-0 flex flex-col gap-0.5">
        <span className="truncate text-[13px]" style={{ color: 'var(--text)' }}>
          {peer.device_name}
        </span>
        <span className="truncate text-xs" style={{ color: 'var(--muted)' }}>
          {peer.ip}
        </span>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <span
          className="text-[10px] px-1.5 py-0.5 rounded"
          style={
            trusted
              ? {
                  backgroundColor: 'var(--surface-hover)',
                  color: 'var(--accent)',
                  border: '1px solid var(--accent)',
                }
              : {
                  backgroundColor: 'var(--surface-hover)',
                  color: 'var(--muted)',
                  border: '1px solid var(--border)',
                }
          }
        >
          {trusted ? 'Trusted' : 'Discovered'}
        </span>
        {trusted ? (
          <button
            onClick={e => {
              e.stopPropagation();
              onSendFile(peer);
            }}
            className="rounded px-2 py-1 text-xs transition-colors"
            style={{
              backgroundColor: 'var(--surface-hover)',
              color: 'var(--accent)',
              border: '1px solid var(--accent)',
            }}
          >
            Send file
          </button>
        ) : (
          <button
            onClick={e => {
              e.stopPropagation();
              onPair(peer);
            }}
            className="rounded px-2 py-1 text-xs transition-colors"
            style={{
              backgroundColor: 'var(--surface)',
              color: 'var(--text)',
              border: '1px solid var(--border)',
            }}
          >
            Pair
          </button>
        )}
      </div>
    </div>
  );
};
