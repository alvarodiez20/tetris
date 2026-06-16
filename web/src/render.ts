// Canvas rendering. The playfield is a fixed 10x20 grid; everything scales from
// the cell size, which we recompute on resize for crisp output at any DPI.

import type { WasmGame } from "./wasm-pkg/tetris_wasm";

export const COLS = 10;
export const ROWS = 20;

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

  constructor(board: HTMLCanvasElement, hold: HTMLCanvasElement, next: HTMLCanvasElement) {
    this.board = board;
    this.hold = hold;
    this.next = next;
  }

  /** Recompute the cell size to fit the available CSS box, then size buffers. */
  resize(): void {
    this.dpr = Math.max(1, window.devicePixelRatio || 1);
    const wrap = this.board.parentElement!;
    const availW = wrap.clientWidth;
    const availH = wrap.clientHeight;
    this.cell = Math.floor(Math.min(availW / COLS, availH / ROWS));
    this.cell = Math.max(8, this.cell);

    const w = this.cell * COLS;
    const h = this.cell * ROWS;
    sizeCanvas(this.board, w, h, this.dpr);
    sizeCanvas(this.hold, 96, 96, this.dpr);
    sizeCanvas(this.next, 96, 96 * 3, this.dpr);
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
    for (let i = 0; i < ghost.length; i += 2) {
      const x = ghost[i];
      const y = ghost[i + 1];
      if (y < 0) continue;
      ctx.globalAlpha = 0.25;
      this.cellRect(ctx, x, y, COLORS[game.activeKind() + 1]);
      ctx.globalAlpha = 1;
    }

    // Active piece.
    const active = game.activeCells();
    for (let i = 0; i < active.length; i += 2) {
      const x = active[i];
      const y = active[i + 1];
      if (y < 0) continue;
      this.cellRect(ctx, x, y, COLORS[game.activeKind() + 1]);
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
    ctx.clearRect(0, 0, 96, 96);
    const kind = game.hold();
    if (kind >= 0) drawThumb(ctx, kind, 0, 96);
    ctx.restore();
  }

  private drawNext(game: WasmGame): void {
    const ctx = this.next.getContext("2d")!;
    ctx.save();
    ctx.scale(this.dpr, this.dpr);
    ctx.clearRect(0, 0, 96, 288);
    const upcoming = game.preview(3);
    for (let i = 0; i < upcoming.length; i++) {
      drawThumb(ctx, upcoming[i], i * 96, 96);
    }
    ctx.restore();
  }
}

function drawThumb(ctx: CanvasRenderingContext2D, kind: number, offsetY: number, box: number): void {
  const shape = THUMBS[kind];
  if (!shape) return;
  const cell = box / 5;
  const pad = (box - cell * 4) / 2;
  for (const [x, y] of shape) {
    ctx.fillStyle = COLORS[kind + 1];
    ctx.fillRect(pad + x * cell + 1, offsetY + pad + y * cell + 1, cell - 2, cell - 2);
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
