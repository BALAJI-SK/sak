/* global React */
const { useEffect, useState, useRef } = React;

// ============================================================
// LiveTrace — Datadog APM-style execution trace panel
// Animates each transaction through AGENT → REFLEX → GUARDIAN
// → (BLOCKED | SOLANA) over ~2.5s using sequential setTimeouts.
// Keeps last 3 traces, oldest fades, drops after 15s.
// ============================================================

const STAGE_TIMINGS = {
  reflex: 350,
  guardian: 750,
  decision: 1450,
  complete: 1700,
};

const STAGE_ORDER = ['agent', 'reflex', 'guardian', 'decision', 'complete'];

function genTimings(tx) {
  const reflex = tx.reflex_time_ms ?? (8 + Math.floor(Math.random() * 11));
  const sim = tx.simulation_time_ms ?? (25 + Math.floor(Math.random() * 31));
  const rule = tx.rule_time_ms ?? (1 + Math.floor(Math.random() * 3));
  const total = tx.total_time_ms ?? (reflex + sim + rule);
  return { reflex, sim, rule, total };
}

function shortSig() {
  const c = 'ABCDEFGHJKLMNPQRSTUVWXYZ123456789abcdefghijkmnopqrstuvwxyz';
  let s = '';
  for (let i = 0; i < 4; i++) s += c[Math.floor(Math.random() * c.length)];
  return s + '…' + c[Math.floor(Math.random() * c.length)] + c[Math.floor(Math.random() * c.length)];
}

// ============================================================
// Single trace card
// ============================================================
function TraceCard({ tx, ageRank }) {
  const [stage, setStage] = useState('agent');
  const blocked = tx.decision === 'rejected' || tx.blocked === true;
  const t = useRef(genTimings(tx)).current;
  const sig = useRef(blocked ? null : shortSig()).current;

  useEffect(() => {
    const timeouts = [];
    timeouts.push(setTimeout(() => setStage('reflex'), STAGE_TIMINGS.reflex));
    timeouts.push(setTimeout(() => setStage('guardian'), STAGE_TIMINGS.guardian));
    timeouts.push(setTimeout(() => setStage('decision'), STAGE_TIMINGS.decision));
    timeouts.push(setTimeout(() => setStage('complete'), STAGE_TIMINGS.complete));
    return () => timeouts.forEach(clearTimeout);
  }, [tx.id]);

  // re-run lucide on each stage transition
  useEffect(() => { if (window.lucide) window.lucide.createIcons(); }, [stage]);

  const reached = (s) => STAGE_ORDER.indexOf(stage) >= STAGE_ORDER.indexOf(s);
  const opacity = ageRank === 0 ? 1 : ageRank === 1 ? 0.78 : 0.5;
  const totalColor = t.total < 100 ? 'var(--sak-green)' : t.total < 500 ? 'var(--sak-orange)' : 'var(--sak-red)';

  const [flash, setFlash] = useState(false);
  useEffect(() => {
    if (stage === 'decision') {
      setFlash(true);
      const id = setTimeout(() => setFlash(false), 1000);
      return () => clearTimeout(id);
    }
  }, [stage]);
  const flashColor = blocked ? 'rgba(255,51,102,0.85)' : 'rgba(0,255,136,0.85)';

  return (
    <div style={{
      background: 'var(--surface)',
      border: '1px solid var(--border)',
      borderRadius: 10,
      padding: '12px 14px',
      opacity,
      transition: 'opacity .4s ease, box-shadow .4s ease',
      boxShadow: flash ? `0 0 0 1px ${flashColor}, 0 0 32px -6px ${flashColor}` : 'none',
      animation: ageRank === 0 ? 'trace-slide-in 220ms ease-out' : 'none',
    }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 14 }}>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--fg-2)' }}>
          TX #{(tx.id || '').toString().slice(-4) || '0000'}
        </span>
        <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--fg)', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {tx.attack_type || 'Transaction'}
        </span>
        <span style={{
          fontFamily: 'var(--font-mono)', fontSize: 10, fontWeight: 700,
          padding: '2px 7px', borderRadius: 999,
          background: blocked ? 'rgba(255,51,102,0.12)' : 'rgba(0,255,136,0.12)',
          color: blocked ? 'var(--sak-red)' : 'var(--sak-green)',
          border: `1px solid ${blocked ? 'rgba(255,51,102,0.35)' : 'rgba(0,255,136,0.35)'}`,
        }}>
          {blocked ? '● BLOCKED' : '● ALLOWED'}
        </span>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: totalColor, fontWeight: 700 }}>
          {t.total}ms
        </span>
      </div>

      <TraceLine stage={stage} blocked={blocked} timings={t} sig={sig} tx={tx} reached={reached} />
    </div>
  );
}

