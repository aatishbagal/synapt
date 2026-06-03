import React from 'react';
import { SearchResult } from '../types';

interface Props {
  results: SearchResult[];
  selectedIndex: number;
  onSelect: (result: SearchResult) => void;
  onHover: (index: number) => void;
}

function sourceLabel(source: SearchResult['source']): string {
  if (source === 'Local') return 'Local';
  return source.Remote.device_name;
}

export const ResultList: React.FC<Props> = ({ results, selectedIndex, onSelect, onHover }) => {
  return (
    <div>
      {results.map((result, index) => {
        const selected = index === selectedIndex;
        const isRemote = result.source !== 'Local';
        return (
          <div
            key={`${result.path}-${index}`}
            onClick={() => onSelect(result)}
            onMouseEnter={() => onHover(index)}
            className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer transition-colors"
            style={{
              borderBottom: '1px solid var(--border)',
              backgroundColor: selected ? 'var(--surface-hover)' : 'transparent',
            }}
          >
            <div className="flex-1 min-w-0 flex flex-col gap-0.5">
              <span className="truncate text-[13px]" style={{ color: 'var(--text)' }}>
                {result.name}
              </span>
              <span className="truncate text-xs" style={{ color: 'var(--muted)' }}>
                {result.path}
              </span>
            </div>
            <span
              className="text-[10px] px-1.5 py-0.5 rounded shrink-0"
              style={
                isRemote
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
              {sourceLabel(result.source)}
            </span>
          </div>
        );
      })}
    </div>
  );
};
