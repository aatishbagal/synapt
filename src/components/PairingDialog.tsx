import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Peer, TrustedPeer } from '../types';
import { describeError } from '../utils/errors';

interface IncomingPair {
  device_id: string;
  device_name: string;
  verify_code: string;
}

interface Props {
  /// 'manual' is the initiator side of manual pairing: the caller has already
  /// run begin_pairing via pair_by_ip or redeem_invite_code, so the dialog opens
  /// straight on the verification step with the code that call returned.
  mode: 'initiator' | 'responder' | 'manual';
  peer?: Peer;
  incomingPair?: IncomingPair;
  verifyCode?: string;
  peerLabel?: string;
  onClose: () => void;
}

type Step = 'connecting' | 'verify' | 'prompt' | 'waiting' | 'success' | 'error';

const codeStyle: React.CSSProperties = {
  fontFamily: 'monospace',
  fontSize: '32px',
  letterSpacing: '0.2em',
  color: 'var(--text)',
};

function formatCode(code: string): string {
  return code.length === 8 ? `${code.slice(0, 4)} ${code.slice(4)}` : code;
}

function initialStep(mode: Props['mode']): Step {
  if (mode === 'initiator') return 'connecting';
  if (mode === 'manual') return 'verify';
  return 'prompt';
}

export const PairingDialog: React.FC<Props> = ({
  mode,
  peer,
  incomingPair,
  verifyCode,
  peerLabel,
  onClose,
}) => {
  const [step, setStep] = useState<Step>(initialStep(mode));
  const [code, setCode] = useState<string>(verifyCode ?? incomingPair?.verify_code ?? '');
  const [pairedName, setPairedName] = useState<string>(
    peerLabel ?? incomingPair?.device_name ?? '',
  );
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
        setErrorMsg(describeError(e, 'Could not reach the other device. Try pairing again.'));
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
      setErrorMsg(describeError(e, 'Pairing did not complete. Try pairing again.'));
      setStep('error');
    }
  };

  const onAccept = async () => {
    setStep('waiting');
    try {
      await invoke('accept_pair_cmd');
    } catch (e) {
      setErrorMsg(describeError(e, 'Pairing did not complete. Try pairing again.'));
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

  // Escape always backs out: reject an incoming request, otherwise close.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      if (step === 'prompt' || step === 'waiting') void onReject();
      else onClose();
    };
    document.addEventListener('keydown', onKeyDown, true);
    return () => document.removeEventListener('keydown', onKeyDown, true);
    // onReject only depends on onClose.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, onClose]);

  const primary = 'btn btn-primary flex-1';
  const secondary = 'btn btn-secondary flex-1';

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Pair a device"
      className="fixed inset-0 flex items-center justify-center z-50"
      style={{ backgroundColor: 'var(--backdrop)' }}
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
          <div className="flex flex-col gap-4">
            <p role="status" className="text-sm text-center" style={{ color: 'var(--muted)' }}>
              Connecting...
            </p>
            <button onClick={onClose} className={secondary}>Cancel</button>
          </div>
        )}

        {step === 'verify' && (
          <div className="flex flex-col gap-4">
            <p className="text-center" style={codeStyle}>{formatCode(code)}</p>
            <p className="text-xs text-center" style={{ color: 'var(--muted)' }}>
              Compare this code on the other device. If they match, click Confirm.
            </p>
            <div className="flex gap-2">
              <button onClick={onConfirm} className={primary}>Confirm</button>
              <button onClick={onClose} className={secondary}>Cancel</button>
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
              <button onClick={onAccept} className={primary}>Accept</button>
              <button onClick={onReject} className={secondary}>Reject</button>
            </div>
          </div>
        )}

        {step === 'waiting' && (
          <div className="flex flex-col gap-4">
            <p role="status" className="text-sm text-center" style={{ color: 'var(--muted)' }}>
              Waiting for the other device to confirm...
            </p>
            <button onClick={onReject} className={secondary}>Cancel pairing</button>
          </div>
        )}

        {step === 'success' && (
          <div className="flex flex-col gap-4">
            <p className="text-sm text-center" style={{ color: 'var(--text)' }}>Device paired.</p>
            <p className="text-xs text-center" style={{ color: 'var(--muted)' }}>{pairedName}</p>
            <button onClick={onClose} className="btn btn-primary">Done</button>
          </div>
        )}

        {step === 'error' && (
          <div className="flex flex-col gap-4">
            <p className="text-sm text-center" style={{ color: 'var(--text)' }}>Pairing failed.</p>
            <p className="text-xs text-center break-words" style={{ color: 'var(--muted)' }}>{errorMsg}</p>
            <button onClick={onClose} className="btn btn-secondary">Close</button>
          </div>
        )}
      </div>
    </div>
  );
};
