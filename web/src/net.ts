// Peer-to-peer transport for versus mode. Two browsers talk over a WebRTC data
// channel (via PeerJS); the public PeerJS signalling server is only used for the
// initial handshake, so no game server is needed and the site stays static.

import Peer, { type DataConnection, type PeerError, type PeerOptions } from "peerjs";

/** A compact picture of one player's field, streamed to the opponent. */
export interface Snapshot {
  /** Visible board as 200 digits, row-major (0 = empty, 8 = garbage). */
  b: string;
  /** Active piece cells, flat `[x0,y0, x1,y1, ...]` in visible coordinates. */
  a: number[];
  /** Active piece kind (0..6). */
  k: number;
  score: number;
  lines: number;
  level: number;
  /** Garbage lines queued to rise on this player's board. */
  p: number;
}

export type Msg =
  | { t: "hello" }
  | { t: "start"; seed: string }
  | { t: "state"; d: Snapshot }
  | { t: "garbage"; n: number }
  | { t: "dead" }
  | { t: "ready" };

const ID_PREFIX = "tetris-vs-";
const CODE_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no 0/O/1/I
const CODE_LENGTH = 5;
const CONNECT_TIMEOUT_MS = 15000;
/** Message of the rejection a cancelled room produces (not a real failure). */
export const CANCELLED = "cancelled";

export const ROOM_CODE_RE = new RegExp(`^[${CODE_ALPHABET}]{${CODE_LENGTH}}$`);

export function normalizeCode(raw: string): string {
  return raw.trim().toUpperCase().replace(/[^A-Z0-9]/g, "");
}

// --- message validation -------------------------------------------------------
// Everything from the other side is untrusted: validate before it reaches the
// renderer or the engine.

const int = (v: unknown, min: number, max: number): number | null =>
  typeof v === "number" && Number.isInteger(v) && v >= min && v <= max ? v : null;

function parseSnapshot(d: any): Snapshot | null {
  if (!d || typeof d !== "object") return null;
  if (typeof d.b !== "string" || !/^[0-8]{200}$/.test(d.b)) return null;
  if (!Array.isArray(d.a) || d.a.length !== 8) return null;
  if (!d.a.every((n: unknown) => int(n, -40, 40) !== null)) return null;
  const k = int(d.k, 0, 6);
  const score = int(d.score, 0, 1e9);
  const lines = int(d.lines, 0, 1e6);
  const level = int(d.level, 1, 1e6);
  const p = int(d.p, 0, 20);
  if (k === null || score === null || lines === null || level === null || p === null) return null;
  return { b: d.b, a: d.a, k, score, lines, level, p };
}

export function parseMsg(raw: unknown): Msg | null {
  if (!raw || typeof raw !== "object") return null;
  const m = raw as any;
  switch (m.t) {
    case "start":
      return typeof m.seed === "string" && /^\d{1,20}$/.test(m.seed)
        ? { t: "start", seed: m.seed }
        : null;
    case "state": {
      const d = parseSnapshot(m.d);
      return d ? { t: "state", d } : null;
    }
    case "garbage": {
      const n = int(m.n, 1, 20);
      return n === null ? null : { t: "garbage", n };
    }
    case "hello":
    case "dead":
    case "ready":
      return { t: m.t };
    default:
      return null;
  }
}

// --- connection ---------------------------------------------------------------

/** An open, ordered, reliable channel to the opponent. */
export class Link {
  private messageCb: ((m: Msg) => void) | null = null;
  private closeCb: () => void = () => {};
  private closed = false;
  /** Messages that arrived before anyone was listening. */
  private backlog: Msg[] = [];

  constructor(
    private peer: Peer,
    private conn: DataConnection,
  ) {
    conn.on("data", (raw) => {
      const msg = parseMsg(raw);
      if (!msg) return;
      if (this.messageCb) this.messageCb(msg);
      else this.backlog.push(msg);
    });
    const onClose = () => {
      if (this.closed) return;
      this.closed = true;
      this.closeCb();
      this.peer.destroy();
    };
    conn.on("close", onClose);
    conn.on("error", onClose);
    // PeerJS has no heartbeat of its own: if the other tab just vanishes, the ICE
    // layer is what tells us the connection is gone.
    conn.peerConnection?.addEventListener("connectionstatechange", () => {
      const s = conn.peerConnection.connectionState;
      if (s === "failed" || s === "closed" || s === "disconnected") onClose();
    });
  }

