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

export interface QueueEntry {
  transfer_id: string;
  filename: string;
  remote_path: string;
  peer_name: string;
  status: 'Queued' | 'InProgress' | 'Complete' | { Failed: { reason: string } } | 'Partial';
  bytes_received: number;
  total: number;
  started_at: number;
}

export type IndexPhase =
  | { type: 'Starting' }
  | { type: 'Scanning' }
  | { type: 'BuildingIndex' }
  | { type: 'Complete'; total_files: number }
  | { type: 'Failed'; reason: string };

export interface IndexProgress {
  phase: IndexPhase;
  files_scanned: number;
  current_dir: string;
  total_dirs: number;
  dirs_done: number;
}

export type InputMode = 'local' | 'folder' | 'remote' | 'settings' | 'calc';

export interface ParsedInput {
  raw: string;
  mode: InputMode;
  query: string;
  deviceName: string | null;
}

export type ResultSource = 'LocalFile' | 'LocalApp' | { Remote: { device_name: string } };

export type ResultType = 'File' | 'App';

export interface SearchResult {
  name: string;
  path: string;
  exec: string | null;
  result_type: ResultType;
  source: ResultSource;
  score: number;
}