// ============================================================
// 4-node trace strip (flex row: node, line, node, line, node, line, node)
// ============================================================
function TraceLine({ stage, blocked, timings, sig, tx, reached }) {
  const guardianState = stage === 'guardian'
    ? 'sim'
    : (reached('decision') ? (blocked ? 'block' : 'allow') : 'idle');

  const finalIsBlocked = blocked;

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'flex-start', gap: 0 }}>
        <Node
          icon="bot" label="Agent" sub="intent"
          activeColor="var(--sak-purple)" active={reached('agent')}
          tip={[['Source', 'AI Agent'], ['Intent', tx.attack_type || 'transfer'], ['Origin', 'agent.solanakit.dev']]}
        />
        <Segment active={reached('reflex')} color="var(--sak-blue, #3b82f6)" />
        <Node
          icon="zap" label="Reflex" sub={reached('reflex') ? `${timings.reflex}ms` : 'pending'}
          activeColor="var(--sak-blue, #3b82f6)" active={reached('reflex')}
          tip={[['Reflex Engine', ''], ['Provider', 'Helius Geyser'], ['State age', '<50ms'], ['Accounts loaded', '3'], ['Method', 'Push (not polling)']]}
        />
        <Segment active={reached('guardian')} color="var(--sak-purple)" />
        <Node
          icon="shield" label="Guardian"
          sub={reached('guardian') ? (guardianState === 'sim' ? 'sim…' : `${timings.sim}ms`) : 'LiteSVM'}
          activeColor={guardianState === 'sim' ? 'var(--sak-orange)' : guardianState === 'block' ? 'var(--sak-red)' : guardianState === 'allow' ? 'var(--sak-green)' : 'var(--sak-purple)'}
          active={reached('guardian')}
          spinning={guardianState === 'sim'}
          tip={[['Guardian (LiteSVM)', ''], ['Simulation', 'local SVM'], ['Rules checked', '7'], ['Rule fired', tx.rule || '—'], ['Pre-balance', '1,000,050 lam'], ['Post-balance', blocked ? '50 lam' : '999,938 lam'], ['On-chain cost', '$0.00']]}
        />
        <Segment active={reached('decision')} color={finalIsBlocked ? 'var(--sak-red)' : 'var(--sak-green)'} />
        {finalIsBlocked ? (
          <Node
            icon="x" label="Blocked" sub="zero cost"
            activeColor="var(--sak-red)" active={reached('decision')}
            tip={[['Blocked', ''], ['Rule', tx.rule || 'max_slippage'], ['Reason', tx.reason || '9900bps > 200bps'], ['Saved', tx.simulated_loss_usd ? `$${tx.simulated_loss_usd.toFixed(2)}` : '—']]}
          />
        ) : (
          <Node
            icon="circle" label="Solana" sub={reached('decision') ? 'broadcast' : 'chain'}
            activeColor="#9945ff" active={reached('decision')}
            tip={[['Solana Chain', ''], ['Status', 'broadcast'], ['Signature', sig || '—'], ['Slot', '298,441,019']]}
          />
        )}
      </div>

      {/* Outcome label */}
      <div style={{
        marginTop: 10, fontSize: 11, fontFamily: 'var(--font-mono)', color: 'var(--fg-2)',
        minHeight: 16, opacity: reached('decision') ? 1 : 0, transition: 'opacity .3s',
      }}>
        {reached('decision') && blocked && (
          <span>
            <span style={{ color: 'var(--sak-red)', fontWeight: 700 }}>✕ {tx.rule || 'max_slippage'}</span>
            <span> — {tx.reason || '9900bps > 200bps'}</span>
            {tx.simulated_loss_usd ? <span style={{ color: 'var(--sak-green)', fontWeight: 700, marginLeft: 8 }}>${tx.simulated_loss_usd.toFixed(2)} saved</span> : null}
          </span>
        )}
        {reached('decision') && !blocked && (
          <span>
            <span style={{ color: 'var(--sak-green)', fontWeight: 700 }}>✓ broadcast</span>
            <span> — sig {sig}</span>
          </span>
        )}
      </div>
    </div>
  );
}

