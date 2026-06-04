import React from 'react';
import { DeviceOption } from '../types';

interface Props {
  devices: DeviceOption[];
  selectedIndex: number;
  onSelect: (device: DeviceOption) => void;
  // Escape/close is driven from the search input's key handler in Overlay.
  onClose: () => void;
}

/** Dropdown listing trusted devices, opened by typing @ in the search bar. */
export const DevicePicker: React.FC<Props> = ({ devices, selectedIndex, onSelect }) => {
  return (
    <div
      role="listbox"
      style={{
        position: 'absolute',
        top: '100%',
        left: 0,
        width: '100%',
        backgroundColor: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: '8px',
        zIndex: 100,
        maxHeight: '240px',
        overflowY: 'auto',
      }}
    >
      {devices.length === 0 ? (
        <p
          style={{
            color: 'var(--muted)',
            fontSize: '12px',
            textAlign: 'center',
            padding: '16px',
          }}
        >
          No paired devices. Pair a device in Settings.
        </p>
      ) : (
        devices.map((device, index) => {
          const selected = index === selectedIndex;
          return (
            <div
              key={device.device_id}
              role="option"
              aria-selected={selected}
              onMouseDown={e => {
                // Keep the search input focused; mousedown fires before blur.
                e.preventDefault();
                onSelect(device);
              }}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: '10px',
                padding: '10px 14px',
                cursor: 'pointer',
                backgroundColor: selected
                  ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
                  : 'transparent',
                borderLeft: selected ? '2px solid var(--accent)' : '2px solid transparent',
              }}
            >
              <div style={{ minWidth: 0 }}>
                <div
                  style={{
                    color: 'var(--text)',
                    fontSize: '13px',
                    fontWeight: 500,
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                >
                  {device.device_name}
                </div>
                {device.ip && (
                  <div style={{ color: 'var(--muted)', fontSize: '11px' }}>{device.ip}</div>
                )}
              </div>
              <span
                aria-label={device.online ? 'Online' : 'Offline'}
                style={{
                  flexShrink: 0,
                  width: '7px',
                  height: '7px',
                  borderRadius: '50%',
                  backgroundColor: device.online ? 'var(--accent)' : 'var(--border)',
                }}
              />
            </div>
          );
        })
      )}
    </div>
  );
};
