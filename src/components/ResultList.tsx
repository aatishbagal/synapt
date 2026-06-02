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
    <div className="flex flex-col gap-1">
      {results.map((result, index) => {
        const selected = index === selectedIndex;
        const isRemote = result.source !== 'Local';
        return (
          <div
            key={`${result.path}-${index}`}
            onClick={() => onSelect(result)}
            onMouseEnter={() => onHover(index)}
            className={`rounded-card px-4 py-3 flex justify-between items-center cursor-pointer ${
              selected
                ? 'bg-accent/10 border-l-2 border-accent'
                : 'bg-surface border border-border hover:bg-border/50'
            }`}
          >
            <div className="min-w-0">
              <p className="text-text-primary text-sm font-medium truncate">{result.name}</p>
              <p className="text-text-muted text-xs truncate max-w-xs">{result.path}</p>
            </div>
            <span className={`text-xs ml-3 shrink-0 ${isRemote ? 'text-accent' : 'text-text-muted'}`}>
              {sourceLabel(result.source)}
            </span>
          </div>
        );
      })}
    </div>
  );
};
