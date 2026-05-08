/* global React */
const { useState } = React;

function StatPill({ tone, label, value, suffix }) {
  const tones = {
    green:  { color: 'var(--sak-green)',  bloom: 'rgba(0,255,136,0.12)',  border: 'rgba(0,255,136,0.35)' },
    red:    { color: 'var(--sak-red)',    bloom: 'rgba(255,51,102,0.12)', border: 'rgba(255,51,102,0.35)' },
    purple: { color: 'var(--sak-purple)', bloom: 'rgba(124,58,237,0.18)', border: 'rgba(124,58,237,0.4)'  },
  };
  const t = tones[tone];
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10,
      height: 38, padding: '0 14px',
      borderRadius: 999,
      background: t.bloom,
      border: `1px solid ${t.border}`,
    }}>
      <span style={{
        width: 8, height: 8, borderRadius: '50%',
        background: t.color, boxShadow: `0 0 8px ${t.color}`,
      }} />
      <span style={{ color: 'var(--fg-2)', fontSize: 12, letterSpacing: '0.04em', textTransform: 'uppercase', fontWeight: 600 }}>{label}</span>
      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 15, fontWeight: 700, color: t.color }}>
        {value}{suffix && <span style={{ color: 'var(--fg-2)', fontSize: 11, marginLeft: 2 }}>{suffix}</span>}
      </span>
    </div>
  );
}

function Header({ allowed, blocked, avgMs }) {
  return (
    <header style={{
      display: 'flex', alignItems: 'center', gap: 24,
      padding: '20px 24px',
      background: 'var(--surface)',
      borderBottom: '1px solid var(--border)',
      flexWrap: 'wrap',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <span className="sak-dot sak-dot--pulse" style={{ width: 10, height: 10 }} />
        <div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
            <span style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.03em', color: 'var(--fg)' }}>SAK</span>
            <span style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.03em', color: 'var(--sak-green)' }}>Guardian</span>
          </div>
          <div style={{ fontSize: 12, color: 'var(--fg-2)', marginTop: 2 }}>
            Live safety log — every transaction simulated before signing
          </div>
        </div>
      </div>

      <div style={{ flex: 1 }} />

      <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
        <StatPill tone="green"  label="Allowed" value={allowed} />
        <StatPill tone="red"    label="Blocked" value={blocked} />
        <StatPill tone="purple" label="Avg"     value={avgMs} suffix="ms" />
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8,
          height: 38, padding: '0 14px',
          borderRadius: 999,
          background: 'rgba(0,255,136,0.06)',
          border: '1px solid rgba(0,255,136,0.35)',
        }}>
          <span className="sak-dot sak-dot--pulse" />
          <span style={{ fontSize: 12, color: 'var(--sak-green)', fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', whiteSpace: 'nowrap' }}>System Active</span>
        </div>
      </div>
    </header>
  );
}

window.Header = Header;
