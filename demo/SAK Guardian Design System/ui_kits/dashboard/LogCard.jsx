/* global React, FeedbackBar */

function severityClass(sev) {
  return {
    critical: 'sak-pill--critical',
    high:     'sak-pill--high',
    medium:   'sak-pill--medium',
    low:      'sak-pill--low',
  }[sev] || 'sak-pill--low';
}

function msColor(ms) {
  if (ms == null) return 'var(--fg-2)';
  if (ms < 100)  return 'var(--sak-green)';
  if (ms < 500)  return 'var(--sak-yellow)';
  return 'var(--sak-orange)';
}

function fmtTime(iso) {
  try { return new Date(iso).toLocaleTimeString('en-GB', { hour12: false }); }
  catch { return iso; }
}

// Sanitize raw Rust struct output → human language
function humanReason(reason) {
  if (!reason) return reason;
  let r = reason;
  r = r.replace(/FailedTransactionMetadata\s*\{[^}]*\}/g, 'Transaction would fail on-chain — insufficient funds for rent');
  r = r.replace(/InsufficientFundsForRent/g, 'Would leave account below minimum balance');
  // strip lone hex addresses without context (>=16 chars)
  r = r.replace(/\b[0-9a-fA-F]{16,}\b/g, 'address');
  return r;
}

function LogCard({ entry, onFeedback }) {
  const isReject = entry.decision === 'rejected';
  const glow = isReject ? 'sak-glow-red' : 'sak-glow-green';

  return (
    <div style={{
      background: 'var(--surface)',
      border: '1px solid var(--border)',
      borderRadius: 12,
      padding: 16,
      animation: `sak-slide-in 280ms var(--ease-out), ${glow} 1000ms var(--ease-out)`,
    }}>
      {/* Top row */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
        <span className={`sak-pill ${isReject ? 'sak-pill--blocked' : 'sak-pill--allowed'}`} style={{ whiteSpace: 'nowrap' }}>
          <span style={{
            width: 6, height: 6, borderRadius: '50%',
            background: isReject ? 'var(--sak-red)' : 'var(--sak-green)',
          }} />
          {isReject ? 'Blocked' : 'Allowed'}
        </span>
        <div style={{ flex: 1 }} />
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--fg-2)' }}>{fmtTime(entry.timestamp)}</span>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: msColor(entry.simulation_time_ms), fontWeight: 700 }}>
          {entry.simulation_time_ms}ms
        </span>
      </div>

      {/* Title row */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 6 }}>
        <span style={{ fontSize: 16, fontWeight: 600, color: 'var(--fg)', flex: 1 }}>{entry.attack_type || entry.title}</span>
        {entry.severity && entry.severity !== 'none' && (
          <span className={`sak-pill ${severityClass(entry.severity)}`} style={{ whiteSpace: 'nowrap' }}>{entry.severity}</span>
        )}
      </div>

      {/* Description */}
      <div style={{ color: 'var(--fg-2)', fontSize: 13.5, marginBottom: 10, lineHeight: 1.5 }}>
        {entry.description}
      </div>

      {isReject ? (
        <>
          <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--fg)' }}>
            Rule: <span style={{ color: 'var(--sak-yellow)' }}>{entry.rule}</span>
          </div>
          {entry.reason && (
            <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--fg-2)', marginTop: 2 }}>
              {humanReason(entry.reason)}
            </div>
          )}
          {entry.simulated_loss_usd != null && (
            <div style={{
              marginTop: 12, padding: '10px 12px',
              background: 'rgba(0,255,136,0.06)',
              border: '1px solid rgba(0,255,136,0.25)',
              borderRadius: 8,
              fontFamily: 'var(--font-mono)', fontSize: 14, fontWeight: 700,
              color: 'var(--sak-green)',
            }}>
              💰&nbsp; Prevented loss: ~${entry.simulated_loss_usd.toFixed(2)}
            </div>
          )}
        </>
      ) : (
        <>
          <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--sak-green)' }}>
            All 7 rules passed ✓
          </div>
          <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--fg-2)', marginTop: 2 }}>
            Simulation matched expected output
          </div>
        </>
      )}

      <FeedbackBar onSubmit={(verdict, n) => onFeedback?.(entry.id, verdict, n)} />
    </div>
  );
}

window.LogCard = LogCard;
