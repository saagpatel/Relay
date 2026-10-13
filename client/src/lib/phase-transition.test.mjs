import test from "node:test";
import assert from "node:assert/strict";
import { phaseForState } from "./phase-transition.mjs";

test("sender remains waiting when connecting arrives before or after waiting", () => {
  const first = phaseForState("waiting", "sender", "connecting");
  assert.equal(phaseForState(first, "sender", "waiting"), "waiting");
  const second = phaseForState("selecting", "sender", "waiting");
  assert.equal(phaseForState(second, "sender", "connecting"), "waiting");
});

test("receiver applies its connecting state", () => {
  assert.equal(phaseForState("entering-code", "receiver", "connecting"), "connecting");
});
