import React, { useEffect, useRef, useState } from 'react';
import { Search } from 'lucide-react';
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
    <div
      className="flex items-center gap-2 px-3 shrink-0"
      style={{
        height: '48px',
        backgroundColor: 'var(--surface)',
        borderBottom: '1px solid var(--border)',
      }}
    >
      <Search size={16} style={{ color: 'var(--muted)' }} className="shrink-0" />
      {showBadge && (
        <span
          className="text-[10px] px-1.5 py-0.5 rounded shrink-0"
          style={{
            backgroundColor: 'var(--surface-hover)',
            color: 'var(--accent)',
            border: '1px solid var(--accent)',
          }}
        >
          {MODE_LABELS[parsed.mode]}
        </span>
      )}
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder="Search files, apps, @device, /folder, or calculate..."
        className="flex-1 bg-transparent text-sm outline-none"
        style={{ color: 'var(--text)' }}
      />
      {parsed.mode === 'calc' && calcResult !== null && (
        <span className="text-sm shrink-0" style={{ color: 'var(--accent)' }}>
          = {calcResult}
        </span>
      )}
    </div>
  );
};
