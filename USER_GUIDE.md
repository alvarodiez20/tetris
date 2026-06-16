# Tetris — User Guide

A quick guide to playing. The same content is available in-game via the **？ Help**
button in the top bar.

## Goal

Pieces (tetrominoes) fall from the top of a 10×20 well. Move and rotate them to
fill complete horizontal lines. Each full line **clears** and scores points. The
game ends when a newly spawned piece has no room — the stack has reached the top.

## The pieces

There are seven tetrominoes, each a different color:

| Piece | Color | Shape |
| ----- | ----- | ----- |
| I | Cyan | ▭▭▭▭ |
| O | Yellow | 2×2 square |
| T | Purple | T-shape |
| S | Green | S-shape |
| Z | Red | Z-shape |
| J | Blue | J-shape |
| L | Orange | L-shape |

Pieces are drawn from a **7-bag**: every group of seven contains exactly one of
each, so you never wait too long for the piece you need.

## Controls

### Desktop (keyboard)

| Key | Action |
| --- | ------ |
| ← / → &nbsp;or&nbsp; A / D | Move left / right |
| ↓ &nbsp;or&nbsp; S | Soft drop (descend faster) |
| Space | Hard drop (instant drop + lock) |
| ↑ &nbsp;or&nbsp; X | Rotate clockwise |
| Z &nbsp;or&nbsp; Ctrl | Rotate counter-clockwise |
| C &nbsp;or&nbsp; Shift | Hold piece |
| P &nbsp;or&nbsp; Esc | Pause / resume |

### Mobile (touch)

On touch devices, on-screen controls appear automatically:

- **Left pad** — ◀ ▼ ▶ to move and soft-drop.
- **Right pad** — ↺ ↻ to rotate, ⇄ to hold, and **DROP** for a hard drop.

## Key mechanics

- **Ghost piece** — a faint outline shows exactly where the current piece will
  land, so you can line up placements.
- **Hold** — stash the current piece to swap in later. You can hold once per
  piece (until the next one locks).
- **Lock delay** — a piece doesn't lock instantly when it lands; you have a brief
  moment to slide or rotate it. Moving or rotating refreshes the timer (up to a
  limit).
- **Rotation (SRS)** — uses the Super Rotation System with **wall kicks**: if a
  rotation is blocked, the game nudges the piece off walls or the floor to make
  it fit. This is what makes **T-spins** possible.

## Scoring

Base points scale with your current **level**:

| Clear | Points (× level) |
| ----- | ---------------- |
| Single (1 line) | 100 |
| Double (2 lines) | 300 |
| Triple (3 lines) | 500 |
| **Tetris** (4 lines) | 800 |
| T-spin (no lines) | 400 |
| T-spin single | 800 |
| T-spin double | 1200 |
| T-spin triple | 1600 |

Bonuses:

- **Back-to-back** — chaining "difficult" clears (Tetrises and T-spin line
  clears) without an ordinary line clear in between adds **+50%**.
- **Combo** — clearing lines on consecutive pieces adds `50 × combo × level`.
- **Drops** — soft drop scores 1 per cell, hard drop 2 per cell.

## Levels & speed

You level up every **10 lines cleared**. Each level increases gravity, so pieces
fall faster — the core challenge ramp.

## Tips

- Keep the stack low and flat; build toward a Tetris by leaving one column open
  for an I-piece.
- Use the ghost piece to set up T-spins by tucking the T under an overhang and
  rotating into the gap.
- Don't waste **Hold** — stashing an early I-piece for a Tetris is a classic play.