  send(msg: Msg): void {
    if (!this.closed && this.conn.open) this.conn.send(msg);
  }
  onMessage(cb: (m: Msg) => void): void {
    this.messageCb = cb;
    for (const m of this.backlog.splice(0)) cb(m);
  }
  onClose(cb: () => void): void {
    this.closeCb = cb;
  }
  close(): void {
    this.closed = true;
    this.conn.close();
    this.peer.destroy();
  }
}

export interface Room {
  code: string;
  /** Resolves when the opponent connects; rejects on failure. */
  guest: Promise<Link>;
  /** Stop waiting and release the room code. */
  cancel(): void;
}

/** Signalling server settings. Defaults to the public PeerJS cloud; the
 *  `peerHost`/`peerPort`/`peerPath`/`peerSecure` URL parameters point at your own
 *  `peerjs-server` (also used by the end-to-end tests). */
function peerOptions(): PeerOptions {
  const q = new URLSearchParams(location.search);
  const debug = Number(q.get("peerDebug") ?? 0) as 0 | 1 | 2 | 3;
  const host = q.get("peerHost");
  if (!host) return { debug };
  return {
    host,
    port: Number(q.get("peerPort") ?? 9000),
    path: q.get("peerPath") ?? "/",
    secure: q.get("peerSecure") === "1",
    debug,
  };
}

function randomCode(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(CODE_LENGTH));
  return Array.from(bytes, (b) => CODE_ALPHABET[b % CODE_ALPHABET.length]).join("");
}

function whenOpen(peer: Peer): Promise<void> {
  return new Promise((resolve, reject) => {
    peer.once("open", () => resolve());
    peer.once("error", reject);
  });
}

/** Turn a PeerJS error into something a player can act on. */
export function describeError(err: unknown): string {
  const type = (err as PeerError<string>)?.type;
  switch (type) {
    case "peer-unavailable":
      return "Room not found. Check the code and try again.";
    case "network":
    case "server-error":
    case "socket-error":
    case "socket-closed":
      return "Can't reach the matchmaking server. Check your connection.";
    case "browser-incompatible":
      return "This browser doesn't support online play.";
    default:
      return (err as Error)?.message || "Connection failed.";
  }
}

/** Open a room and wait for one opponent. Resolves once the code is live. */
export async function createRoom(): Promise<Room> {
  for (let attempt = 0; attempt < 5; attempt++) {
    const code = randomCode();
    const peer = new Peer(ID_PREFIX + code, peerOptions());
    try {
      await whenOpen(peer);
    } catch (err) {
      peer.destroy();
      if ((err as PeerError<string>).type === "unavailable-id") continue; // code taken
      throw err;
    }

    let cancelled = false;
    let rejectGuest: (err: Error) => void = () => {};
    const guest = new Promise<Link>((resolve, reject) => {
      rejectGuest = reject;
      let taken = false;
      peer.on("connection", (conn) => {
        if (taken) {
          conn.close(); // room is for exactly two players
          return;
        }
        taken = true;
        conn.on("open", () => resolve(new Link(peer, conn)));
      });
      peer.on("error", (err) => {
        if (!cancelled) reject(err);
      });
      // Lost the signalling server while waiting: reconnect so the code stays valid.
      peer.on("disconnected", () => {
        if (!cancelled && !taken && !peer.destroyed) peer.reconnect();
      });
    });
    guest.catch(() => {}); // a cancelled room is not an unhandled rejection
    return {
      code,
      guest,
      cancel: () => {
        cancelled = true;
        peer.destroy();
        rejectGuest(new Error(CANCELLED));
      },
    };
  }
  throw new Error("Couldn't allocate a room code. Try again.");
}

/** Connect to a host's room. Rejects if the room doesn't exist or can't be reached. */
export async function joinRoom(code: string): Promise<Link> {
  const peer = new Peer(peerOptions());
  try {
    await whenOpen(peer);
    const conn = peer.connect(ID_PREFIX + code, { reliable: true, serialization: "json" });
    // Listen from the very start so nothing the host sends can slip past us.
    const link = new Link(peer, conn);
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("Timed out connecting to the other player.")),
        CONNECT_TIMEOUT_MS,
      );
      const fail = (err: unknown) => {
        clearTimeout(timer);
        reject(err);
      };
      conn.on("open", () => {
        clearTimeout(timer);
        resolve();
      });
      conn.on("close", () => fail(new Error("The other player disconnected.")));
      conn.on("error", fail);
      peer.on("error", fail);
    });
    return link;
  } catch (err) {
    peer.destroy();
    throw err;
  }
}
