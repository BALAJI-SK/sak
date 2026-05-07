/* global React */
function LiveStats({ avgMs, rulesActive, accuracy, threats }) {
  const Cell = ({ label, value, color = 'var(--fg)', borderRight, borderBottom }) => (
    <div style={{
      padding: '14px 16px',
      borderRight: borderRight ? '1px solid var(--border)' : 'none',
      borderBottom: borderBottom ? '1px solid var(--border)' : 'none',
    }}>
      <div style={{ fontSize: 10, color: 'var(--fg-2)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600, marginBottom: 6 }}>{label}</div>
      <div style={{ fontFamily: 'var(--font-mono)', fontSize: 22, fontWeight: 700, color, lineHeight: 1.1 }}>{value}</div>
    </div>
  );
  return (
    <div style={{
      background: 'var(--surface)',
      border: '1px solid var(--border)',
      borderRadius: 12,
      marginTop: 16,
      display: 'grid',
      gridTemplateColumns: '1fr 1fr',
      overflow: 'hidden',
    }}>
      <Cell label="Simulation"       value={`${avgMs}ms`}                color="var(--fg)"        borderRight borderBottom />
      <Cell label="Rules"            value={`${rulesActive} active`}     color="var(--fg)"                  borderBottom />
      <Cell label="Accuracy"         value={`${accuracy.toFixed(1)}%`}   color="var(--sak-green)" borderRight />
      <Cell label="Threats"          value={`${threats} today`}          color="var(--sak-red)" />
    </div>
  );
}
window.LiveStats = LiveStats;
