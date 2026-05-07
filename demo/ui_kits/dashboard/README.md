# Guardian Dashboard — UI Kit

Pixel-faithful redesign of the SAK Guardian live safety log. Replaces the original `demo/race-ui/src/App.tsx` (a single-column dark log dump) with the three-panel **judge-readable security dashboard** specified in the brief.

## Files
- `index.html` — runnable demo. Loads React 18 + Babel + Tailwind via CDN, then this kit's components.
- `tokens.css` — re-exports `colors_and_type.css` design tokens.
- `Header.jsx` — top bar with brand, stats pills, system-active dot.
- `FlowDiagram.jsx` — left panel: animated pipeline (AI Agent → Reflex → Guardian → Allow/Block).
- `LiveStats.jsx` — left panel footer: simulation speed, rules active, accuracy, threats today.
- `LogCard.jsx` — right panel: one transaction log card (blocked / allowed variants).
- `FeedbackBar.jsx` — 1-5 stars + Wrong/Correct row inside each card.
- `Dashboard.jsx` — orchestrates everything, runs a fake transaction stream (no WebSocket needed for the kit).

## Wiring this into the real app
The kit is self-contained — it generates a fake stream so it works offline. To plug into the real WebSocket:
1. In `Dashboard.jsx`, replace the `useFakeStream()` hook with a `useEffect` that opens `ws://localhost:3001/ws` (same as the original).
2. Map incoming JSON to the `LogEntry` shape used here. The shape is identical to the existing repo (`id`, `timestamp`, `decision`, `rule`, `reason`, `severity`, `simulated_loss_usd`, `simulation_time_ms`).
3. Reconnect-with-backoff is required (already part of the brief's tech requirements).

## Visual rules enforced here
- 3-panel layout: header / 40% flow / 60% log. No horizontal scroll at 1080p.
- All colors come from `colors_and_type.css` — no hex literals in JSX.
- Animations: 280ms slide-in for new cards, 1s glow decay, 2s pipeline particle.
- Lucide icons via CDN. No emoji. No SVG illustrations.
