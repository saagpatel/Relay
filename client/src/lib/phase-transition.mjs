export function phaseForState(current, role, state) {
  if (state === "connecting") return role === "receiver" ? "connecting" : current;
  if (state === "waiting") return role === "sender" ? "waiting" : current;
  if (state === "transferring") return "transferring";
  return current;
}
