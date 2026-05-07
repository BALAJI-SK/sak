# SAK Guardian Design System

> Every transaction simulated before signing.

A production-grade design system for **SAK Guardian** — a Rust middleware kernel that sits between an LLM-driven agent and the Solana blockchain, simulating every transaction in LiteSVM before signing and blocking malicious ones at zero on-chain cost.

This system retools the existing dark-log demo into a **judge-readable security dashboard**: a three-panel "story" UI that reads in 10 seconds — AI agent → Guardian intercepts → block / allow → live transaction feed.

---

## Sources

- **Codebase (primary source of truth):** GitHub `BALAJI-SK/sak` @ `main`
  - `demo/race-ui/src/App.tsx` — current React UI (the thing being redesigned)
  - `demo/tx-generator/src/main.rs` — transaction patterns, severity, copy
  - `demo/race-server/src/main.rs` — WebSocket server (`ws://localhost:3001/ws`)
  - `rules.yaml` — Guardian rule set (7 active rules)
  - `SAK.md` / `README.md` — product positioning, three-pillar architecture
- **Hackathon context:** Colosseum Frontier, deadline May 11, 2026

---

## Index

| File / folder | What's in it |
|---|---|
| `README.md` | This file. Brand context + content + visual + iconography |
| `SKILL.md` | Cross-compatible Agent Skill manifest |
| `colors_and_type.css` | All design tokens — colors, type scale, radii, shadows, motion |
| `assets/` | Logos, brand marks, icon snippets |
| `preview/` | Tiny HTML cards that populate the Design System tab |
| `ui_kits/dashboard/` | Pixel-faithful redesign of the Guardian dashboard. Open `index.html` |

---

## Brand at a glance

- **Name:** SAK Guardian
- **Pillar:** "Pillar 2" of SAK (Solana Agent Kernel) — the pre-sign kill switch
- **Tagline:** *Every transaction simulated before signing*
- **Audience:** Hackathon judges (10-second comprehension), then Solana devs and agent founders
- **One-line pitch:** *Ship agents that can't be used against you.*
- **Voice:** Confident, technical, declarative. Numbers over adjectives.

### Color palette (canonical)

| Token | Hex | Use |
|---|---|---|
| `bg` | `#0a0a0f` | Page background (near black, blue-shifted) |
| `surface` | `#12121a` | Cards, panels |
| `border` | `#1e1e2e` | All borders, dividers |
| `green` | `#00ff88` | ALLOWED, system-active, brand |
| `red` | `#ff3366` | BLOCKED, critical |
| `orange` | `#ff9900` | High severity, warning |
| `purple` | `#7c3aed` | Accent (AI agent node, links) |
| `text` | `#ffffff` | Primary text |
| `text-2` | `#8888aa` | Secondary, metadata, timestamps |

### Typography

- **Display + UI:** Inter (300 / 400 / 500 / 600 / 700) — loaded from Google Fonts
- **Code / numbers / addresses:** JetBrains Mono (400 / 500 / 700) — loaded from Google Fonts

---

## CONTENT FUNDAMENTALS

How copy is written across the product.

**Voice.** Declarative and short. The product *does* things; it doesn't suggest. "Blocks malicious tx with zero on-chain cost." No marketing softeners — no "helps you", "enables you to", "powerful". State the action, then the proof.

**Tense + person.** Third-person systems ("Guardian intercepts", "Reflex Engine subscribes"). Avoid "we" outside the README. Never use "I". Address users as "you" only in CTAs ("Ship agents that can't be used against you").

**Casing.** Title Case for product nouns (Guardian, Reflex Engine, Evil Corpus). UPPERCASE for state badges (BLOCKED, ALLOWED, CRITICAL, HIGH). lowercase for rule identifiers (`max_slippage`, `allowed_programs`) — they are code, treat them as code in mono.

**Numbers.** Always concrete. "blocks in 43ms", "$498.50 prevented", "200bps max", "1000× cheaper rent". Round only when speaking to non-technical readers; otherwise show the actual number.

**Status + decision phrasing.** Two columns, parallel construction:
- BLOCKED → `Rule fired: <rule_name>` / `Reason: <bps/lamports/programs>` / `Prevented loss: $X.XX`
- ALLOWED → `All <N> rules passed` / `Simulation matched expected output`

