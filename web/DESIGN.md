# C0RE dashboard: design notes (0.2, "terminal")

## Direction

The owner pointed at a hacker-club site as the look to follow: monochrome, square edges,
condensed poster type over monospace, terminal vocabulary (`./keys`, `[ new key ]`,
`cat ./usage >>`), a ticker strip and scanlines. The fonts, colours and effects
were measured from that site with the clone-ui skill; only the visual language is taken,
none of its content or branding. Accent is red instead of their green.

## Rules

- **Two colours plus red.** Ink `#F2F2F2` on `#070707`; panels `#0E0E0E` / `#161616`;
  lines `#303030`. Red `#FF3D3D` only for: the brand's 0 (engine online), blocked
  requests, errors, offline, CPU spill, the access-log light. Never decoration.
- **No rounded corners anywhere.** Buttons have a hard 4px offset shadow (`#303030`)
  and press into it.
- **Type:** Anton for headlines and big numbers; Fragment Mono for everything you read;
  VT323 for machine output (boot log, gauge, ticker figures). Self-hosted latin subsets.
- **Labels:** 10–11px uppercase with wide tracking, like silkscreen on hardware.
- **Panels can carry a tab** (`data-tab="TRAFFIC_TODAY"`) naming what's inside.

## Signature element

The engine as a **pixel ring**: each cell is a share of the loaded model. White cells sit
on the GPU, red cells spilled onto the CPU, dim when idle, sparse red when offline. It
lights cell by cell once when Overview opens. Beside it, a boot log written from the real
status (`> gpu share ...... 100%`) and ENGINE ONLINE in scanlined Anton with the second
word outlined (red outline when offline).

## Pages

Top system bar (gateway, engine state, public address), sticky `./` nav with the active
page as a lit block, numbered sections (`[01] TODAY`, `[02] ENTRANCES`, `[03] LAST 24
HOURS`, `[04] LIVE REQUESTS`) with a dashed rule and a shell-command link. Keys are rows
with show/hide/copy; a shown key hides again after a minute. Phones: the nav scrolls
sideways, the gauge sits above the boot log.

Motion: the ring lighting up, boot lines appearing, the ticker (pauses on hover), the
cursor and the log light blinking. All of it stops for reduced-motion users.
