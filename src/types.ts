export type PeerStatus = 'Discovered' | 'Pairing' | 'Trusted';

export interface Peer {
  device_id: string;
  device_name: string;
  ip: string;
  pairing_port: number;
  status: PeerStatus;
}

export interface TrustedPeer {
  device_id: string;
  device_name: string;
  pubkey_b64: string;
  fingerprint: string;
  paired_at: number;
  last_seen: number | null;
}

export interface TransferProgress {
  filename: string;
  bytes_received: number;
  total: number;
}

export type InputMode = 'local' | 'folder' | 'remote' | 'settings' | 'calc';

export interface ParsedInput {
  raw: string;
  mode: InputMode;
  query: string;
  deviceName: string | null;
}

export type ResultSource = 'Local' | { Remote: { device_name: string } };

export interface SearchResult {
  name: string;
  path: string;
  source: ResultSource;
  score: number;
}
