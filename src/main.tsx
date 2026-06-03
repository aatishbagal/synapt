import React from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter, Routes, Route } from 'react-router-dom';
import { Overlay } from './pages/Overlay';
import { Settings } from './pages/Settings';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <HashRouter>
      <Routes>
        <Route path="/" element={<Overlay />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </HashRouter>
  </React.StrictMode>
);
