import React, { useEffect, useMemo, useState } from 'react';
import { Search, X } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { InputMode } from '../types';
import { parseInput } from '../utils/parseInput';

interface Props {
  value: string;
  onValueChange: (next: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  selectedDevice: { device_id: string; device_name: string } | null;
  onClearDevice: (reopenPicker: boolean) => void;
  showDevicePicker: boolean;
  canExpand: boolean;
  expanded: boolean;
  onArrowDown: () => void;
  onArrowUp: () => void;
  onArrowRight: () => void;
  onEnter: () => void;
  onEscape: () => void;
  onPickerArrowDown: () => void;
  onPickerArrowUp: () => void;
  onPickerSelect: () => void;
  onPickerClose: () => void;
  onExpandArrowDown: () => void;
  onExpandArrowUp: () => void;
  onExpandEnter: () => void;
  onExpandCollapse: () => void;
}

const MODE_LABELS: Record<InputMode, string> = {
  local: 'Local',
  folder: 'Folder',
  remote: 'Remote',
  settings: 'Settings',
  calc: 'Calc',
};

export const SearchBar: React.FC<Props> = ({
  value,
  onValueChange,
  inputRef,
  selectedDevice,
  onClearDevice,
  showDevicePicker,
  canExpand,
  expanded,
  onArrowDown,
  onArrowUp,
  onArrowRight,
  onEnter,
  onEscape,
  onPickerArrowDown,
  onPickerArrowUp,
  onPickerSelect,
  onPickerClose,
  onExpandArrowDown,
  onExpandArrowUp,
  onExpandEnter,
  onExpandCollapse,
}) => {
  const [calcResult, setCalcResult] = useState<number | null>(null);
  const parsed = useMemo(() => parseInput(value, selectedDevice !== null), [value, selectedDevice]);

  useEffect(() => {
    inputRef.current?.focus();
  }, [inputRef]);

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

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (showDevicePicker) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          onPickerArrowDown();
          break;
        case 'ArrowUp':
          e.preventDefault();
          onPickerArrowUp();
          break;
        case 'Enter':
          e.preventDefault();
          onPickerSelect();
          break;
        case 'Escape':
          e.preventDefault();
          onPickerClose();
          break;
      }
      return;
    }

    // The file-action expansion captures navigation while it is open.
    if (expanded) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          onExpandArrowDown();
          break;
        case 'ArrowUp':
          e.preventDefault();
          onExpandArrowUp();
          break;
        case 'Enter':
          e.preventDefault();
          onExpandEnter();
          break;
        case 'ArrowLeft':
        case 'Escape':
          e.preventDefault();
          onExpandCollapse();
          break;
      }
      return;
    }

    // Backspace on an empty input with a device tag removes the tag and reopens
    // the picker so another device can be chosen.
    if (e.key === 'Backspace' && value === '' && selectedDevice) {
      e.preventDefault();
      onClearDevice(true);
      return;
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        onArrowDown();
        break;
      case 'ArrowUp':
        e.preventDefault();
        onArrowUp();
        break;
      case 'ArrowRight':
        // Only hijack ArrowRight to expand file actions; otherwise let it move
        // the text cursor.
        if (canExpand) {
          e.preventDefault();
          onArrowRight();
        }
        break;
      case 'Enter':
        e.preventDefault();
        onEnter();
        break;
      case 'Escape':
        e.preventDefault();
        onEscape();
        break;
    }
  };

  const showBadge = !selectedDevice && !showDevicePicker && parsed.mode !== 'local';

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
      {selectedDevice && (
        <span
          className="flex items-center gap-1 shrink-0"
          style={{
            backgroundColor: 'color-mix(in srgb, var(--accent) 15%, transparent)',
            border: '1px solid color-mix(in srgb, var(--accent) 40%, transparent)',
            borderRadius: '4px',
            padding: '3px 8px',
            fontSize: '12px',
            color: 'var(--accent)',
          }}
        >
          {selectedDevice.device_name}
          <button
            type="button"
            aria-label="Clear device"
            onMouseDown={e => {
              e.preventDefault();
              onClearDevice(false);
            }}
            className="flex items-center justify-center"
            style={{
              width: '14px',
              height: '14px',
              border: 'none',
              background: 'transparent',
              color: 'color-mix(in srgb, var(--accent) 60%, transparent)',
              cursor: 'pointer',
            }}
          >
            <X size={12} />
          </button>
        </span>
      )}
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
        onChange={e => onValueChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={
          selectedDevice
            ? `Search on ${selectedDevice.device_name}...`
            : 'Search files, apps, @device, /folder, or calculate...'
        }
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