**Don't show.** Per spec: no raw Rust struct output, internal variable names, stack traces, debug formatting, "Unknown program" without a label, or hex addresses without a human-readable label. If the only data is a pubkey, label it ("Recipient", "Program") and truncate to `Aa11…ZzZZ`.

**Emoji.** Avoided in production UI. Glyph-level severity is shown via colored dots (●) and pill colors, not 🟢/🔴. Two narrow exceptions in marketing copy: ❤ in "Built with ❤ for Solana" (existing repo line) and ⚡ as a synonym for speed in stat pills.

**Examples (lift these verbatim).**
- Header subtitle: *Live safety log — every transaction simulated before signing.*
- Card title (block): *99% Slippage Swap*
- Card body (block): *Agent tried to swap 100 USDC with 99% slippage.*
- Rule line: *Rule fired: max_slippage*
- Reason line: *9900bps exceeds maximum 200bps*
- Prevented loss: *Prevented loss: ~$498.50* (green)
- Card title (allow): *Valid USDC Transfer*
- Card body (allow): *All 7 rules passed ✓*

---

## VISUAL FOUNDATIONS

The vibe is **monitoring console** — Bloomberg Terminal energy crossed with a modern security ops dashboard. Dense, dark, fast-feeling, no decoration that doesn't earn its place.

### Color use
- Background is **near-black with a slight blue cast** (`#0a0a0f`) — never pure black, never grey. This shift makes purple/green accents read clean.
- Surfaces are one step lighter (`#12121a`) and edges are picked out with a single hairline border (`#1e1e2e`, 1px). No layered shadows for elevation in the dark — borders do the work.
- **Three semantic colors:** green for go, red for stop, orange for caution. Purple is the brand accent (used sparingly: the AI agent node in the flow diagram, hyperlinks, "Pillar" callouts). Don't introduce a 4th hue.
- Text contrast: `#ffffff` for primary, `#8888aa` for secondary. There is no "tertiary" — if it's not important enough to be `#8888aa`, cut it.

### Type
- Inter for everything UI. JetBrains Mono only for: rule names, hex/base58 addresses, lamport amounts, timing values (ms), rule code blocks.
- Display sizes use **tight tracking** (`-0.02em` to `-0.04em`) and weight 600–700.
- Body is 14–15px / 1.5 line-height.
- Numbers in stat pills are weight 700, mono — they should feel like a count-up readout.

### Backgrounds + texture
- No images, no illustrations, no patterns. The dashboard is the product.
- One permitted effect: **soft radial color blooms** behind stat tiles using `bg-{color}/5` + `blur-3xl` (lifted from the existing demo). Used as 5–10% saturation tints, never above ~10%.
- Optional faint dot grid (`#1e1e2e` 1px every 24px) on left-panel canvas only, behind the flow diagram.

### Borders, radii, elevation
- **1px hairline borders everywhere.** `--border` (`#1e1e2e`) by default; semantic-tinted borders on cards (`rgba(255,51,102,0.4)` for blocked, `rgba(0,255,136,0.4)` for allowed) for the 1-second appear-glow.
- **Radii:** `8px` for buttons/pills/inputs, `12px` for cards and tiles, `999px` for status pills, `4px` for tiny chips.
- **Shadows in dark mode are color glows, not drop shadows.** A blocked card on appear: `box-shadow: 0 0 24px -4px rgba(255,51,102,0.5)` decaying to `0` over 1s. Same shape for allowed (green).