// ============================================================
// Node circle
// ============================================================
function Node({ icon, label, sub, active, activeColor, spinning, tip }) {
  const [hover, setHover] = useState(false);
  const ringColor = active ? activeColor : 'var(--border)';
  const fill = active ? `color-mix(in oklab, ${activeColor} 18%, var(--surface))` : 'var(--surface)';
  const iconColor = active ? activeColor : 'var(--fg-2)';

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        position: 'relative',
        display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6,
        cursor: 'help', flex: '0 0 auto', width: 76,
      }}
    >
      <div style={{
        width: 36, height: 36, borderRadius: '50%',
        border: `1.5px solid ${ringColor}`, background: fill,
        display: 'grid', placeItems: 'center',
        boxShadow: active ? `0 0 16px -4px ${activeColor}` : 'none',
        transition: 'all .3s ease',
        animation: spinning ? 'guard-pulse 900ms ease-in-out infinite' : 'none',
      }}>
        <i data-lucide={icon} style={{ width: 16, height: 16, color: iconColor, strokeWidth: 1.8 }}></i>
      </div>
      <div style={{ fontSize: 9.5, fontWeight: 700, letterSpacing: '0.05em', textTransform: 'uppercase', color: active ? 'var(--fg)' : 'var(--fg-2)', transition: 'color .3s', whiteSpace: 'nowrap' }}>{label}</div>
      <div style={{ fontFamily: 'var(--font-mono)', fontSize: 9.5, color: 'var(--fg-2)', minHeight: 12, textAlign: 'center' }}>{sub}</div>
      {hover && tip && (
        <div style={{
          position: 'absolute', bottom: 'calc(100% + 6px)', left: '50%', transform: 'translateX(-50%)',
          background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 8,
          padding: '8px 10px', whiteSpace: 'nowrap', zIndex: 50,
          boxShadow: '0 8px 24px -8px rgba(0,0,0,0.6)',
        }}>
          <table style={{ borderCollapse: 'collapse', fontFamily: 'var(--font-mono)', fontSize: 10.5 }}>
            <tbody>
              {tip.map(([k, v], i) => (
                <tr key={i}>
                  <td style={{ color: v ? 'var(--fg-2)' : 'var(--fg)', fontWeight: v ? 400 : 700, padding: '2px 12px 2px 0' }}>{k}</td>
                  <td style={{ color: 'var(--fg)', padding: '2px 0' }}>{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ============================================================
// Connecting line segment with travelling dot
// ============================================================
function Segment({ active, color }) {
  return (
    <div style={{
      flex: 1, position: 'relative',
      height: 36, // match node circle so dots vertically center
      display: 'flex', alignItems: 'center',
      minWidth: 24,
    }}>
      <div style={{
        width: '100%', height: 2,
        backgroundImage: active ? 'none' : 'linear-gradient(to right, #2a2a3a 50%, transparent 0%)',
        backgroundSize: '8px 2px',
        background: active ? color : undefined,
        backgroundColor: active ? color : 'transparent',
        boxShadow: active ? `0 0 8px -1px ${color}` : 'none',
        transition: 'all .3s ease',
        position: 'relative',
        overflow: 'visible',
      }}>
        {!active && <div style={{
          position: 'absolute', inset: 0,
          backgroundImage: 'repeating-linear-gradient(to right, #2a2a3a 0, #2a2a3a 4px, transparent 4px, transparent 8px)',
        }} />}
        {active && (
          <div style={{
            position: 'absolute', top: -3, left: 0,
            width: 8, height: 8, borderRadius: '50%',
            background: color, boxShadow: `0 0 10px ${color}`,
            animation: 'trace-dot 350ms ease-out forwards',
          }} />
        )}
      </div>
    </div>
  );
}

// ============================================================
// Top-level — accepts a single `tx` prop; pushes onto stack on change
// ============================================================
function LiveTrace({ tx }) {
  const [traces, setTraces] = useState([]);

  useEffect(() => {
    if (!tx) return;
    setTraces(prev => {
      if (prev.find(p => p.id === tx.id)) return prev;
      return [tx, ...prev].slice(0, 3);
    });
    const id = setTimeout(() => {
      setTraces(prev => prev.filter(p => p.id !== tx.id));
    }, 15000);
    return () => clearTimeout(id);
  }, [tx?.id]);

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
        <i data-lucide="git-branch" style={{ width: 16, height: 16, color: 'var(--fg-2)', strokeWidth: 1.6 }}></i>
        <span style={{ fontSize: 11, color: 'var(--fg-2)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600, whiteSpace: 'nowrap' }}>Live Execution Trace</span>
        <span className="sak-dot sak-dot--pulse" style={{ marginLeft: 'auto' }} />
      </div>
      <div style={{ fontSize: 11, color: 'var(--fg-2)', marginBottom: 12 }}>
        Watch each transaction move through SAK
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {traces.length === 0 && (
          <div style={{ padding: '24px 12px', textAlign: 'center', color: 'var(--fg-2)', fontSize: 12, border: '1px dashed var(--border)', borderRadius: 10 }}>
            Waiting for transactions…
          </div>
        )}
        {traces.map((t, idx) => (
          <TraceCard key={t.id} tx={t} ageRank={idx} />
        ))}
      </div>

      <style>{`
        @keyframes trace-slide-in {
          from { transform: translateY(-8px); opacity: 0; }
          to   { transform: translateY(0);    opacity: 1; }
        }
        @keyframes guard-pulse {
          0%, 100% { box-shadow: 0 0 12px -4px var(--sak-orange); transform: scale(1); }
          50%      { box-shadow: 0 0 22px -2px var(--sak-orange); transform: scale(1.06); }
        }
        @keyframes trace-dot {
          from { left: 0%;   opacity: 1; }
          to   { left: 100%; opacity: 0; }
        }
      `}</style>
    </div>
  );
}

window.LiveTrace = LiveTrace;
