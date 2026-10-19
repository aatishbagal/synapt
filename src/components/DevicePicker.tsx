import React from 'react';
import { motion } from 'motion/react';
import { SPRING_GENTLE } from '../animations';
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
    <motion.div
      role="listbox"
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={SPRING_GENTLE}
      style={{
        transformOrigin: 'top center',
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
          No paired devices yet. Pair one in Settings under Add Device.
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
              data-selected={selected}
              className="list-row"
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: '12px',
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
                  <div style={{ color: 'var(--muted)', fontSize: '12px' }}>{device.ip}</div>
                )}
              </div>
              {/* Text label, not a status dot. */}
              <span style={{ flexShrink: 0, fontSize: '12px', color: device.online ? 'var(--text)' : 'var(--muted)' }}>
                {device.online ? 'Online' : 'Offline'}
              </span>
            </div>
          );
        })
      )}
    </motion.div>
  );
};
