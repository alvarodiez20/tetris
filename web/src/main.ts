// App entry: load the WASM engine, run a frame loop that feeds real `dt` into
// the deterministic core, wire up input backends, and refresh the UI.

import "./style.css";
import init, { WasmGame } from "./wasm-pkg/tetris_wasm";
import { Renderer } from "./render";
import {
  attachButtons,
  attachKeyboard,
  isTouchDevice,
  type InputHandlers,
} from "./input";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

async function main() {
  await init();

  const boardCanvas = $<HTMLCanvasElement>("board");
  const holdCanvas = $<HTMLCanvasElement>("hold");
  const nextCanvas = $<HTMLCanvasElement>("next");
  const renderer = new Renderer(boardCanvas, holdCanvas, nextCanvas);

  let game = new WasmGame(BigInt(Date.now()));
  let paused = false;

  const handlers: InputHandlers = {
    onAction: (action) => {
      if (paused || game.isDone()) return;
      game.input(action);
    },
    onSoftDrop: (held) => {
      if (paused || game.isDone()) return;
      game.setSoftDrop(held);
    },
    onPauseToggle: () => togglePause(),
  };

  attachKeyboard(handlers);
  const touchControls = $<HTMLElement>("touch-controls");
  if (isTouchDevice()) {
    touchControls.classList.remove("hidden");
    attachButtons(touchControls, handlers);
  }

  // UI buttons.
  $("pause-btn").addEventListener("click", () => togglePause());
  $("restart-btn").addEventListener("click", () => restart());

  // Help / user guide overlay. Opening it pauses play; closing resumes only if
  // the game wasn't already paused by the player.
  let resumeAfterHelp = false;
  const help = $("help");
  $("help-btn").addEventListener("click", () => {
    resumeAfterHelp = !paused && !game.isDone();
    if (resumeAfterHelp) togglePause();
    help.classList.remove("hidden");
  });
  $("help-close").addEventListener("click", () => {
    help.classList.add("hidden");
    if (resumeAfterHelp && paused) togglePause();
    resumeAfterHelp = false;
  });

  function togglePause() {
    if (game.isDone()) return;
    paused = !paused;
    $("pause-btn").textContent = paused ? "Resume" : "Pause";
  }

  function restart() {
    game = new WasmGame(BigInt(Date.now()));
    paused = false;
    $("overlay").classList.add("hidden");
    $("pause-btn").textContent = "Pause";
  }

  // Responsive sizing.
  const ro = new ResizeObserver(() => renderer.resize());
  ro.observe(boardCanvas.parentElement!);
  renderer.resize();
  window.addEventListener("orientationchange", () => setTimeout(() => renderer.resize(), 100));

  // Pause when the tab is hidden so the piece doesn't fall offscreen.
  document.addEventListener("visibilitychange", () => {
    if (document.hidden && !paused) togglePause();
  });

  // Main loop.
  let last = performance.now();
  function frame(now: number) {
    const dt = Math.min(now - last, 100); // clamp big gaps (tab switches)
    last = now;

    if (!paused && !game.isDone()) {
      game.tick(dt);
    }
    renderer.draw(game);

    $("score").textContent = String(game.score());
    $("lines").textContent = String(game.lines());
    $("level").textContent = String(game.level());

    if (game.isDone()) {
      $("overlay-title").textContent = "Game Over";
      $("overlay").classList.remove("hidden");
    }

    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

main().catch((err) => {
  document.body.innerHTML = `<pre style="color:#f88;padding:1rem">Failed to start: ${err}</pre>`;
  console.error(err);
});
