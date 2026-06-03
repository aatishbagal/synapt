import React, { useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter, Routes, Route } from 'react-router-dom';
import { Overlay } from './pages/Overlay';
import { Settings } from './pages/Settings';
import { useTheme } from './hooks/useTheme';
import './index.css';

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
  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<Overlay />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </HashRouter>
  );
};

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
