import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Peer, TrustedPeer } from '../types';

interface IncomingPair {
  device_id: string;
  device_name: string;
  verify_code: string;
}

interface Props {
  mode: 'initiator' | 'responder';
  peer?: Peer;
  incomingPair?: IncomingPair;
  onClose: () => void;
}

type Step = 'connecting' | 'verify' | 'prompt' | 'waiting' | 'success' | 'error';

const codeStyle: React.CSSProperties = {
  fontFamily: 'monospace',
  fontSize: '32px',
  letterSpacing: '0.2em',
  color: 'var(--accent)',
};

const primaryButton: React.CSSProperties = {
  backgroundColor: 'var(--surface-hover)',
  color: 'var(--accent)',
  border: '1px solid var(--accent)',
};

const secondaryButton: React.CSSProperties = {
  backgroundColor: 'var(--surface)',
  color: 'var(--text)',
  border: '1px solid var(--border)',
};

function formatCode(code: string): string {
  return code.length === 8 ? `${code.slice(0, 4)} ${code.slice(4)}` : code;
}

export const PairingDialog: React.FC<Props> = ({ mode, peer, incomingPair, onClose }) => {
  const [step, setStep] = useState<Step>(mode === 'initiator' ? 'connecting' : 'prompt');
  const [code, setCode] = useState<string>(incomingPair?.verify_code ?? '');
  const [pairedName, setPairedName] = useState<string>(incomingPair?.device_name ?? '');
  const [errorMsg, setErrorMsg] = useState<string>('');

  useEffect(() => {
    if (mode !== 'initiator' || !peer) {
      return;
    }
    let active = true;
    (async () => {
      try {
        const verify = await invoke<string>('begin_pairing_cmd', { deviceId: peer.device_id });
        if (!active) {
          return;
        }
        setCode(verify);
        setPairedName(peer.device_name);
        setStep('verify');
      } catch (e) {
        if (!active) {
          return;
        }
        setErrorMsg(String(e));
        setStep('error');
      }
    })();
    return () => {
      active = false;
    };
  }, [mode, peer]);

  useEffect(() => {
    if (mode !== 'responder' || step !== 'waiting') {
      return;
    }
    const unlisten = listen<TrustedPeer>('pair-complete', e => {
      setPairedName(e.payload.device_name);
      setStep('success');
    });
    return () => {
      unlisten.then(fn => fn());
    };
  }, [mode, step]);

  const onConfirm = async () => {
    try {
      const trusted = await invoke<TrustedPeer>('confirm_pairing_cmd');
      setPairedName(trusted.device_name);
      setStep('success');
    } catch (e) {
      setErrorMsg(String(e));
      setStep('error');
    }
  };

  const onAccept = async () => {
    setStep('waiting');
    try {
      await invoke('accept_pair_cmd');
    } catch (e) {
      setErrorMsg(String(e));
      setStep('error');
    }
  };

  const onReject = async () => {
    try {
      await invoke('reject_pair_cmd');
    } catch {
      // ignore: closing regardless
    }
    onClose();
  };

  const btnClass = 'flex-1 text-xs px-3 py-2 rounded transition-colors';

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-50"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.6)' }}
    >
      <div
        className="p-6 w-80"
        style={{
          backgroundColor: 'var(--surface)',
          border: '1px solid var(--border)',
          borderRadius: '8px',
        }}
      >
        {step === 'connecting' && (
          <p className="text-sm text-center" style={{ color: 'var(--muted)' }}>
            Connecting...
          </p>
        )}

        {step === 'verify' && (
          <div className="flex flex-col gap-4">
            <p className="text-center" style={codeStyle}>{formatCode(code)}</p>
            <p className="text-xs text-center" style={{ color: 'var(--muted)' }}>
              Compare this code on the other device. If they match, click Confirm.
            </p>
            <div className="flex gap-2">
              <button onClick={onConfirm} className={btnClass} style={primaryButton}>Confirm</button>
              <button onClick={onClose} className={btnClass} style={secondaryButton}>Cancel</button>
            </div>
          </div>
        )}

        {step === 'prompt' && incomingPair && (
          <div className="flex flex-col gap-4">
            <p className="text-sm text-center" style={{ color: 'var(--text)' }}>
              {incomingPair.device_name} wants to pair.
            </p>
            <p className="text-center" style={codeStyle}>{formatCode(code)}</p>
            <p className="text-xs text-center" style={{ color: 'var(--muted)' }}>
              If this code matches the one on the other device, click Accept.
            </p>
            <div className="flex gap-2">
              <button onClick={onAccept} className={btnClass} style={primaryButton}>Accept</button>
              <button onClick={onReject} className={btnClass} style={secondaryButton}>Reject</button>
            </div>
          </div>
        )}

        {step === 'waiting' && (
          <p className="text-sm text-center" style={{ color: 'var(--muted)' }}>
            Completing pairing...
          </p>
        )}

        {step === 'success' && (
          <div className="flex flex-col gap-4">
            <p className="text-sm text-center" style={{ color: 'var(--text)' }}>Device paired.</p>
            <p className="text-xs text-center" style={{ color: 'var(--muted)' }}>{pairedName}</p>
            <button onClick={onClose} className="text-xs px-3 py-2 rounded transition-colors" style={primaryButton}>Done</button>
          </div>
        )}

        {step === 'error' && (
          <div className="flex flex-col gap-4">
            <p className="text-sm text-center" style={{ color: 'var(--text)' }}>Pairing failed.</p>
            <p className="text-xs text-center break-words" style={{ color: 'var(--muted)' }}>{errorMsg}</p>
            <button onClick={onClose} className="text-xs px-3 py-2 rounded transition-colors" style={secondaryButton}>Close</button>
          </div>
        )}
      </div>
    </div>
  );
};
