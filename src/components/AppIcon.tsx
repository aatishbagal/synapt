import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  iconPath: string | null;
  size?: number;
}

/**
 * Render an installed application's icon, loaded from the backend as a data
 * URI. Falls back to a generic placeholder when no icon is available or it
 * fails to load.
 */
export const AppIcon: React.FC<Props> = ({ iconPath, size = 20 }) => {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!iconPath) {
      setSrc(null);
      return;
    }
    // Remote results carry the icon inline as a data URI (the file lives on the
    // other device); render it directly rather than reading a local path.
    if (iconPath.startsWith('data:')) {
      setSrc(iconPath);
      return;
    }
    let active = true;
    invoke<string>('get_app_icon', { iconPath })
      .then(result => {
        if (active) setSrc(result);
      })
      .catch(() => {
        if (active) setSrc(null);
      });
    return () => {
      active = false;
    };
  }, [iconPath]);

  if (src) {
    return (
      <img
        src={src}
        width={size}
        height={size}
        style={{ borderRadius: 4, objectFit: 'contain', flexShrink: 0 }}
        alt=""
        draggable={false}
      />
    );
  }

  return (
    <div
      style={{
        width: size,
        height: size,
        borderRadius: 4,
        background: 'var(--border)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        flexShrink: 0,
      }}
    >
      <svg width={size * 0.6} height={size * 0.6} viewBox="0 0 12 12" fill="none">
        <rect x="1" y="1" width="10" height="10" rx="2" stroke="var(--muted)" strokeWidth="1.5" />
        <path d="M4 6h4M6 4v4" stroke="var(--muted)" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    </div>
  );
};
