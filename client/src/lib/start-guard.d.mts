export interface StartGuard {
  begin(): number | null;
  isCurrent(token: number | null): boolean;
  end(token: number | null): void;
  invalidate(): void;
}

export function createStartGuard(): StartGuard;

/** A guard invalidated automatically when the owning component is disposed. */
export function useStartGuard(): StartGuard;
