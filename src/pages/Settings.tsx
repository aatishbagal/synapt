import React from 'react';
import { useNavigate } from 'react-router-dom';

export const Settings: React.FC = () => {
  const nav = useNavigate();
  return (
    <div className="w-full h-screen bg-bg p-6">
      <button onClick={() => nav('/')} className="text-text-muted text-sm mb-6 hover:text-text-primary">Back</button>
      <p className="text-text-muted text-sm">Settings — v0.1.0</p>
    </div>
  );
};
