import React from 'react';
import { AppWindow } from 'lucide-react';
import { SearchResult } from '../types';

interface Props {
  results: SearchResult[];
  selectedIndex: number;
  onSelect: (result: SearchResult) => void;
  onHover: (index: number) => void;
}

function isRemote(source: SearchResult['source']): source is { Remote: { device_name: string } } {
  return typeof source === 'object' && 'Remote' in source;
}

function sourceLabel(source: SearchResult['source']): string {
  return isRemote(source) ? source.Remote.device_name : 'Local';
}

export const ResultList: React.FC<Props> = ({ results, selectedIndex, onSelect, onHover }) => {
  return (
    <div>
      {results.map((result, index) => {
        const selected = index === selectedIndex;
        const isApp = result.result_type === 'App';
        const remote = isRemote(result.source);
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
            <div className="flex items-center gap-2.5 flex-1 min-w-0">
              {isApp && (
                <span
                  className="flex items-center justify-center shrink-0 rounded"
                  style={{
                    width: '24px',
                    height: '24px',
                    backgroundColor: 'var(--surface-hover)',
                    color: 'var(--accent)',
                    border: '1px solid var(--accent)',
                  }}
                >
                  <AppWindow size={13} />
                </span>
              )}
              <div className="flex-1 min-w-0 flex flex-col gap-0.5">
                <span
                  className="truncate text-[13px]"
                  style={{ color: 'var(--text)', fontWeight: isApp ? 500 : 400 }}
                >
                  {result.name}
                </span>
                <span className="truncate text-xs" style={{ color: 'var(--muted)' }}>
                  {isApp ? 'Application' : result.path}
                </span>
              </div>
            </div>
            {!isApp && (
              <span
                className="text-[10px] px-1.5 py-0.5 rounded shrink-0"
                style={
                  remote
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
            )}
          </div>
        );
      })}
    </div>
  );
};
