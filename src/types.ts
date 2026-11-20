export type View = "nearby" | "chat"

export type Platform = "macos" | "windows" | "linux" | string

export interface DeviceInfo {
  id: string
  name: string
  platform: Platform
}

export interface Peer {
  id: string
  name: string
  platform: Platform
  ip: string
  port: number
  busy: boolean
  lastSeen: number
}

export interface NetworkInfo {
  ip: string
  discoveryPort: number
  available: boolean
}

export interface SessionInfo {
  id: string
  peer: Peer
  startedAt: number
  initiatedByMe: boolean
}

export type TransferState =
  | "waiting"
  | "transferring"
  | "paused"
  | "completed"
  | "cancelled"
  | "failed"

export interface Transfer {
  id: string
  sessionId: string
  direction: "in" | "out"
  name: string
  size: number
  transferred: number
  state: TransferState
  speedBps: number
  etaSecs: number | null
  path: string | null
  error: string | null
}

export interface ChatMessage {
  id: string
  sessionId: string
  direction: "in" | "out"
  kind: "text" | "file"
  text: string | null
  transfer: Transfer | null
  at: number
}

export interface IncomingRequest {
  requestId: string
  peer: Peer
}

export type OutgoingStatus =
  | "pending"
  | "accepted"
  | "rejected"
  | "failed"
  | "cancelled"
  | "timeout"

export interface OutgoingRequest {
  requestId: string
  peer: Peer
  status: OutgoingStatus
  reason: string | null
}

export interface SessionEnded {
  sessionId: string
  reason: "local" | "peer" | "error" | string
  message: string | null
}

export interface Bootstrap {
  device: DeviceInfo
  peers: Peer[]
  session: SessionInfo | null
  network: NetworkInfo
  discoverable: boolean
  downloadDir: string
  version: string
}

export interface LocalinkState {
  device: DeviceInfo | null
  peers: Peer[]
  network: NetworkInfo | null
  session: SessionInfo | null
  messages: ChatMessage[]
  transfers: Record<string, Transfer>
  incomingRequest: IncomingRequest | null
  outgoingRequest: OutgoingRequest | null
  view: View
  version: string
  ready: boolean
  error: string | null
}
