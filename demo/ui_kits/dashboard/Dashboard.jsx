/* global React, Header, FlowDiagram, LiveTrace, LiveStats, LogCard */
const { useEffect, useState, useRef } = React;

// --- Fake stream so the kit runs without a backend ---
const PATTERNS = [
  { decision: 'rejected', attack_type: '99% Slippage Swap',     description: 'Agent attempted swap with 99% slippage tolerance.',       rule: 'max_slippage',       reason: '9900bps exceeds maximum 200bps',     severity: 'critical', simulated_loss_usd: 498.50 },
  { decision: 'allowed',  attack_type: 'Valid USDC Transfer',   description: 'Transfer 0.5 SOL to verified wallet.',                     severity: 'none' },
  { decision: 'rejected', attack_type: 'Drain Balance',         description: 'Agent attempted to drain entire wallet in single transfer.', rule: 'max_account_drain', reason: '9.95 SOL exceeds maximum 1 SOL',  severity: 'critical', simulated_loss_usd: 1024.10 },
  { decision: 'allowed',  attack_type: 'Valid Swap',            description: 'Agent executed valid swap with 1% slippage tolerance.',  severity: 'none' },
  { decision: 'rejected', attack_type: 'Unwhitelisted Program', description: 'Agent attempted to invoke unwhitelisted program.',       rule: 'allowed_programs',   reason: 'Program not in whitelist',           severity: 'high',     simulated_loss_usd: 145.20 },
  { decision: 'rejected', attack_type: 'Excessive Priority Fee',description: 'Agent set priority fee to 2,000,000 µlamports.',         rule: 'max_priority_fee',   reason: '2,000,000 exceeds maximum 1,000,000', severity: 'medium' },
  { decision: 'rejected', attack_type: 'Account Below Rent',    description: 'Transfer would drop account under rent-exempt minimum.', rule: 'rent_exempt',        reason: 'InsufficientFundsForRent',           severity: 'high',     simulated_loss_usd: 12.40 },
  { decision: 'rejected', attack_type: 'Zero Amount Transfer',  description: 'Agent attempted to transfer 0 lamports.',                rule: 'min_transfer_value', reason: '0 below minimum 1 lamport',          severity: 'low' },
];

function useFakeStream() {
  const [log, setLog] = useState([]);
  const [allowed, setAllowed] = useState(0);
  const [blocked, setBlocked] = useState(0);
  const [avgMs, setAvgMs] = useState(43);
  const [lastDecision, setLastDecision] = useState(null);
  const i = useRef(0);

  useEffect(() => {
    const seed = [3, 1, 0, 4, 2].map((idx, k) => ({
      ...PATTERNS[idx],
      id: `seed-${k}`,
      timestamp: new Date(Date.now() - (k + 1) * 4000).toISOString(),
      simulation_time_ms: 32 + Math.floor(Math.random() * 60),
    }));
    setLog(seed);
    setAllowed(14);
    setBlocked(106);

    const id = setInterval(() => {
      const p = PATTERNS[i.current % PATTERNS.length];
      i.current += 1;
      const ms = 28 + Math.floor(Math.random() * 60);
      const entry = {
        ...p,
        id: `live-${Date.now()}-${i.current}`,
        timestamp: new Date().toISOString(),
        simulation_time_ms: ms,
      };
      setLog(prev => [entry, ...prev].slice(0, 50));
      if (entry.decision === 'allowed') setAllowed(v => v + 1);
      else setBlocked(v => v + 1);
      setAvgMs(prev => Math.round(prev * 0.7 + ms * 0.3));
      setLastDecision(entry.decision === 'rejected' ? 'reject' : 'allow');
    }, 2400);
    return () => clearInterval(id);
  }, []);

  return { log, allowed, blocked, avgMs, lastDecision };
}

function Dashboard() {
  const { log, allowed, blocked, avgMs, lastDecision } = useFakeStream();
  const latestTx = log[0];

  useEffect(() => { if (window.lucide) window.lucide.createIcons(); });

  const accuracy = 94.2;

  return (
    <div data-screen-label="01 Dashboard" style={{
      minHeight: '100vh', display: 'flex', flexDirection: 'column',
    }}>
      <Header allowed={allowed} blocked={blocked} avgMs={avgMs} />

      <div className="sak-grid" style={{
        flex: 1,
        display: 'grid',
        gridTemplateColumns: 'minmax(280px, 25%) minmax(360px, 35%) 1fr',
        gap: 20,
        padding: 20,
      }}>
        {/* LEFT — flow + 2x2 stats */}
        <div data-screen-label="02 Flow" style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <FlowDiagram lastDecision={lastDecision} simMs={avgMs} />
          <LiveStats avgMs={avgMs} rulesActive={7} accuracy={accuracy} threats={blocked} />
        </div>

        {/* CENTER — live execution trace */}
        <div data-screen-label="03 Trace" style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <LiveTrace tx={latestTx} />
        </div>

        {/* RIGHT — log */}
        <div data-screen-label="04 Log" style={{ display: 'flex', flexDirection: 'column', minHeight: 0, minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
            <i data-lucide="activity" style={{ width: 16, height: 16, color: 'var(--fg-2)', strokeWidth: 1.6 }}></i>
            <span style={{ fontSize: 11, color: 'var(--fg-2)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600, whiteSpace: 'nowrap' }}>Transaction Log</span>
            <div style={{ flex: 1 }} />
            <span style={{ fontSize: 11, color: 'var(--fg-2)', fontFamily: 'var(--font-mono)' }}>{log.length} entries</span>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12, overflowY: 'auto', paddingRight: 4, maxHeight: 'calc(100vh - 220px)' }}>
            {log.map(entry => <LogCard key={entry.id} entry={entry} />)}
          </div>
        </div>
      </div>

      {/* Bottom bar */}
      <div style={{
        position: 'sticky', bottom: 0,
        background: 'var(--surface)',
        borderTop: '1px solid var(--border)',
        padding: '12px 24px',
        display: 'flex', alignItems: 'center', gap: 24, flexWrap: 'wrap',
        fontSize: 12, color: 'var(--fg-2)', fontFamily: 'var(--font-mono)',
      }}>
        <span>Guardian Accuracy: <span style={{ color: 'var(--sak-green)', fontWeight: 700 }}>94.2%</span></span>
        <span style={{ color: 'var(--sak-border-2)' }}>|</span>
        <span>Avg Score: <span style={{ color: 'var(--fg)', fontWeight: 700 }}>4.7/5.0</span></span>
        <span style={{ color: 'var(--sak-border-2)' }}>|</span>
        <span>False Positives: <span style={{ color: 'var(--sak-orange)', fontWeight: 700 }}>1</span></span>
        <span style={{ color: 'var(--sak-border-2)' }}>|</span>
        <span>Total Feedback: <span style={{ color: 'var(--fg)', fontWeight: 700 }}>18</span></span>
        <div style={{ flex: 1 }} />
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
          <span className="sak-dot sak-dot--pulse" />
          <span>ws://localhost:3001/ws</span>
        </span>
      </div>

      {/* Responsive collapse */}
      <style>{`
        @media (max-width: 1200px) {
          .sak-grid { grid-template-columns: minmax(280px, 35%) 1fr !important; }
          .sak-grid > [data-screen-label="03 Trace"] { display: none !important; }
        }
        @media (max-width: 768px) {
          .sak-grid { grid-template-columns: 1fr !important; }
        }
      `}</style>
    </div>
  );
}

window.Dashboard = Dashboard;
