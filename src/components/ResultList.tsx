import React from 'react';
import { AppIcon } from './AppIcon';
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

/** Generic 16x16 document glyph with a folded top-right corner, for file rows. */
function FileIcon(): React.ReactElement {
  return (
    <svg width={16} height={16} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <path
        d="M4 2H9L12 5V14H4Z"
        stroke="var(--muted)"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
      <path d="M9 2V5H12" stroke="var(--muted)" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
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
            <div className="flex items-center flex-1 min-w-0" style={{ gap: '10px' }}>
              {isApp ? (
                <AppIcon iconPath={result.icon_path} size={20} />
              ) : (
                <span
                  className="flex items-center justify-center shrink-0"
                  style={{ width: 20, height: 20 }}
                >
                  <FileIcon />
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
