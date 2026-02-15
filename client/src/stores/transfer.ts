import { createStore } from "solid-js/store";
import type { FileOfferInfo } from "../lib/tauri-bridge";

export interface SpeedDataPoint {
  timestamp: number;
  speedBps: number;
}

export interface TransferProgress {
  bytesTransferred: number;
  bytesTotal: number;
  speedBps: number;
  etaSeconds: number;
  currentFile: string;
  percent: number;
  completedFiles: string[];
  speedHistory: SpeedDataPoint[];
}

export interface TransferSummary {
  fileCount: number;
  totalBytes: number;
  durationSeconds: number;
  averageSpeed: number;
}

export type TransferPhase =
  | "idle"
  | "selecting"
  | "waiting"
  | "entering-code"
  | "connecting"
  | "offer"
  | "transferring"
  | "completed"
  | "error";

export interface TransferStore {
  phase: TransferPhase;
  role: "sender" | "receiver" | null;
  code: string;
  sessionId: string;
  senderPort: number;
  selectedFiles: string[];
  offerFiles: FileOfferInfo[];
  progress: TransferProgress;
  summary: TransferSummary;
  error: string;
  connectionType: "direct" | "relay" | "negotiating";
}

const defaultProgress: TransferProgress = {
  bytesTransferred: 0,
  bytesTotal: 0,
  speedBps: 0,
  etaSeconds: 0,
  currentFile: "",
  percent: 0,
  completedFiles: [],
  speedHistory: [],
};

const defaultSummary: TransferSummary = {
  fileCount: 0,
  totalBytes: 0,
  durationSeconds: 0,
  averageSpeed: 0,
};

export const [transfer, setTransfer] = createStore<TransferStore>({
  phase: "idle",
  role: null,
  code: "",
  sessionId: "",
  senderPort: 0,
  selectedFiles: [],
  offerFiles: [],
  progress: { ...defaultProgress },
  summary: { ...defaultSummary },
  error: "",
  connectionType: "negotiating",
});

export function resetTransfer() {
  setTransfer({
    phase: "idle",
    role: null,
    code: "",
    sessionId: "",
    senderPort: 0,
    selectedFiles: [],
    offerFiles: [],
    progress: { ...defaultProgress },
    summary: { ...defaultSummary },
    error: "",
    connectionType: "negotiating",
  });
}

export function addSpeedDataPoint(speedBps: number) {
  const now = Date.now();
  setTransfer("progress", "speedHistory", (prev) => {
    // Keep only last 30 seconds of data
    const filtered = prev.filter(p => now - p.timestamp < 30000);
    return [...filtered, { timestamp: now, speedBps }];
  });
}
