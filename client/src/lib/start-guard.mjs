import { onCleanup } from "solid-js";

// Guards the sender's start against duplicate clicks and stale results.
// start_send now talks to the signaling server before it returns (nameplate
// allocation), so a start can still be in flight when the user presses Back
// or tries again. Only the current start may apply its result.
export function createStartGuard() {
  let generation = 0;
  let inFlight = null;
  return {
    /** Returns a token, or null if a start is already in flight. */
    begin() {
      if (inFlight !== null) return null;
      generation += 1;
      inFlight = generation;
      return generation;
    },
    /** True while `token` is the start whose result should be applied. */
    isCurrent(token) {
      return token !== null && token === generation;
    },
    /** Called when a start finishes; releases the slot if it is still current. */
    end(token) {
      if (token === inFlight) inFlight = null;
    },
    /** Back or reset: whatever is in flight is now stale. */
    invalidate() {
      generation += 1;
      inFlight = null;
    },
  };
}

/**
 * A start guard owned by the calling component: when the component is
 * disposed (Escape, opening Settings, leaving the view), any start still in
 * flight becomes stale, so its late result is discarded instead of
 * overwriting the shared transfer state.
 */
export function useStartGuard() {
  const guard = createStartGuard();
  onCleanup(() => guard.invalidate());
  return guard;
}
