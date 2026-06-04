import React, { useEffect, useRef, useState } from 'react';

interface Props {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Inline confirmation overlaid on a result row, e.g. before a remote download. */
export const InlineConfirm: React.FC<Props> = ({ message, onConfirm, onCancel }) => {
  const [focused, setFocused] = useState<'confirm' | 'cancel'>('confirm');
  const confirmRef = useRef<HTMLButtonElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    (focused === 'confirm' ? confirmRef : cancelRef).current?.focus();
  }, [focused]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (focused === 'confirm') onConfirm();
      else onCancel();
    } else if (e.key === 'y' || e.key === 'Y') {
      e.preventDefault();
      onConfirm();
    } else if (e.key === 'n' || e.key === 'N' || e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'Tab') {
      e.preventDefault();
      setFocused(f => (f === 'confirm' ? 'cancel' : 'confirm'));
    }
  };

  const buttonBase: React.CSSProperties = {
    borderRadius: '4px',
    padding: '4px 12px',
    fontSize: '12px',
    cursor: 'pointer',
  };

  return (
    <div
      onKeyDown={handleKeyDown}
      onClick={e => e.stopPropagation()}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        zIndex: 200,
        backgroundColor: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: '6px',
        padding: '8px 12px',
      }}
    >
      <p style={{ color: 'var(--text)', fontSize: '13px' }}>{message}</p>
      <div className="flex items-center gap-2" style={{ marginTop: '8px' }}>
        <button
          ref={confirmRef}
          type="button"
          onClick={onConfirm}
          style={{
            ...buttonBase,
            backgroundColor: 'var(--accent)',
            color: '#fff',
            border: '1px solid var(--accent)',
          }}
        >
          Download
        </button>
        <button
          ref={cancelRef}
          type="button"
          onClick={onCancel}
          style={{
            ...buttonBase,
            backgroundColor: 'transparent',
            color: 'var(--muted)',
            border: '1px solid var(--border)',
          }}
        >
          Cancel
        </button>
      </div>
    </div>
  );
};
