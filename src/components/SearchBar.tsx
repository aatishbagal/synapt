import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { InputMode, ParsedInput } from '../types';
import { parseInput } from '../utils/parseInput';

interface Props {
  onInput: (parsed: ParsedInput) => void;
  onArrowDown: () => void;
  onEscape: () => void;
}

const MODE_LABELS: Record<InputMode, string> = {
  local: 'Local',
  folder: 'Folder',
  remote: 'Remote',
  settings: 'Settings',
  calc: 'Calc',
};

export const SearchBar: React.FC<Props> = ({ onInput, onArrowDown, onEscape }) => {
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState('');
  const [parsed, setParsed] = useState<ParsedInput>(parseInput(''));
  const [calcResult, setCalcResult] = useState<number | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (parsed.mode !== 'calc' || parsed.query.length === 0) {
      setCalcResult(null);
      return;
    }
    const handle = setTimeout(() => {
      invoke<number>('evaluate_expr', { input: parsed.query })
        .then(result => setCalcResult(result))
        .catch(() => setCalcResult(null));
    }, 300);
    return () => clearTimeout(handle);
  }, [parsed.mode, parsed.query]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const next = e.target.value;
    setValue(next);
    const p = parseInput(next);
    setParsed(p);
    onInput(p);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      onArrowDown();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onEscape();
    }
  };

  const showBadge = parsed.mode !== 'local';

  return (
    <div className="relative">
      {showBadge && (
        <span className="absolute left-4 top-1/2 -translate-y-1/2 text-xs px-2 py-0.5 rounded bg-accent/10 text-accent">
          {MODE_LABELS[parsed.mode]}
        </span>
      )}
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder="Search files, @device, /folder, or calculate..."
        className={`w-full bg-surface border border-border rounded-card py-3 text-text-primary text-sm focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-muted ${
          showBadge ? 'pl-20 pr-4' : 'px-4'
        }`}
      />
      {parsed.mode === 'calc' && calcResult !== null && (
        <p className="text-accent text-sm mt-1 px-4">= {calcResult}</p>
      )}
    </div>
  );
};
