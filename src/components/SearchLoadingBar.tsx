import React from 'react';

interface Props {
  visible: boolean;
}

/** Thin indeterminate loading bar shown below the search bar during remote search. */
export const SearchLoadingBar: React.FC<Props> = ({ visible }) => {
  if (!visible) return null;
  return (
    <div
      className="shrink-0"
      style={{
        position: 'relative',
        width: '100%',
        height: '3px',
        backgroundColor: 'var(--border)',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          position: 'absolute',
          height: '100%',
          width: '40%',
          backgroundColor: 'var(--accent)',
          animation: 'search-loading 1.2s linear infinite',
        }}
      />
    </div>
  );
};
