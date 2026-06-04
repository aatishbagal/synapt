import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DeviceOption } from '../types';

interface Props {
  devices: DeviceOption[];
  filePath: string;
  onClose: () => void;
}

/** Panel for choosing one or more trusted devices to send a file to. */
export const DeviceMultiSelect: React.FC<Props> = ({ devices, filePath, onClose }) => {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [focusIndex, setFocusIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    containerRef.current?.focus();
  }, []);

  const toggle = (device: DeviceOption) => {
    if (!device.online) return;
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(device.device_id)) next.delete(device.device_id);
      else next.add(device.device_id);
      return next;
    });
  };

  const send = () => {
    if (selected.size === 0) return;
    for (const deviceId of selected) {
      invoke('send_files_cmd', { deviceId, localPaths: [filePath] }).catch(() => {});
    }
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setFocusIndex(i => Math.min(i + 1, Math.max(devices.length - 1, 0)));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setFocusIndex(i => Math.max(i - 1, 0));
        break;
      case ' ':
        e.preventDefault();
        if (devices[focusIndex]) toggle(devices[focusIndex]);
        break;
      case 'Enter':
        e.preventDefault();
        send();
        break;
      case 'Escape':
        e.preventDefault();
        onClose();
        break;
    }
  };

  return (
    <div
      ref={containerRef}
      tabIndex={-1}
      onKeyDown={handleKeyDown}
      onClick={e => e.stopPropagation()}
      style={{
        backgroundColor: 'var(--surface)',
        border: '1px solid color-mix(in srgb, var(--accent) 30%, transparent)',
        borderTop: '1px solid var(--border)',
        borderRadius: '0 0 8px 8px',
        padding: '8px 0',
        outline: 'none',
      }}
    >
      <div
        style={{
          padding: '4px 16px 8px',
          fontSize: '12px',
          textTransform: 'uppercase',
          letterSpacing: '0.06em',
          color: 'var(--muted)',
        }}
      >
        Send to devices
      </div>

      {devices.length === 0 ? (
        <p style={{ padding: '8px 16px', fontSize: '12px', color: 'var(--muted)' }}>
          No paired devices available. Pair a device in Settings.
        </p>
      ) : (
        devices.map((device, index) => {
          const isSelected = selected.has(device.device_id);
          const focused = index === focusIndex;
          return (
            <div
              key={device.device_id}
              onMouseEnter={() => setFocusIndex(index)}
              onClick={() => toggle(device)}
              className="flex items-center gap-2"
              style={{
                padding: '8px 16px',
                cursor: device.online ? 'pointer' : 'default',
                backgroundColor: focused
                  ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
                  : 'transparent',
              }}
            >
              <span
                aria-hidden
                style={{
                  width: '12px',
                  height: '12px',
                  flexShrink: 0,
                  borderRadius: '2px',
                  backgroundColor: isSelected ? 'var(--accent)' : 'transparent',
                  border: `1px solid ${isSelected ? 'var(--accent)' : 'var(--border)'}`,
                }}
              />
              <span
                className="flex-1 truncate"
                style={{
                  fontSize: '13px',
                  color: device.online ? 'var(--text)' : 'var(--muted)',
                }}
              >
                {device.device_name}
              </span>
              <span
                aria-label={device.online ? 'Online' : 'Offline'}
                style={{
                  width: '7px',
                  height: '7px',
                  flexShrink: 0,
                  borderRadius: '50%',
                  backgroundColor: device.online ? 'var(--accent)' : 'var(--border)',
                }}
              />
            </div>
          );
        })
      )}

      <div className="flex items-center gap-2" style={{ padding: '8px 16px 4px' }}>
        <button
          type="button"
          onClick={send}
          disabled={selected.size === 0}
          style={{
            borderRadius: '4px',
            padding: '4px 12px',
            fontSize: '12px',
            border: 'none',
            cursor: selected.size === 0 ? 'default' : 'pointer',
            backgroundColor:
              selected.size === 0
                ? 'color-mix(in srgb, var(--accent) 30%, transparent)'
                : 'var(--accent)',
            color: selected.size === 0 ? 'var(--muted)' : '#fff',
          }}
        >
          Send
        </button>
        <button
          type="button"
          onClick={onClose}
          style={{
            borderRadius: '4px',
            padding: '4px 12px',
            fontSize: '12px',
            background: 'transparent',
            color: 'var(--muted)',
            border: '1px solid var(--border)',
            cursor: 'pointer',
          }}
        >
          Cancel
        </button>
      </div>
    </div>
  );
};
