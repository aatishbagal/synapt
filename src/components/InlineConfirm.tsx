import React, { useEffect, useRef, useState } from 'react';
import { motion, useReducedMotion } from 'motion/react';
import { INLINE_CONFIRM } from '../animations';

interface Props {
  message: string;
  confirmLabel?: string;
  /** 'danger' marks an action that removes something and cannot be undone. */
  tone?: 'default' | 'danger';
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Inline confirmation overlaid on its positioned parent, e.g. a result row
 * before a remote download or a Settings card before removing a device.
 */
export const InlineConfirm: React.FC<Props> = ({
  message,
  confirmLabel = 'Download',
  tone = 'default',
  onConfirm,
  onCancel,
}) => {
  const [focused, setFocused] = useState<'confirm' | 'cancel'>('confirm');
  // Height is not a transform, so MotionConfig's reduced-motion setting does
  // not cover it; fall back to a plain fade.
  const reduceMotion = useReducedMotion();
  const confirmRef = useRef<HTMLButtonElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);

  // preventScroll: focusing inside the clipped, still-growing box would
  // otherwise scroll its content and make it jump while it expands.
  useEffect(() => {
    (focused === 'confirm' ? confirmRef : cancelRef).current?.focus({ preventScroll: true });
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

  return (
    <motion.div
      initial={reduceMotion ? { opacity: 0 } : INLINE_CONFIRM.initial}
      animate={reduceMotion ? { opacity: 1 } : INLINE_CONFIRM.animate}
      exit={reduceMotion ? { opacity: 0 } : INLINE_CONFIRM.exit}
      transition={INLINE_CONFIRM.transition}
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
        overflow: 'hidden',
      }}
    >
      {/* Padding lives on an inner box so the outer one can animate to height 0. */}
      <div style={{ padding: '8px 12px' }}>
        <p role="alert" style={{ color: 'var(--text)', fontSize: '13px' }}>{message}</p>
        <div className="flex items-center gap-2" style={{ marginTop: '8px' }}>
          {/* DESIGN.md: secondary-weight confirm with accent text (danger text for
              removals), plain muted cancel. */}
          <button
            ref={confirmRef}
            type="button"
            onClick={onConfirm}
            className={tone === 'danger' ? 'btn btn-destructive' : 'btn btn-secondary'}
            style={tone === 'danger' ? undefined : { color: 'var(--accent)' }}
          >
            {confirmLabel}
          </button>
          <button ref={cancelRef} type="button" onClick={onCancel} className="btn btn-ghost">
            Cancel
          </button>
        </div>
      </div>
    </motion.div>
  );
};
