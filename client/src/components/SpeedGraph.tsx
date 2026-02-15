import { onMount, createEffect } from "solid-js";
import type { SpeedDataPoint } from "../stores/transfer";

interface Props {
  speedHistory: SpeedDataPoint[];
}

export default function SpeedGraph(props: Props) {
  let canvasRef: HTMLCanvasElement | undefined;

  onMount(() => {
    // Redraw when speed history changes
    createEffect(() => {
      if (canvasRef) {
        drawGraph(canvasRef, props.speedHistory);
      }
    });
  });

  function drawGraph(canvas: HTMLCanvasElement, history: SpeedDataPoint[]) {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const padding = 30;

    // Clear canvas
    ctx.fillStyle = "#0a0a0a";
    ctx.fillRect(0, 0, width, height);

    if (history.length < 2) {
      // Not enough data points
      ctx.fillStyle = "#a0a0a0";
      ctx.font = "12px Inter, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("Waiting for data...", width / 2, height / 2);
      return;
    }

    // Find max speed for scaling
    const maxSpeed = Math.max(...history.map(p => p.speedBps), 1); // At least 1 to avoid division by zero
    const minTimestamp = history[0].timestamp;
    const maxTimestamp = history[history.length - 1].timestamp;
    const timeRange = maxTimestamp - minTimestamp || 1;

    // Draw grid lines
    ctx.strokeStyle = "#1e1e1e";
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
      const y = padding + (height - 2 * padding) * (i / 4);
      ctx.beginPath();
      ctx.moveTo(padding, y);
      ctx.lineTo(width - padding, y);
      ctx.stroke();
    }

    // Draw axes
    ctx.strokeStyle = "#333";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(padding, padding);
    ctx.lineTo(padding, height - padding);
    ctx.lineTo(width - padding, height - padding);
    ctx.stroke();

    // Draw line graph
    ctx.strokeStyle = "#3b82f6";
    ctx.lineWidth = 2;
    ctx.beginPath();

    history.forEach((point, i) => {
      const x = padding + ((point.timestamp - minTimestamp) / timeRange) * (width - 2 * padding);
      const y = height - padding - (point.speedBps / maxSpeed) * (height - 2 * padding);

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });

    ctx.stroke();

    // Fill area under the curve
    ctx.fillStyle = "rgba(59, 130, 246, 0.2)";
    ctx.lineTo(width - padding, height - padding);
    ctx.lineTo(padding, height - padding);
    ctx.closePath();
    ctx.fill();

    // Draw labels
    ctx.fillStyle = "#a0a0a0";
    ctx.font = "10px Inter, sans-serif";
    ctx.textAlign = "right";

    // Y-axis labels (speed)
    for (let i = 0; i <= 4; i++) {
      const speed = maxSpeed * (1 - i / 4);
      const y = padding + (height - 2 * padding) * (i / 4);
      const label = formatSpeed(speed);
      ctx.fillText(label, padding - 5, y + 3);
    }

    // Current speed label
    const currentSpeed = history[history.length - 1]?.speedBps || 0;
    ctx.fillStyle = "#ffffff";
    ctx.font = "12px Inter, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(`Current: ${formatSpeed(currentSpeed)}`, padding + 5, padding + 15);
  }

  function formatSpeed(bps: number): string {
    const mbps = bps / 1_000_000;
    if (mbps >= 1) {
      return `${mbps.toFixed(1)} MB/s`;
    }
    const kbps = bps / 1_000;
    return `${kbps.toFixed(0)} KB/s`;
  }

  return (
    <div class="mt-4 p-4 bg-[#0f0f0f] rounded-lg border border-[#1e1e1e]">
      <h3 class="text-xs text-[#a0a0a0] mb-2">Transfer Speed</h3>
      <canvas
        ref={canvasRef}
        width={400}
        height={150}
        class="w-full"
        style={{ "image-rendering": "crisp-edges" }}
      />
    </div>
  );
}
