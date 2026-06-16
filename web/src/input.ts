// Input abstraction: the game logic only ever sees `Action` codes, so keyboard,
// on-screen buttons, and swipe gestures are interchangeable backends.

// Must match the Rust `Action` enum discriminants.
export const Action = {
  MoveLeft: 0,
  MoveRight: 1,
  SoftDrop: 2,
  HardDrop: 3,
  RotateCW: 4,
  RotateCCW: 5,
  Hold: 6,
} as const;

export type ActionName = keyof typeof Action;

export interface InputHandlers {
  /** Fire a one-shot action. */
  onAction: (action: number) => void;
  /** Soft-drop is a held state (affects gravity), so it has press/release. */
  onSoftDrop: (held: boolean) => void;
  onPauseToggle: () => void;
}

/** Detect a touch-first device so we can default to on-screen buttons. */
export function isTouchDevice(): boolean {
  return (
    "ontouchstart" in window ||
    navigator.maxTouchPoints > 0 ||
    window.matchMedia("(pointer: coarse)").matches
  );
}

// --- Keyboard (PC) with simple DAS/ARR auto-repeat for horizontal movement ---

const DAS_MS = 150; // delay before auto-repeat
const ARR_MS = 40; // auto-repeat interval

export function attachKeyboard(h: InputHandlers): () => void {
  let repeatDir = 0; // -1 left, +1 right, 0 none
  let dasTimer: number | null = null;
  let arrTimer: number | null = null;

  const clearRepeat = () => {
    if (dasTimer !== null) clearTimeout(dasTimer);
    if (arrTimer !== null) clearInterval(arrTimer);
    dasTimer = arrTimer = null;
  };

  const startRepeat = (dir: number, action: number) => {
    if (repeatDir === dir) return;
    clearRepeat();
    repeatDir = dir;
    h.onAction(action);
    dasTimer = window.setTimeout(() => {
      arrTimer = window.setInterval(() => h.onAction(action), ARR_MS);
    }, DAS_MS);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.repeat) return;
    switch (e.key) {
      case "ArrowLeft":
      case "a":
        startRepeat(-1, Action.MoveLeft);
        break;
      case "ArrowRight":
      case "d":
        startRepeat(1, Action.MoveRight);
        break;
      case "ArrowDown":
      case "s":
        h.onSoftDrop(true);
        break;
      case "ArrowUp":
      case "x":
        h.onAction(Action.RotateCW);
        break;
      case "z":
      case "Control":
        h.onAction(Action.RotateCCW);
        break;
      case " ":
        e.preventDefault();
        h.onAction(Action.HardDrop);
        break;
      case "c":
      case "Shift":
        h.onAction(Action.Hold);
        break;
      case "p":
      case "Escape":
        h.onPauseToggle();
        break;
    }
  };

  const onKeyUp = (e: KeyboardEvent) => {
    switch (e.key) {
      case "ArrowLeft":
      case "a":
        if (repeatDir === -1) {
          clearRepeat();
          repeatDir = 0;
        }
        break;
      case "ArrowRight":
      case "d":
        if (repeatDir === 1) {
          clearRepeat();
          repeatDir = 0;
        }
        break;
      case "ArrowDown":
      case "s":
        h.onSoftDrop(false);
        break;
    }
  };

  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
  return () => {
    clearRepeat();
    window.removeEventListener("keydown", onKeyDown);
    window.removeEventListener("keyup", onKeyUp);
  };
}

// --- On-screen buttons (mobile default) --------------------------------------

export function attachButtons(root: HTMLElement, h: InputHandlers): () => void {
  const cleanups: Array<() => void> = [];

  root.querySelectorAll<HTMLButtonElement>(".tbtn").forEach((btn) => {
    const name = btn.dataset.action as ActionName | undefined;
    if (!name) return;
    const code = Action[name];

    if (name === "SoftDrop") {
      const down = (e: Event) => {
        e.preventDefault();
        h.onSoftDrop(true);
      };
      const up = (e: Event) => {
        e.preventDefault();
        h.onSoftDrop(false);
      };
      btn.addEventListener("pointerdown", down);
      btn.addEventListener("pointerup", up);
      btn.addEventListener("pointerleave", up);
      btn.addEventListener("pointercancel", up);
      cleanups.push(() => {
        btn.removeEventListener("pointerdown", down);
        btn.removeEventListener("pointerup", up);
        btn.removeEventListener("pointerleave", up);
        btn.removeEventListener("pointercancel", up);
      });
    } else {
      // Auto-repeat horizontal moves while held; one-shot for rotate/hold/drop.
      const repeats = name === "MoveLeft" || name === "MoveRight";
      let das: number | null = null;
      let arr: number | null = null;
      const stop = () => {
        if (das !== null) clearTimeout(das);
        if (arr !== null) clearInterval(arr);
        das = arr = null;
      };
      const down = (e: Event) => {
        e.preventDefault();
        h.onAction(code);
        if (repeats) {
          das = window.setTimeout(() => {
            arr = window.setInterval(() => h.onAction(code), ARR_MS);
          }, DAS_MS);
        }
      };
      btn.addEventListener("pointerdown", down);
      btn.addEventListener("pointerup", stop);
      btn.addEventListener("pointerleave", stop);
      btn.addEventListener("pointercancel", stop);
      cleanups.push(() => {
        stop();
        btn.removeEventListener("pointerdown", down);
        btn.removeEventListener("pointerup", stop);
        btn.removeEventListener("pointerleave", stop);
        btn.removeEventListener("pointercancel", stop);
      });
    }
  });

  return () => cleanups.forEach((c) => c());
}
