import React, { useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter, Routes, Route } from 'react-router-dom';
import { MotionConfig } from 'motion/react';
import { Overlay } from './pages/Overlay';
import { Settings } from './pages/Settings';
import { TransferHistory } from './pages/TransferHistory';
import { useTheme } from './hooks/useTheme';
import './index.css';
// Reads experimental_ui once at startup and applies data-experimental-ui.
import './experimental';

// Apply the theme before first render to avoid a flash. invoke() is async, so
// we read the last-known value from localStorage synchronously here; the real
// value from SQLite is reconciled in Root once the app mounts.
const stored = localStorage.getItem('synapt-theme') ?? 'dark';
document.documentElement.setAttribute('data-theme', stored);

const Root: React.FC = () => {
  const { load } = useTheme();
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  // reducedMotion="user" drops transform and layout animation when the OS asks
  // for reduced motion, leaving only opacity fades.
  return (
    <MotionConfig reducedMotion="user">
      <HashRouter>
        <Routes>
          <Route path="/" element={<Overlay />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/transfers" element={<TransferHistory />} />
        </Routes>
      </HashRouter>
    </MotionConfig>
  );
};

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