### Animation
- All transitions: **180–300ms, `cubic-bezier(0.2, 0.8, 0.2, 1)`** (ease-out-quart-ish). No bouncy springs anywhere — this is a security tool.
- New log card: slide-in from right `translateX(24px) → 0`, fade `0 → 1`, 280ms.
- Decision flash: card border glows (red or green) at peak intensity, decays over 1000ms.
- Flow diagram particle: 2-second loop, linear, with a 120ms color-flash at the decision node.
- Guardian shield: pulses `scale(1) → scale(1.06)` over 240ms when a decision lands; halts when idle.
- Counters: tween numerically (don't snap), 600ms, ease-out.
- System Active dot: 2s pulse, infinite.
- **Hover states:** opacity stays at 1 — instead, raise border by one step (`#1e1e2e` → `#2a2a3e`) and lift surface by `bg-white/[0.02]`. No translate-y on hover (keeps the dense feel).
- **Press states:** 96% scale, 80ms. Buttons go to a darker fill, not a lighter one.

### Transparency + blur
- Used only for the soft color blooms (above) and for the System Active pill background (`rgba(0,255,136,0.08)` over surface).
- No frosted-glass nav bars, no acrylic. Decisive opaque surfaces.

### Layout rules
- **Three-panel grid:** header (auto), left flow (40%), right log (60%). On 1080p and below, no horizontal scroll — left panel can collapse to 360px min, right panel takes remainder.
- **Header is sticky.** Log panel scrolls; flow panel is static (it's a story, not a list).
- 24px gutter between panels, 24px page padding, 16px inside cards.

### Card anatomy (transaction log)
- Surface `#12121a`, border `#1e1e2e` 1px, radius 12px, padding 16px.
- Top row: status badge (BLOCKED/ALLOWED) + title — left; timestamp + ms timing — right.
- Body: 1-line description (`#ffffff`), 1-line rule fired (mono, `#8888aa`), 1-line reason (`#8888aa`).
- Footer (blocked only): green "Prevented loss: $X.XX" mono.
- Below body: feedback row (1–5 stars + Wrong / Correct), separated by a 1px divider.
- Newest card first; max 50 in DOM (lifted from existing).

---

## ICONOGRAPHY

The product uses **Lucide** (lucide-react, but loaded via the CDN icon-font in this design system) — it's already what the existing UI imports (`shield-alert`, `shield-check`, `check-circle`, `x-circle`, `target`, `activity`, `bar-chart-3`, `clock`).

- **Style:** outline, 1.5px stroke, 24×24 default. Never filled.
- **Color:** inherits text color of its container. Status icons take the semantic color (green/red/orange/purple) of their state.
- **Loading:** CDN `https://unpkg.com/lucide@latest/dist/umd/lucide.js`, then `lucide.createIcons()` after mount. (Documented in `ui_kits/dashboard/index.html`.)
- **Used set in this system:** `shield-alert`, `shield-check`, `shield`, `cpu`, `bot`, `zap`, `activity`, `check-circle`, `x-circle`, `alert-triangle`, `target`, `clock`, `bar-chart-3`, `arrow-down`, `circle-dot`, `gauge`. Only add to this list if a new screen genuinely needs an unused icon.
- **No emoji** in product surfaces. The brief shows 🟢🔴⚡ in metric pills — render those as colored dots (`●`) or actual lucide glyphs (`check-circle`, `x-circle`, `zap`), not emoji.
- **No custom SVG illustrations.** The flow diagram is built from boxes + lines + lucide icons; if a node needs a logo (Solana, Geyser), copy the SPL/Solana mark from `assets/`.
- **Brand mark:** the SAK wordmark is type-only (Inter 700, with a green dot before "Guardian"). See `assets/sak-wordmark.svg`.

---

## Substitutions to confirm

- **Fonts:** Inter and JetBrains Mono are loaded from Google Fonts via CSS `@import`. No local `.ttf` is checked in. If you want offline-safe local copies, attach them and I'll wire them up in `fonts/`.
- **Lucide icons:** Loaded from the unpkg CDN — same approach as the existing demo. If the production build needs to ship offline, swap to `lucide-react` as an npm dep and tree-shake.
- **Solana / Geyser logos:** Not provided in repo. The flow diagram references them by name only ("REFLEX ENGINE — Geyser logo" per the brief); I've placed neutral icon glyphs as placeholders in the kit. Drop real SVGs into `assets/` to upgrade.

---

## Iterate with me

I built the system from the codebase + brief alone. Areas worth confirming with you:
1. **Fonts** — happy with Inter + JetBrains Mono via CDN, or do you want local files?
2. **Logo** — the SAK wordmark in `assets/` is type-only with a green dot. Do you have a real mark?
3. **Imagery** — brief says no images; the kit has none. Confirm.
4. **Iconography** — Lucide is used as in the existing demo. Confirm before I propagate it.
