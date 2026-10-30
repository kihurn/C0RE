# C0RE dashboard — design plan

Reviewed against `.claude/skills/apple-design/SKILL.md` (HIG foundations + desktop
conventions; web app, so iOS-specific rules don't apply).

## Subject

The admin console of a home AI service. One person (the owner) uses it, mostly from a Mac
or phone over Tailscale, to answer three questions quickly: *Is my AI up? Who is using it?
Is anything getting through that shouldn't?* — and to hand out or revoke keys.

The most characteristic thing in this world is **the door**: C0RE is a doorkeeper with two
entrances (private and public) in front of one engine. The design is built around that.

## Signature element

**The core.** The "0" in C0RE is drawn as a ring: solid and lit when the engine is online,
hollow and grey when it is offline. On Overview it sits beside a small **two-door
schematic** — Private entrance and Public entrance, each with today's admitted and blocked
counts and its on/off state — feeding into the engine. It is the one place boldness is
spent; everything else is quiet system UI.

## Colour (semantic tokens, light / dark)

| Token | Light | Dark | Use | Contrast |
|---|---|---|---|---|
| `--bg` | `#F4F4F6` | `#121214` | page | — |
| `--surface` | `#FFFFFF` | `#1B1B1F` | panels, table rows | — |
| `--text` | `#1B1B1F` | `#ECECEF` | primary text | 16.9:1 / 15.1:1 on surface |
| `--text-2` | `#5C5D66` | `#A3A4AD` | secondary text | 6.5:1 / 7.2:1 on surface |
| `--accent` | `#5646C8` | `#A99CFF` | primary actions, the lit core | 7.3:1 / 8.0:1 on surface |
| `--ok` | `#1D7A45` | `#46D18A` | engine online, admitted | 5.4:1 / 9.0:1 |
| `--warn` | `#9A5800` | `#F2B24A` | limits, public on | 5.6:1 / 10.1:1 |
| `--danger` | `#B42318` | `#FF7A6B` | blocked, delete | 6.5:1 / 6.9:1 |
| `--line` | `rgba(27,27,31,.10)` | `rgba(236,236,239,.10)` | separators | — |

Why violet: it is not a status colour, so it never competes with green/amber/red, and it
avoids the three generated-UI defaults (cream + terracotta, black + acid green, broadsheet).
Status is never shown by colour alone: every status also has a word ("Online", "Blocked").

## Type

System UI stack (`-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI",
system-ui, sans-serif`) for everything — this is a utility, navigation and controls should
feel native. Numbers use `font-variant-numeric: tabular-nums`. A monospace stack
(`ui-monospace, "SF Mono", "Cascadia Mono", Consolas, monospace`) only for key ids and the
one-time secret, because people compare and copy them character by character.

Scale (desktop; HIG default 13 pt, minimum 10): page title 22/28 semibold · section 15/20
semibold · body 13/18 · caption 12/16. Nothing smaller than 12 px.

## Layout

Sidebar (one level: Overview, Keys, Usage, Models, Settings) + content column (max 960 px,
left-aligned). Under 720 px the sidebar becomes a top bar with the same five items
(scrollable), so phones work.

```
┌────────────┬──────────────────────────────────────────────┐
│ ◉ C0RE     │ Overview                                     │
│            │ ◉ Online · Ollama 0.40.13       Public: Off  │
│ Overview   │ ┌ Private ─ 142 in · 3 blocked ┐→ engine     │
│ Keys       │ └ Public  ─ off               ┘              │
│ Usage      │ Loaded  qwen3-vl:8b-instruct  100% GPU 6.1GB │
│ Models     │ Requests, last 24 h  ▁▂▅▇▃▁ (bars)           │
│ Settings   │ Recent requests (table)                      │
└────────────┴──────────────────────────────────────────────┘
```

Panels are flat surfaces separated by spacing and a hairline — no drop shadows, no
gradients, one corner radius family (8 px panels, 6 px controls, 999 px pills only for
status chips).

## Components

- **Reach** = segmented control (Private / Public / Off) — closely related, mutually
  exclusive choices (`segmented-controls.md`).
- **Public address** = toggle with a label naming exactly what it affects (`toggles.md`).
- **New key** = one dialog (`sheets.md`: one at a time, Cancel + Create). The secret is
  shown once inside the same dialog with Copy and "You won't see this key again."
- **Delete key** = confirm in place (destructive, not undoable).
- **Chart** = simple bars with a text summary and an equivalent hidden table
  (`charting-data.md`: keep simple, make accessible).
- Feedback in the interface (inline messages), no alerts except the delete confirmation.

## Motion

None beyond hover/focus transitions (≤ 120 ms) and the core ring's lit state. Respects
`prefers-reduced-motion`.

## Quality floor

Visible keyboard focus everywhere (2 px accent outline), every icon-only control labelled,
hit targets ≥ 28 × 28 px, light and dark by `prefers-color-scheme` (no in-app switch),
works at 390 px width.
