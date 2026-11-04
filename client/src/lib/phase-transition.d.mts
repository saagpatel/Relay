import type { TransferPhase } from "../stores/transfer";
export function phaseForState(current: TransferPhase, role: "sender" | "receiver" | null, state: string): TransferPhase;
