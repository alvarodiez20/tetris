// App entry: load the WASM engine, run a frame loop that feeds real `dt` into
// the deterministic core, wire up input backends, and refresh the UI.

import "./style.css";
import init, { WasmGame } from "./wasm-pkg/tetris_wasm";
import { OpponentRenderer, Renderer } from "./render";
import {
  CANCELLED,
  createRoom,
  describeError,
  joinRoom,
  normalizeCode,
  ROOM_CODE_RE,
  type Link,
  type Msg,
  type Room,
  type Snapshot,
} from "./net";
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

  const oppRenderer = new OpponentRenderer($<HTMLCanvasElement>("opp-board"));

  let game = new WasmGame(BigInt(Date.now()));
  let paused = false;
  let soloOverlayShown = false; // Game Over (or a custom end screen) is up

  // --- Versus (online 2-player) state ---
  // solo: normal game · countdown: 3-2-1 before a match · playing: live match ·
  // over: match decided, waiting for a rematch or for someone to leave.
  type Phase = "solo" | "countdown" | "playing" | "over";
  const COUNTDOWN_MS = 3000;
  const SNAPSHOT_INTERVAL_MS = 50; // at most 20 state updates per second
  let phase: Phase = "solo";
  let link: Link | null = null;
  let role: "host" | "guest" = "host";
  let helloTimer: number | undefined;
  let matchSeed = "";
  let room: Room | null = null;
  let countdownEnd = 0;
  let myReady = false;
  let oppReady = false;
  let oppSnap: Snapshot | null = null;
  let lastSentJson = "";
  let lastSentAt = 0;

  // --- AI player ---
  type AiMode = "off" | "smart" | "random";
  let aiMode: AiMode = "off";
  let aiSpeed = 9; // 1 (slow) .. 20 (fast)
  let aiPlan: number[] = [];
  let aiAccum = 0;

  const aiModeSelect = $<HTMLSelectElement>("ai-mode");
  const aiSpeedInput = $<HTMLInputElement>("ai-speed");
  aiModeSelect.addEventListener("change", () => {
    aiMode = aiModeSelect.value as AiMode;
    aiPlan = [];
    aiAccum = 0;
    if (aiMode !== "off") game.setSoftDrop(false);
  });
  aiSpeedInput.addEventListener("input", () => {
    aiSpeed = Number(aiSpeedInput.value);
  });

  const handlers: InputHandlers = {
    onAction: (action) => {
      if (!canControl()) return;
      game.input(action);
    },
    onSoftDrop: (held) => {
      if (!canControl()) return;
      game.setSoftDrop(held);
    },
    onPauseToggle: () => togglePause(),
  };

  /** Human input is live: not paused/finished, not bot-driven, not mid-countdown. */
  function canControl(): boolean {
    if (paused || game.isDone() || aiMode !== "off") return false;
    return phase === "solo" || phase === "playing";
  }

  // Build a random (but progress-guaranteed) plan: rotate, shuffle sideways, drop.
  function randomPlan(): number[] {
    const plan: number[] = [];
    const rotations = Math.floor(Math.random() * 4);
    for (let i = 0; i < rotations; i++) plan.push(4); // RotateCW
    const shift = Math.floor(Math.random() * 9) - 4;
    for (let i = 0; i < Math.abs(shift); i++) plan.push(shift < 0 ? 0 : 1);
    plan.push(3); // HardDrop
    return plan;
  }

  function stepAi(dt: number) {
    if (aiMode === "off" || paused || game.isDone()) return;
    aiAccum += dt;
    const interval = Math.max(18, 480 / aiSpeed);
    while (aiAccum >= interval) {
      aiAccum -= interval;
      if (aiPlan.length === 0) {
        aiPlan = aiMode === "smart" ? Array.from(game.botPlan()) : randomPlan();
      }
      const action = aiPlan.shift();
      if (action !== undefined) game.input(action);
      if (game.isDone()) break;
    }
  }

  attachKeyboard(handlers);
  const touchControls = $<HTMLElement>("touch-controls");
  if (isTouchDevice()) {
    touchControls.classList.remove("hidden");
    attachButtons(touchControls, handlers);
  }

  // UI buttons.
  $("pause-btn").addEventListener("click", () => togglePause());
  $("restart-btn").addEventListener("click", () => (link ? requestRematch() : restart()));

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
    if (game.isDone() || phase !== "solo") return; // a live match can't be paused
    paused = !paused;
    $("pause-btn").textContent = paused ? "Resume" : "Pause";
  }

  function restart() {
    game = new WasmGame(BigInt(Date.now()));
    paused = false;
    soloOverlayShown = false;
    aiPlan = [];
    aiAccum = 0;
    $("overlay").classList.add("hidden");
    $("pause-btn").textContent = "Pause";
  }

  // ---------------------------------------------------------------------------
  // Versus: lobby, protocol and match flow
  // ---------------------------------------------------------------------------

  const versusMenu = $("versus");
  const statusEl = $("versus-status");
  const setStatus = (text: string, isError = false) => {
    statusEl.textContent = text;
    statusEl.classList.toggle("error", isError);
  };
  const refit = () =>
    requestAnimationFrame(() => {
      renderer.resize();
      oppRenderer.resize();
    });

  function openMenu() {
    if (link) return;
    resumeAfterMenu = !paused && !game.isDone() && phase === "solo";
    if (resumeAfterMenu) togglePause();
    versusMenu.classList.remove("hidden");
  }
  let resumeAfterMenu = false;
  function closeMenu() {
    versusMenu.classList.add("hidden");
    if (resumeAfterMenu && paused && !link) togglePause();
    resumeAfterMenu = false;
  }

  function resetLobby() {
    room?.cancel();
    room = null;
    $("host-idle").classList.remove("hidden");
    $("host-room").classList.add("hidden");
    $<HTMLButtonElement>("host-btn").disabled = false;
    $("copy-link-btn").textContent = "Copy invite link";
    $<HTMLButtonElement>("join-btn").disabled = false;
    setStatus("");
  }

  function setVersusUi(on: boolean) {
    document.body.classList.toggle("versus", on);
    $("opp-panel").classList.toggle("hidden", !on);
    $("leave-btn").classList.toggle("hidden", !on);
    $("versus-btn").textContent = on ? "✕ Leave" : "⚔ Versus";
    refit();
  }

  function showOverlay(title: string, sub = "", button = "Rematch") {
    $("overlay-title").textContent = title;
    const subEl = $("overlay-sub");
    subEl.textContent = sub;
    subEl.classList.toggle("hidden", sub === "");
    const btn = $<HTMLButtonElement>("restart-btn");
    btn.textContent = button;
    btn.disabled = false;
    $("overlay").classList.remove("hidden");
  }

  /** A connection to the opponent is up: take over the UI and (host) start. */
  function attach(l: Link, asRole: "host" | "guest") {
    link = l;
    role = asRole;
    room = null;
    aiMode = "off";
    aiModeSelect.value = "off";
    aiPlan = [];
    versusMenu.classList.add("hidden");
    resumeAfterMenu = false;
    resetLobby();
    setVersusUi(true);
    l.onMessage(handleMessage);
    l.onClose(() => {
      if (link !== l) return;
      endVersus("Opponent left", phase === "over" ? "" : "You win by forfeit.");
    });
    showOverlay("Connected", role === "host" ? "Starting…" : "Waiting for the host…", "Waiting…");
    $<HTMLButtonElement>("restart-btn").disabled = true;
    if (role === "guest") {
      // The guest speaks first and the host answers with `start`. A message sent
      // by the host the instant the channel opens can be dropped by the browser,
      // so keep saying hello until the host's `start` arrives.
      const hello = () => link?.send({ t: "hello" });
      hello();
      helloTimer = window.setInterval(hello, 1000);
    }
  }

  function hostStartMatch() {
    // 52 random bits: safe as a JS number and a valid u64 for the engine.
    const bits = crypto.getRandomValues(new Uint32Array(2));
    const seed = (BigInt(bits[0] & 0xfffff) << 32n) | BigInt(bits[1]);
    matchSeed = seed.toString();
    link?.send({ t: "start", seed: matchSeed });
    beginMatch(seed);
  }

  function beginMatch(seed: bigint) {
    clearInterval(helloTimer);
    game = new WasmGame(seed);
    paused = false;
    myReady = false;
    oppReady = false;
    oppSnap = null;
    lastSentJson = "";
    lastSentAt = 0;
    phase = "countdown";
    countdownEnd = performance.now() + COUNTDOWN_MS;
    $("overlay").classList.add("hidden");
    $("countdown").classList.remove("hidden");
  }

  function handleMessage(msg: Msg) {
    switch (msg.t) {
      case "hello":
        // Guest is ready: start the first match, or repeat a `start` it missed.
        if (role !== "host") break;
        if (phase === "solo") hostStartMatch();
        else if (phase === "countdown") link?.send({ t: "start", seed: matchSeed });
        break;
      case "start":
        // Only the host decides when a match starts, and never mid-match.
        if (role === "guest" && phase !== "countdown" && phase !== "playing") {
          beginMatch(BigInt.asUintN(64, BigInt(msg.seed)));
        }
        break;
      case "state":
        oppSnap = msg.d;
        break;
      case "garbage":
        if (phase === "playing" && !game.isDone()) game.receiveGarbage(msg.n);
        break;
      case "dead":
        if (phase === "playing") finishMatch(true);
        break;
      case "ready":
        if (phase !== "over") break;
        oppReady = true;
        if (role === "host" && myReady) hostStartMatch();
        else if (!myReady) $("overlay-sub").textContent = "Opponent wants a rematch.";
        break;
    }
  }

  function finishMatch(won: boolean) {
    phase = "over";
    game.setSoftDrop(false);
    $("countdown").classList.add("hidden");
    showOverlay(won ? "You win!" : "You lose", oppReady ? "Opponent wants a rematch." : "");
    if (!won) {
      sendSnapshot(true); // let them see the final board
      link?.send({ t: "dead" });
    }
  }

  function requestRematch() {
    if (phase !== "over" || myReady) return;
    myReady = true;
    link?.send({ t: "ready" });
    $<HTMLButtonElement>("restart-btn").disabled = true;
    $("overlay-sub").classList.remove("hidden");
    $("overlay-sub").textContent =
      role === "host" && !oppReady ? "Waiting for the opponent…" : "Waiting for the host…";
    if (role === "host" && oppReady) hostStartMatch();
  }

  /** Tear down the connection and go back to solo play. */
  function endVersus(title?: string, sub = "") {
    clearInterval(helloTimer);
    const l = link;
    link = null;
    l?.close();
    phase = "solo";
    oppSnap = null;
    $("countdown").classList.add("hidden");
    setVersusUi(false);
    if (title) {
      soloOverlayShown = true;
      showOverlay(title, sub, "Play solo");
    } else {
      restart();
    }
  }

  function buildSnapshot(): Snapshot {
    const cells = game.visibleBoard();
    let b = "";
    for (let i = 0; i < cells.length; i++) b += cells[i];
    return {
      b,
      a: Array.from(game.activeCells()),
      k: game.activeKind(),
      score: game.score(),
      lines: game.lines(),
      level: game.level(),
      p: game.pendingGarbage(),
    };
  }

  function sendSnapshot(force = false) {
    if (!link) return;
    const now = performance.now();
    if (!force && now - lastSentAt < SNAPSHOT_INTERVAL_MS) return;
    const d = buildSnapshot();
    const json = JSON.stringify(d);
    if (!force && json === lastSentJson) return;
    lastSentJson = json;
    lastSentAt = now;
    link.send({ t: "state", d });
  }

  // Lobby UI.
  $("versus-btn").addEventListener("click", () => (link ? endVersus() : openMenu()));
  $("versus-close").addEventListener("click", () => {
    resetLobby();
    closeMenu();
  });
  $("leave-btn").addEventListener("click", () => endVersus());

  $("host-btn").addEventListener("click", async () => {
    $<HTMLButtonElement>("host-btn").disabled = true;
    setStatus("Creating room…");
    try {
      const r = await createRoom();
      room = r;
      $("room-code").textContent = r.code;
      $("host-idle").classList.add("hidden");
      $("host-room").classList.remove("hidden");
      setStatus("Waiting for your opponent to join…");
      const l = await r.guest;
      if (room === r) attach(l, "host");
      else l.close();
    } catch (err) {
      if ((err as Error)?.message === CANCELLED) return;
      if (room) {
        resetLobby();
        setStatus(describeError(err), true);
      } else if (!versusMenu.classList.contains("hidden")) {
        setStatus(describeError(err), true);
      }
    } finally {
      $<HTMLButtonElement>("host-btn").disabled = false;
    }
  });

  $("copy-link-btn").addEventListener("click", async () => {
    const url = new URL(location.href);
    url.search = `?room=${$("room-code").textContent}`;
    const btn = $("copy-link-btn");
    try {
      await navigator.clipboard.writeText(url.toString());
      btn.textContent = "Copied!";
    } catch {
      btn.textContent = url.toString(); // clipboard blocked: show it to copy by hand
    }
  });

  async function joinWithCode(raw: string) {
    const code = normalizeCode(raw);
    if (!ROOM_CODE_RE.test(code)) {
      setStatus("Enter the 5-character room code.", true);
      return;
    }
    $<HTMLButtonElement>("join-btn").disabled = true;
    setStatus("Connecting…");
    try {
      attach(await joinRoom(code), "guest");
    } catch (err) {
      setStatus(describeError(err), true);
    } finally {
      $<HTMLButtonElement>("join-btn").disabled = false;
    }
  }
  $("join-form").addEventListener("submit", (e) => {
    e.preventDefault();
    void joinWithCode($<HTMLInputElement>("join-code").value);
  });

  // Invite links: ?room=CODE opens the lobby and joins straight away.
  const inviteCode = new URLSearchParams(location.search).get("room");
  if (inviteCode) {
    const url = new URL(location.href);
    url.searchParams.delete("room");
    history.replaceState(null, "", url);
    $<HTMLInputElement>("join-code").value = normalizeCode(inviteCode);
    openMenu();
    void joinWithCode(inviteCode);
  }

  // Responsive sizing.
  const ro = new ResizeObserver(() => {
    renderer.resize();
    oppRenderer.resize();
  });
  ro.observe(boardCanvas.parentElement!);
  ro.observe($("opp-board").parentElement!);
  renderer.resize();
  oppRenderer.resize();
  window.addEventListener("orientationchange", () => setTimeout(refit, 100));

  // Pause when the tab is hidden so the piece doesn't fall offscreen.
  document.addEventListener("visibilitychange", () => {
    if (document.hidden && !paused) togglePause(); // no-op during a match
  });

  // Main loop.
  let last = performance.now();
  function frame(now: number) {
    const dt = Math.min(now - last, 100); // clamp big gaps (tab switches)
    last = now;

    if (phase === "countdown") {
      const left = countdownEnd - now;
      if (left <= 0) {
        phase = "playing";
        $("countdown").classList.add("hidden");
      } else {
        $("countdown").textContent = String(Math.ceil(left / 1000));
      }
    } else if (phase === "playing") {
      game.tick(dt);
      const sent = game.takeGarbageSent();
      if (sent > 0) link?.send({ t: "garbage", n: sent });
      if (game.isDone()) finishMatch(false);
      else sendSnapshot();
    } else if (phase === "solo" && !paused && !game.isDone()) {
      stepAi(dt);
      game.tick(dt);
    }
    renderer.draw(game);

    $("score").textContent = String(game.score());
    $("lines").textContent = String(game.lines());
    $("level").textContent = String(game.level());

    if (link) {
      oppRenderer.draw(oppSnap);
      $("opp-score").textContent = String(oppSnap?.score ?? 0);
      $("opp-lines").textContent = String(oppSnap?.lines ?? 0);
    } else if (game.isDone() && !soloOverlayShown) {
      soloOverlayShown = true;
      $("overlay-title").textContent = "Game Over";
      $("overlay-sub").classList.add("hidden");
      $("overlay").classList.remove("hidden");
    }

    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  // Test hook for the end-to-end suite (only with ?debug in the URL).
  if (new URLSearchParams(location.search).has("debug")) {
    Object.defineProperty(window, "__tetris", {
      value: {
        get game() {
          return game;
        },
        get phase() {
          return phase;
        },
        get opponent() {
          return oppSnap;
        },
      },
    });
  }
}

main().catch((err) => {
  document.body.innerHTML = `<pre style="color:#f88;padding:1rem">Failed to start: ${err}</pre>`;
  console.error(err);
});
