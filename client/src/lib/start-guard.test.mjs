import test from "node:test";
import assert from "node:assert/strict";
import { createStartGuard } from "./start-guard.mjs";

test("a second start while one is in flight is refused", () => {
  const guard = createStartGuard();
  const first = guard.begin();
  assert.notEqual(first, null);
  assert.equal(guard.begin(), null);
  guard.end(first);
  assert.notEqual(guard.begin(), null);
});

test("Back makes the in-flight start stale so its result is discarded", () => {
  const guard = createStartGuard();
  const token = guard.begin();
  guard.invalidate();
  assert.equal(guard.isCurrent(token), false);
  // A new start is allowed right away and is the current one.
  const next = guard.begin();
  assert.equal(guard.isCurrent(next), true);
  assert.equal(guard.isCurrent(token), false);
});

test("ending a stale token does not release a newer start", () => {
  const guard = createStartGuard();
  const stale = guard.begin();
  guard.invalidate();
  const fresh = guard.begin();
  guard.end(stale);
  assert.equal(guard.begin(), null, "fresh start is still in flight");
  guard.end(fresh);
  assert.notEqual(guard.begin(), null);
});
