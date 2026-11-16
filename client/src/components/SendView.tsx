import { Show, createSignal } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";
import { transfer, setTransfer, resetTransfer } from "../stores/transfer";
import { settings } from "../stores/settings";
import { startSend } from "../lib/tauri-bridge";
import { cancelTransfer } from "../lib/tauri-bridge";
import { createStartGuard } from "../lib/start-guard.mjs";
import CodeDisplay from "./CodeDisplay";
import FileList from "./FileList";

export default function SendView() {
  // start_send allocates a nameplate before it returns, so a start can be in
  // flight when the user clicks again or goes Back.
  const guard = createStartGuard();
  const [starting, setStarting] = createSignal(false);

  async function selectFiles() {
    const selected = await open({
      multiple: true,
      directory: false,
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      setTransfer("selectedFiles", [...transfer.selectedFiles, ...paths]);
    }
  }

  async function selectFolder() {
    const selected = await open({
      multiple: false,
      directory: true,
    });
    if (selected) {
      const path = Array.isArray(selected) ? selected[0] : selected;
      setTransfer("selectedFiles", [...transfer.selectedFiles, path]);
    }
  }

  async function handleSend() {
    if (transfer.selectedFiles.length === 0) return;
    const token = guard.begin();
    if (token === null) return;
    setStarting(true);
    try {
      const result = await startSend(
        transfer.selectedFiles,
        settings.signalServerUrl || undefined
      );
      if (!guard.isCurrent(token)) {
        // The user left or restarted while the nameplate was being allocated.
        await cancelTransfer(result.session_id).catch(() => undefined);
        return;
      }
      setTransfer("code", result.code);
      setTransfer("sessionId", result.session_id);
      setTransfer("senderPort", result.port);
      setTransfer("phase", "waiting");
    } catch (e) {
      if (!guard.isCurrent(token)) return;
      setTransfer("phase", "error");
      setTransfer("error", String(e));
    } finally {
      guard.end(token);
      if (guard.isCurrent(token)) setStarting(false);
    }
  }

  function abandonStart() {
    guard.invalidate();
    setStarting(false);
  }

  return (
    <div class="w-full max-w-md space-y-6">
      <Show when={transfer.phase === "selecting"}>
        <div class="text-center space-y-6">
          <h2 class="text-2xl font-semibold">Send Files</h2>

          <div class="flex gap-3">
            <button
              class="flex-1 py-12 border-2 border-dashed border-[#333] hover:border-[#3b82f6] rounded-xl text-[#a0a0a0] hover:text-white transition-colors"
              onClick={selectFiles}
            >
              <div class="space-y-2">
                <p class="text-lg">Select Files</p>
                <p class="text-sm">Choose individual files</p>
              </div>
            </button>

            <button
              class="flex-1 py-12 border-2 border-dashed border-[#333] hover:border-[#3b82f6] rounded-xl text-[#a0a0a0] hover:text-white transition-colors"
              onClick={selectFolder}
            >
              <div class="space-y-2">
                <p class="text-lg">Select Folder</p>
                <p class="text-sm">Send an entire folder</p>
              </div>
            </button>
          </div>

          <Show when={transfer.selectedFiles.length > 0}>
            <FileList
              files={transfer.selectedFiles.map((path) => ({
                name: path.split("/").pop() || path,
                size: 0,
              }))}
            />
            <button
              class="w-full py-3 bg-[#3b82f6] hover:bg-[#2563eb] disabled:opacity-40 disabled:cursor-not-allowed rounded-lg font-semibold transition-colors"
              onClick={handleSend}
              disabled={starting()}
            >
              {starting() ? "Starting..." : "Start Sending"}
            </button>
          </Show>

          <button
            class="text-sm text-[#a0a0a0] hover:text-white transition-colors"
            onClick={async () => {
              abandonStart();
              if (transfer.sessionId) await cancelTransfer(transfer.sessionId).catch(() => undefined);
              resetTransfer();
            }}
          >
            Back
          </button>
        </div>
      </Show>

      <Show when={transfer.phase === "waiting"}>
        <div class="text-center space-y-6">
          <CodeDisplay code={transfer.code} />
          <div class="flex items-center justify-center gap-2 text-[#a0a0a0]">
            <div class="w-2 h-2 bg-[#3b82f6] rounded-full animate-pulse" />
            <p>Waiting for receiver...</p>
          </div>
          <Show when={transfer.senderPort > 0}>
            <p class="text-xs text-[#555]">
              Listening on port {transfer.senderPort}
            </p>
          </Show>
          <button
            class="text-sm text-[#a0a0a0] hover:text-white transition-colors"
            onClick={async () => {
              abandonStart();
              // Release the session (and its nameplate) on the backend too.
              if (transfer.sessionId) await cancelTransfer(transfer.sessionId).catch(() => undefined);
              resetTransfer();
            }}
          >
            Cancel
          </button>
        </div>
      </Show>
    </div>
  );
}
