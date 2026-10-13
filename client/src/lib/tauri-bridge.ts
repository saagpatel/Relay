import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface SendStarted {
  code: string;
  session_id: string;
  port: number;
}

export interface FileOfferInfo {
  name: string;
  size: number;
  relativePath?: string;
}

export interface TransferProgress {
  type: "transferProgress";
  session_id: string;
  bytes_transferred: number;
  bytes_total: number;
  speed_bps: number;
  eta_seconds: number;
  current_file: string;
  percent: number;
}

export interface TransferCompleteEvent {
  type: "transferComplete";
  session_id: string;
  duration_seconds: number;
  average_speed: number;
  total_bytes: number;
  file_count: number;
}

export interface FileOfferEvent {
  type: "fileOffer";
  session_id: string;
  files: FileOfferInfo[];
}

export interface FileCompletedEvent {
  type: "fileCompleted";
  session_id: string;
  name: string;
}

export interface ErrorEvent {
  type: "error";
  session_id: string;
  message: string;
}

export interface StateChangedEvent {
  type: "stateChanged";
  session_id: string;
  state: string;
}

export interface ConnectionTypeChangedEvent {
  type: "connectionTypeChanged";
  session_id: string;
  connection_type: "direct" | "relay";
}

export type ProgressEvent =
  | TransferProgress
  | TransferCompleteEvent
  | FileOfferEvent
  | FileCompletedEvent
  | ErrorEvent
  | StateChangedEvent
  | ConnectionTypeChangedEvent;

export async function startSend(
  filePaths: string[],
  signalServerUrl?: string
): Promise<SendStarted> {
  return invoke<SendStarted>("start_send", {
    filePaths,
    signalServerUrl,
  });
}

export async function startReceive(
  code: string,
  saveDir: string,
  signalServerUrl?: string
): Promise<string> {
  return invoke<string>("start_receive", {
    code,
    saveDir,
    signalServerUrl,
  });
}

export async function acceptTransfer(
  sessionId: string,
  accept: boolean
): Promise<void> {
  return invoke("accept_transfer", { sessionId, accept });
}

export async function cancelTransfer(sessionId: string): Promise<void> {
  return invoke("cancel_transfer", { sessionId });
}

export function onTransferProgress(
  handler: (event: ProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<ProgressEvent>("transfer:progress", (e) => handler(e.payload));
}
