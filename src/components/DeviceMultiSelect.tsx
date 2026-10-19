import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DeviceOption } from '../types';
import { describeError } from '../utils/errors';

interface Props {
  devices: DeviceOption[];
  filePath: string;
  onClose: () => void;
}

/** Panel for choosing one or more trusted devices to send a file to. */
export const DeviceMultiSelect: React.FC<Props> = ({ devices, filePath, onClose }) => {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [focusIndex, setFocusIndex] = useState(0);
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
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

  // Stay open until every send has been handed off, so a device that cannot
  // be reached is reported here rather than the panel closing on a failure.
  const send = async () => {
    if (selected.size === 0 || sending) return;
    setSending(true);
    setSendError(null);
    const targets = devices.filter(d => selected.has(d.device_id));
    const results = await Promise.allSettled(
      targets.map(d => invoke('send_files_cmd', { deviceId: d.device_id, localPaths: [filePath] })),
    );
    const failed = targets.filter((_, i) => results[i].status === 'rejected');
    setSending(false);
    if (failed.length === 0) {
      onClose();
      return;
    }
    const first = results.find(r => r.status === 'rejected') as PromiseRejectedResult;
    const names = failed.map(d => d.device_name).join(', ');
    setSendError(`Could not send to ${names}. ${describeError(first.reason, 'Try again.')}`);
    setSelected(new Set(failed.map(d => d.device_id)));
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
        void send();
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
        borderRadius: '0 0 8px 8px',
        padding: '8px 0',
        outline: 'none',
      }}
    >
      <div className="section-heading" style={{ padding: '4px 16px 0' }}>
        Send to devices
      </div>

      {devices.length === 0 ? (
        <p style={{ padding: '8px 16px', fontSize: '12px', color: 'var(--muted)' }}>
          No paired devices yet. Pair one in Settings under Add Device.
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
                backgroundColor: focused ? 'var(--surface-hover)' : 'transparent',
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
              {!device.online && (
                <span style={{ flexShrink: 0, fontSize: '12px', color: 'var(--muted)' }}>Offline</span>
              )}
            </div>
          );
        })
      )}

      {sendError && (
        <p role="alert" className="error-text" style={{ padding: '4px 16px' }}>
          {sendError}
        </p>
      )}

      <div className="flex items-center gap-2" style={{ padding: '8px 16px 4px' }}>
        <button
          type="button"
          onClick={() => void send()}
          disabled={selected.size === 0 || sending}
          className="btn btn-primary"
        >
          {sending ? 'Sending...' : 'Send'}
        </button>
        <button type="button" onClick={onClose} className="btn btn-ghost">
          Cancel
        </button>
      </div>
    </div>
  );
};
