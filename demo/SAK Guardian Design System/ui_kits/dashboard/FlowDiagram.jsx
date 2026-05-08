/* global React */
const { useEffect, useState, useRef } = React;

// ============================================================
// FlowDiagram — HORIZONTAL pipeline
// AGENT → REFLEX → GUARDIAN → (BLOCKED | SOLANA)
// 52px nodes, 80px gap, particle with glow trail.
// ============================================================

function FlowDiagram({ lastDecision, simMs }) {
  const [run, setRun] = useState(0);
  const [outcome, setOutcome] = useState('allow'); // 'allow' | 'block'

  // tick a new run every 2.6s; alternate outcomes if lastDecision unset
  useEffect(() => {
    const id = setInterval(() => setRun(v => v + 1), 2600);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    if (lastDecision === 'reject') setOutcome('block');
    else if (lastDecision === 'allow') setOutcome('allow');
    else setOutcome(run % 3 === 0 ? 'block' : 'allow');
  }, [run, lastDecision]);

  useEffect(() => { if (window.lucide) window.lucide.createIcons(); });

  const isBlock = outcome === 'block';

  return (
    <div style={{
      background: 'var(--bg)',
      border: '1px solid var(--border)',
      borderRadius: 16,
      padding: '20px 16px 24px',
      backgroundImage: 'radial-gradient(circle at 1px 1px, var(--sak-border) 1px, transparent 0)',
      backgroundSize: '24px 24px',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 18 }}>
        <i data-lucide="git-branch" style={{ width: 16, height: 16, color: 'var(--fg-2)', strokeWidth: 1.6 }}></i>
        <span style={{ fontSize: 11, color: 'var(--fg-2)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600 }}>How SAK Works</span>
      </div>

      <FlowRow key={run} isBlock={isBlock} simMs={simMs} />
    </div>
  );
}

function FlowRow({ isBlock, simMs }) {
  // 4 stages: 0=agent, 1=reflex, 2=guardian, 3=final
  const [stage, setStage] = useState(0);
  const [particle, setParticle] = useState(0); // 0..3 segment index (-1 hidden)
  const [particlePos, setParticlePos] = useState(0); // 0..1 within segment

  useEffect(() => {
    const timers = [];
    // segment travel: 600ms each
    const advance = (seg) => {
      setParticle(seg);
      setParticlePos(0);
      // RAF-ish: animate over 600ms
      const start = performance.now();
      const stepId = { id: 0 };
      const step = (now) => {
        const t = Math.min(1, (now - start) / 600);
        setParticlePos(t);
        if (t < 1) stepId.id = requestAnimationFrame(step);
      };
      stepId.id = requestAnimationFrame(step);
      return stepId;
    };

    let raf1, raf2, raf3;
    timers.push(setTimeout(() => { setStage(1); raf1 = advance(0); }, 100));   // → reflex
    timers.push(setTimeout(() => { setStage(2); raf2 = advance(1); }, 800));   // → guardian
    if (isBlock) {
      timers.push(setTimeout(() => { setStage(3); }, 1500));                   // guardian flash, no further travel
    } else {
      timers.push(setTimeout(() => { setStage(3); raf3 = advance(2); }, 1500)); // → solana
    }
    return () => {
      timers.forEach(clearTimeout);
      cancelAnimationFrame(raf1?.id); cancelAnimationFrame(raf2?.id); cancelAnimationFrame(raf3?.id);
    };
  }, [isBlock]);

  const reflex = 8 + Math.floor(Math.random() * 11);
  const sim = simMs || 31;

  // shared node renderer
  const node = (i, opts) => (
    <FlowNode {...opts} active={stage > i} />
  );

  // Layout: 4 columns (nodes) interleaved with 3 segments
  // Use flex with fixed-width nodes and flex:1 segments.
  return (
    <div style={{
      display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between',
      gap: 0, paddingTop: 6,
    }}>
      <FlowNode label="AI Agent" sub="intent" icon="bot" color="var(--sak-purple)" active={stage >= 0} pulsing={stage === 0} />
      <FlowSeg active={stage >= 1} color="var(--sak-blue)" particleHere={particle === 0} pos={particlePos} />
      <FlowNode label="Reflex" sub={stage >= 1 ? `${reflex}ms` : '—'} icon="zap" color="var(--sak-blue)" active={stage >= 1} pulsing={stage === 1} />
      <FlowSeg active={stage >= 2} color="var(--sak-purple)" particleHere={particle === 1} pos={particlePos} />
      <FlowNode
        label="Guardian"
        sub={stage >= 2 ? (stage === 2 ? 'sim…' : `${sim}ms`) : 'LiteSVM'}
        icon="shield"
        color={stage >= 3 ? (isBlock ? 'var(--sak-red)' : 'var(--sak-green)') : 'var(--sak-orange)'}
        active={stage >= 2}
        pulsing={stage === 2}
        flashing={stage >= 3 && isBlock}
      />
      <FlowSeg
        active={stage >= 3}
        color={isBlock ? 'var(--sak-red)' : 'var(--sak-green)'}
        particleHere={!isBlock && particle === 2}
        pos={particlePos}
      />
      {isBlock ? (
        <FlowNode label="Blocked" sub="zero cost" icon="x" color="var(--sak-red)" active={stage >= 3} pulsing={stage === 3} variant="x" />
      ) : (
        <FlowNode label="Solana" sub={stage >= 3 ? 'sent ✓' : 'chain'} icon="circle" color="#9945ff" active={stage >= 3} pulsing={stage === 3} />
      )}
    </div>
  );
}

function FlowNode({ label, sub, icon, color, active, pulsing, flashing, variant }) {
  const ring = active ? color : 'var(--border)';
  const fill = active ? `color-mix(in oklab, ${color} 20%, var(--surface))` : 'var(--surface)';
  const iconColor = active ? color : 'var(--fg-2)';

  return (
    <div style={{
      display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6,
      flex: '0 0 auto', minWidth: 64,
    }}>
      <div style={{
        width: 52, height: 52, borderRadius: '50%',
        border: `2px solid ${ring}`, background: fill,
        display: 'grid', placeItems: 'center',
        boxShadow: active ? `0 0 24px -4px ${color}, inset 0 0 12px -6px ${color}` : 'none',
        transition: 'all 240ms var(--ease-out)',
        animation: flashing ? 'flow-flash-red 320ms ease-in-out 3' : (pulsing ? 'flow-node-pulse 700ms ease-in-out infinite' : 'none'),
      }}>
        <i data-lucide={icon} style={{ width: 22, height: 22, color: iconColor, strokeWidth: 1.8 }}></i>
      </div>
      <div style={{ fontSize: 10.5, fontWeight: 700, letterSpacing: '0.05em', textTransform: 'uppercase', color: active ? 'var(--fg)' : 'var(--fg-2)', whiteSpace: 'nowrap' }}>{label}</div>
      <div style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--fg-2)', whiteSpace: 'nowrap', minHeight: 12 }}>{sub}</div>
    </div>
  );
}

function FlowSeg({ active, color, particleHere, pos }) {
  return (
    <div style={{
      flex: 1, minWidth: 28, position: 'relative', height: 52,
      display: 'flex', alignItems: 'center',
    }}>
      {/* base line */}
      <div style={{
        width: '100%', height: 2,
        backgroundColor: active ? color : 'transparent',
        boxShadow: active ? `0 0 8px -1px ${color}` : 'none',
        backgroundImage: active ? 'none' : 'repeating-linear-gradient(to right, #2a2a3a 0, #2a2a3a 5px, transparent 5px, transparent 10px)',
        transition: 'background-color 240ms var(--ease-out)',
        position: 'relative',
      }}>
        {/* particle */}
        {particleHere && (
          <>
            {/* glow trail */}
            <div style={{
              position: 'absolute',
              top: -4, left: `calc(${pos * 100}% - 24px)`,
              width: 24, height: 10, borderRadius: 5,
              background: `linear-gradient(to right, transparent, ${color})`,
              opacity: 0.6, filter: 'blur(2px)',
              pointerEvents: 'none',
            }} />
            {/* head */}
            <div style={{
              position: 'absolute',
              top: -4, left: `calc(${pos * 100}% - 5px)`,
              width: 10, height: 10, borderRadius: '50%',
              background: '#ffffff',
              boxShadow: `0 0 14px ${color}, 0 0 4px #fff`,
              pointerEvents: 'none',
            }} />
          </>
        )}
      </div>
    </div>
  );
}

// inject keyframes once
if (!document.getElementById('flow-kf')) {
  const s = document.createElement('style');
  s.id = 'flow-kf';
  s.textContent = `
    @keyframes flow-node-pulse {
      0%,100% { transform: scale(1); }
      50%     { transform: scale(1.08); }
    }
    @keyframes flow-flash-red {
      0%,100% { box-shadow: 0 0 0 transparent; }
      50%     { box-shadow: 0 0 32px -2px var(--sak-red), inset 0 0 16px -4px var(--sak-red); }
    }
  `;
  document.head.appendChild(s);
}

window.FlowDiagram = FlowDiagram;
