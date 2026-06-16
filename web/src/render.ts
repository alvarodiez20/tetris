// Canvas rendering. The playfield is a fixed 10x20 grid; everything scales from
// the cell size, which we recompute on resize for crisp output at any DPI. The
// Hold/Next thumbnails switch to a compact horizontal layout on small screens.

import type { WasmGame } from "./wasm-pkg/tetris_wasm";

export const COLS = 10;
export const ROWS = 20;
const NEXT_COUNT = 3;

// Indexed by cell id (kind + 1): 0 empty, then I J L O S T Z.
const COLORS = [
  "#11131f", // 0 empty
  "#22d3ee", // I cyan
  "#3b82f6", // J blue
  "#f59e0b", // L orange
  "#fbbf24", // O yellow
  "#22c55e", // S green
  "#a855f7", // T purple
  "#ef4444", // Z red
];

// 4x4 spawn shapes for hold/next thumbnails (just the visual, kind-indexed).
const THUMBS: Record<number, [number, number][]> = {
  0: [[0, 1], [1, 1], [2, 1], [3, 1]], // I
  1: [[0, 0], [0, 1], [1, 1], [2, 1]], // J
  2: [[2, 0], [0, 1], [1, 1], [2, 1]], // L
  3: [[1, 0], [2, 0], [1, 1], [2, 1]], // O
  4: [[1, 0], [2, 0], [0, 1], [1, 1]], // S
  5: [[1, 0], [0, 1], [1, 1], [2, 1]], // T
  6: [[0, 0], [1, 0], [1, 1], [2, 1]], // Z
};

export class Renderer {
  private board: HTMLCanvasElement;
  private hold: HTMLCanvasElement;
  private next: HTMLCanvasElement;
  private cell = 24;
  private dpr = Math.max(1, window.devicePixelRatio || 1);

  private compact = false;
  private holdBox = 88;
  private nextMini = 80;

  constructor(board: HTMLCanvasElement, hold: HTMLCanvasElement, next: HTMLCanvasElement) {
    this.board = board;
    this.hold = hold;
    this.next = next;
  }

  /** Recompute sizes to fit the available CSS boxes, then size the buffers. */
  resize(): void {
    this.dpr = Math.max(1, window.devicePixelRatio || 1);
    this.compact = window.matchMedia("(max-width: 640px)").matches;

    // Playfield: largest cell that fits its wrapper.
    const wrap = this.board.parentElement!;
    this.cell = Math.max(8, Math.floor(Math.min(wrap.clientWidth / COLS, wrap.clientHeight / ROWS)));
    sizeCanvas(this.board, this.cell * COLS, this.cell * ROWS, this.dpr);

    // Hold + Next thumbnails.
    this.holdBox = this.compact ? 50 : 88;
    this.nextMini = this.compact ? 46 : 80;
    sizeCanvas(this.hold, this.holdBox, this.holdBox, this.dpr);
    if (this.compact) {
      sizeCanvas(this.next, this.nextMini * NEXT_COUNT, this.nextMini, this.dpr);
    } else {
      sizeCanvas(this.next, this.nextMini, this.nextMini * NEXT_COUNT, this.dpr);
    }
  }

  draw(game: WasmGame): void {
    this.drawBoard(game);
    this.drawHold(game);
    this.drawNext(game);
  }

  private drawBoard(game: WasmGame): void {
    const ctx = this.board.getContext("2d")!;
    const c = this.cell;
    ctx.save();
    ctx.scale(this.dpr, this.dpr);
    ctx.clearRect(0, 0, c * COLS, c * ROWS);

    // Grid background.
    ctx.fillStyle = COLORS[0];
    ctx.fillRect(0, 0, c * COLS, c * ROWS);
    ctx.strokeStyle = "rgba(255,255,255,0.04)";
    for (let x = 1; x < COLS; x++) line(ctx, x * c, 0, x * c, ROWS * c);
    for (let y = 1; y < ROWS; y++) line(ctx, 0, y * c, COLS * c, y * c);

    // Locked cells.
    const cells = game.visibleBoard();
    for (let i = 0; i < cells.length; i++) {
      const id = cells[i];
      if (id === 0) continue;
      this.cellRect(ctx, i % COLS, Math.floor(i / COLS), COLORS[id]);
    }

    // Ghost piece.
    const ghost = game.ghostCells();
    const color = COLORS[game.activeKind() + 1];
    for (let i = 0; i < ghost.length; i += 2) {
      const y = ghost[i + 1];
      if (y < 0) continue;
      ctx.globalAlpha = 0.25;
      this.cellRect(ctx, ghost[i], y, color);
      ctx.globalAlpha = 1;
    }

    // Active piece.
    const active = game.activeCells();
    for (let i = 0; i < active.length; i += 2) {
      const y = active[i + 1];
      if (y < 0) continue;
      this.cellRect(ctx, active[i], y, color);
    }
    ctx.restore();
  }

  private cellRect(ctx: CanvasRenderingContext2D, x: number, y: number, color: string): void {
    const c = this.cell;
    ctx.fillStyle = color;
    ctx.fillRect(x * c + 1, y * c + 1, c - 2, c - 2);
    ctx.fillStyle = "rgba(255,255,255,0.18)";
    ctx.fillRect(x * c + 1, y * c + 1, c - 2, Math.max(2, c * 0.16));
  }

  private drawHold(game: WasmGame): void {
    const ctx = this.hold.getContext("2d")!;
    ctx.save();
    ctx.scale(this.dpr, this.dpr);
    ctx.clearRect(0, 0, this.holdBox, this.holdBox);
    const kind = game.hold();
    if (kind >= 0) drawThumb(ctx, kind, 0, 0, this.holdBox);
    ctx.restore();
  }

  private drawNext(game: WasmGame): void {
    const ctx = this.next.getContext("2d")!;
    const m = this.nextMini;
    ctx.save();
    ctx.scale(this.dpr, this.dpr);
    ctx.clearRect(0, 0, this.compact ? m * NEXT_COUNT : m, this.compact ? m : m * NEXT_COUNT);
    const upcoming = game.preview(NEXT_COUNT);
    for (let i = 0; i < upcoming.length; i++) {
      const ox = this.compact ? i * m : 0;
      const oy = this.compact ? 0 : i * m;
      drawThumb(ctx, upcoming[i], ox, oy, m);
    }
    ctx.restore();
  }
}

function drawThumb(
  ctx: CanvasRenderingContext2D,
  kind: number,
  ox: number,
  oy: number,
  box: number,
): void {
  const shape = THUMBS[kind];
  if (!shape) return;
  const cell = box / 5;
  const pad = (box - cell * 4) / 2;
  ctx.fillStyle = COLORS[kind + 1];
  for (const [x, y] of shape) {
    ctx.fillRect(ox + pad + x * cell + 1, oy + pad + y * cell + 1, cell - 2, cell - 2);
  }
}

function sizeCanvas(canvas: HTMLCanvasElement, w: number, h: number, dpr: number): void {
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);
}

function line(ctx: CanvasRenderingContext2D, x0: number, y0: number, x1: number, y1: number): void {
  ctx.beginPath();
  ctx.moveTo(x0, y0);
  ctx.lineTo(x1, y1);
  ctx.stroke();
}
