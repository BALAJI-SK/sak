/* global React */
const { useState } = React;

function FeedbackBar({ onSubmit }) {
  const [stars, setStars] = useState(0);
  const [done, setDone] = useState(false);

  if (done) {
    return (
      <div style={{ paddingTop: 12, borderTop: '1px solid var(--border)', marginTop: 12 }}>
        <span style={{ color: 'var(--sak-green)', fontSize: 12.5, fontFamily: 'var(--font-mono)' }}>
          ✓ Feedback recorded
        </span>
      </div>
    );
  }

  const submit = (verdict, n) => { setStars(n); setDone(true); onSubmit?.(verdict, n); };

  return (
    <div style={{
      paddingTop: 12, marginTop: 12,
      borderTop: '1px solid var(--border)',
      display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap',
    }}>
      {[1,2,3,4,5].map(n => (
        <button key={n}
          onMouseEnter={() => setStars(n)}
          onMouseLeave={() => setStars(0)}
          onClick={() => submit(n <= 2 ? 'wrong' : n >= 4 ? 'correct' : 'neutral', n)}
          style={{
            width: 30, height: 30,
            borderRadius: 8,
            border: `1px solid ${n <= stars ? 'var(--sak-yellow)' : 'var(--border)'}`,
            background: n <= stars ? 'rgba(255,215,0,0.10)' : 'transparent',
            color: n <= stars ? 'var(--sak-yellow)' : 'var(--fg-2)',
            fontFamily: 'var(--font-mono)', fontSize: 11, cursor: 'pointer',
            transition: 'all 120ms var(--ease-out)',
          }}>{n}★</button>
      ))}
      <div style={{ flex: 1 }} />
      <button onClick={() => submit('wrong', 1)} style={{
        height: 30, padding: '0 12px',
        borderRadius: 8,
        border: '1px solid var(--sak-red)',
        background: 'rgba(255,51,102,0.06)',
        color: 'var(--sak-red)',
        fontFamily: 'var(--font-sans)', fontSize: 12, fontWeight: 600,
        cursor: 'pointer',
      }}>✗ Wrong</button>
      <button onClick={() => submit('correct', 5)} style={{
        height: 30, padding: '0 12px',
        borderRadius: 8,
        border: '1px solid var(--sak-green)',
        background: 'rgba(0,255,136,0.06)',
        color: 'var(--sak-green)',
        fontFamily: 'var(--font-sans)', fontSize: 12, fontWeight: 600,
        cursor: 'pointer',
      }}>✓ Correct</button>
    </div>
  );
}
window.FeedbackBar = FeedbackBar;
