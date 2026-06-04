import React from 'react';

interface Props {
  /** 'active' animates; 'done'/'failed' show a static coloured underline. */
  state?: 'active' | 'done' | 'failed';
  width?: number;
}

/**
 * Indexing activity indicator: a thin underline that fills from the left and
 * then empties from the left in a smooth, infinite cycle (Arc / Pixel-boot
 * style). On a terminal state it renders a static success/danger underline.
 */
export const UnderlineLoader: React.FC<Props> = ({ state = 'active', width = 20 }) => {
  const colour =
    state === 'done' ? 'var(--success)' : state === 'failed' ? 'var(--danger)' : 'var(--text)';

  return (
    <span
      style={{
        position: 'relative',
        display: 'inline-block',
        width,
        height: 2,
        borderRadius: 1,
        flexShrink: 0,
        overflow: 'hidden',
      }}
    >
      <span
        style={
          state === 'active'
            ? {
                position: 'absolute',
                top: 0,
                bottom: 0,
                background: colour,
                borderRadius: 1,
                animation: 'underline-load 1.2s ease-in-out infinite',
              }
            : { position: 'absolute', inset: 0, background: colour, borderRadius: 1 }
        }
      />
    </span>
  );
};
