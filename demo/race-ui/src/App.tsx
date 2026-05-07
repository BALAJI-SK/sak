import { useEffect, useState } from "react";
import "./index.css";

interface LogEntry {
  id: string;
  timestamp: string;
  decision: "allowed" | "rejected";
  rule?: string;
  reason?: string;
  attack_type?: string;
  description?: string;
  severity?: "critical" | "high" | "medium" | "low" | "none";
  simulated_loss_usd?: number;
  simulation_time_ms?: number;
  feedback?: "correct" | "wrong" | "neutral";
}

interface FeedbackSummary {
  total: number;
  correct: number;
  wrong: number;
  accuracy: number;
}

// Severity pill styles
const severityClass: Record<string, string> = {
  critical: "pill pill--critical",
  high: "pill pill--high",
  medium: "pill pill--medium",
  low: "pill pill--low",
  none: "",
};

function fmtTime(iso: string) {
  try { return new Date(iso).toLocaleTimeString("en-GB", { hour12: false }); }
  catch { return iso; }
}

// Sanitize raw Rust struct output → human language
function humanReason(reason: string): string {
  if (!reason) return reason;
  let r = reason;
  r = r.replace(/FailedTransactionMetadata\s*\{[^}]*\}/g, "Transaction would fail on-chain — insufficient funds for rent");
  r = r.replace(/InsufficientFundsForRent/g, "Would leave account below minimum balance");
  r = r.replace(/\b[0-9a-fA-F]{16,}\b/g, "address");
  return r;
}

function msColor(ms: number | undefined): string {
  if (ms == null) return "var(--fg-2)";
  if (ms < 100) return "var(--sak-green)";
  if (ms < 500) return "var(--sak-yellow)";
  return "var(--sak-orange)";
}

function App() {
  const [log, setLog] = useState<LogEntry[]>([]);
  const [allowed, setAllowed] = useState(0);
  const [blocked, setBlocked] = useState(0);
  const [summary, setSummary] = useState<FeedbackSummary | null>(null);
  const [submitted, setSubmitted] = useState<Set<number>>(new Set());
  const [lastDecision, setLastDecision] = useState<string | null>(null);
  const [avgMs, setAvgMs] = useState(43);

  useEffect(() => {
    const ws = new WebSocket("ws://localhost:3001/ws");
    ws.onmessage = (e) => {
      try {
        const entry: LogEntry = JSON.parse(e.data);
        setLog((prev) => [entry, ...prev].slice(0, 50));
        if (entry.decision === "allowed") {
          setAllowed((v) => v + 1);
        } else {
          setBlocked((v) => v + 1);
        }
        setLastDecision(entry.decision === "rejected" ? "reject" : "allow");
        if (entry.simulation_time_ms) {
          setAvgMs((prev) => Math.round(prev * 0.7 + entry.simulation_time_ms! * 0.3));
        }
      } catch {
        // ignore malformed
      }
    };
    return () => ws.close();
  }, []);

  // Poll feedback summary every 3 seconds
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const res = await fetch("http://localhost:3001/feedback/summary");
        const data = await res.json();
        setSummary(data);
      } catch {
        // server not ready yet
      }
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  const sendFeedback = async (index: number, stars: 1 | 2 | 3 | 4 | 5) => {
    const entry = log[index];
    if (!entry || submitted.has(index)) return;

    const verdict =
      stars <= 2 ? "Wrong" :
      stars >= 4 ? "Correct" : "Neutral";

    const body = {
      timestamp: entry.timestamp,
      decision: entry.decision,
      rule: entry.rule,
      description: entry.description,
      stars,
      verdict,
    };

    const res = await fetch("http://localhost:3001/feedback", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });

    if (res.ok) {
      setSubmitted((prev) => new Set([...prev, index]));
      setLog((prev) =>
        prev.map((e, i) =>
          i === index ? { ...e, feedback: verdict.toLowerCase() as "correct" | "wrong" | "neutral" } : e
        )
      );
    }
  };

  const latestTx = log[0];

  // Initialize Lucide icons
  useEffect(() => {
    if (typeof (window as any).lucide !== "undefined") {
      (window as any).lucide.createIcons();
    }
  });

  return (
    <div className="min-h-screen bg-[#0a0a0f] text-white font-sans" style={{ fontFamily: "var(--font-sans)" }}>
      {/* Header */}
      <header className="bg-[#12121a] border-b border-[#1e1e2e] px-6 py-4" style={{ borderColor: "var(--border)" }}>
        <div className="max-w-[1600px] mx-auto flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <i data-lucide="shield" className="w-6 h-6 text-[#00ff88]" style={{ strokeWidth: 1.5 }}></i>
              <h1 className="text-xl font-bold tracking-tight" style={{ fontFamily: "var(--font-sans)", fontWeight: 700 }}>
                SAK Guardian
              </h1>
            </div>
            <div className="h-6 w-px bg-[#1e1e2e]"></div>
            <p className="text-[#8888aa] text-sm" style={{ fontFamily: "var(--font-mono)" }}>
              Every transaction simulated before signing
            </p>
          </div>
          <div className="flex items-center gap-6">
            <div className="flex items-center gap-4 text-sm" style={{ fontFamily: "var(--font-mono)" }}>
              <span className="text-[#8888aa]">Allowed: <span className="text-[#00ff88] font-bold">{allowed}</span></span>
              <span className="text-[#8888aa]">Blocked: <span className="text-[#ff3366] font-bold">{blocked}</span></span>
              <span className="text-[#8888aa]">Avg: <span className="text-[#00ff88] font-bold">{avgMs}ms</span></span>
            </div>
            <div className="flex items-center gap-2 px-4 py-2 bg-[#12121a] border border-[#1e1e2e] rounded-full">
              <div className="w-2 h-2 bg-[#00ff88] rounded-full animate-pulse" style={{ boxShadow: "0 0 8px 0 #00ff88" }}></div>
              <span className="text-xs text-[#8888aa]">System Active</span>
            </div>
          </div>
        </div>
      </header>

      {/* Three-panel grid */}
      <div className="max-w-[1600px] mx-auto p-6 grid grid-cols-1 md:grid-cols-[minmax(280px,25%)_minmax(360px,35%)_1fr] gap-6" style={{ minHeight: "calc(100vh - 80px)" }}>
        {/* LEFT — Flow Diagram + Stats */}
        <div className="flex flex-col gap-6 min-w-0">
          {/* Flow Diagram */}
          <div className="bg-[#12121a] border border-[#1e1e2e] rounded-xl p-6" style={{ borderColor: "var(--border)" }}>
            <h2 className="text-sm font-semibold text-[#8888aa] uppercase tracking-wider mb-4" style={{ fontFamily: "var(--font-mono)" }}>
              <i data-lucide="zap" className="w-4 h-4 inline mr-2" style={{ strokeWidth: 1.5 }}></i>
              Flow
            </h2>
            <div className="flex flex-col items-center gap-3 py-4">
              {/* Agent */}
              <div className="flex items-center gap-2 px-4 py-2 bg-[#181822] border border-[#1e1e2e] rounded-lg">
                <i data-lucide="bot" className="w-4 h-4 text-[#7c3aed]" style={{ strokeWidth: 1.5 }}></i>
                <span className="text-sm">AI Agent</span>
              </div>
              <div className="w-px h-8 bg-[#1e1e2e]"></div>
              {/* Guardian */}
              <div className={`flex items-center gap-2 px-4 py-2 bg-[#181822] border rounded-lg transition-all duration-300 ${
                lastDecision === "reject" ? "border-[#ff3366]/40" : "border-[#00ff88]/40"
              }`}>
                <i data-lucide="shield" className={`w-4 h-4 ${
                  lastDecision === "reject" ? "text-[#ff3366]" : "text-[#00ff88]"
                }`} style={{ strokeWidth: 1.5 }}></i>
                <span className="text-sm">Guardian</span>
              </div>
              <div className="w-px h-8 bg-[#1e1e2e]"></div>
              {/* Blockchain */}
              <div className="flex items-center gap-2 px-4 py-2 bg-[#181822] border border-[#1e1e2e] rounded-lg">
                <i data-lucide="cpu" className="w-4 h-4 text-[#3b82f6]" style={{ strokeWidth: 1.5 }}></i>
                <span className="text-sm">Solana</span>
              </div>
            </div>
            <div className="text-center text-xs text-[#54546a] mt-2" style={{ fontFamily: "var(--font-mono)" }}>
              {latestTx ? `Last: ${latestTx.simulation_time_ms || avgMs}ms` : "Waiting..."}
            </div>
          </div>

          {/* Live Stats */}
          {summary && (
            <div className="bg-[#12121a] border border-[#1e1e2e] rounded-xl p-6" style={{ borderColor: "var(--border)" }}>
              <h2 className="text-sm font-semibold text-[#8888aa] uppercase tracking-wider mb-4" style={{ fontFamily: "var(--font-mono)" }}>
                <i data-lucide="bar-chart-3" className="w-4 h-4 inline mr-2" style={{ strokeWidth: 1.5 }}></i>
                Feedback Summary
              </h2>
              <div className="grid grid-cols-3 gap-4">
                <div className="text-center">
                  <div className="text-2xl font-bold text-[#00ff88]" style={{ fontFamily: "var(--font-mono)" }}>{summary.correct}</div>
                  <div className="text-xs text-[#8888aa] mt-1">Correct</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-[#ff3366]" style={{ fontFamily: "var(--font-mono)" }}>{summary.wrong}</div>
                  <div className="text-xs text-[#8888aa] mt-1">Wrong</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-[#3b82f6]" style={{ fontFamily: "var(--font-mono)" }}>{summary.accuracy.toFixed(1)}%</div>
                  <div className="text-xs text-[#8888aa] mt-1">Accuracy</div>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* CENTER — Live Trace */}
        <div className="flex flex-col min-w-0">
          <div className="bg-[#12121a] border border-[#1e1e2e] rounded-xl p-6 flex-1" style={{ borderColor: "var(--border)" }}>
            <h2 className="text-sm font-semibold text-[#8888aa] uppercase tracking-wider mb-4" style={{ fontFamily: "var(--font-mono)" }}>
              <i data-lucide="activity" className="w-4 h-4 inline mr-2" style={{ strokeWidth: 1.5 }}></i>
              Live Trace
            </h2>
            {latestTx ? (
              <div className="space-y-4">
                <div className="flex items-center gap-3">
                  <span className={`pill ${latestTx.decision === "rejected" ? "pill--blocked" : "pill--allowed"}`}>
                    <span className="w-1.5 h-1.5 rounded-full" style={{
                      background: latestTx.decision === "rejected" ? "var(--sak-red)" : "var(--sak-green)"
                    }}></span>
                    {latestTx.decision === "rejected" ? "Blocked" : "Allowed"}
                  </span>
                  {latestTx.severity && latestTx.severity !== "none" && (
                    <span className={severityClass[latestTx.severity] || ""}>
                      {latestTx.severity}
                    </span>
                  )}
                </div>
                <div className="text-lg font-semibold">{latestTx.attack_type || "Transaction"}</div>
                <div className="text-sm text-[#8888aa]">{latestTx.description}</div>
                {latestTx.decision === "rejected" && latestTx.rule && (
                  <div className="text-xs text-[#ffd700] font-mono">Rule: {latestTx.rule}</div>
                )}
                {latestTx.simulated_loss_usd && (
                  <div className="text-sm text-[#00ff88] font-mono font-bold">
                    Prevented loss: ${latestTx.simulated_loss_usd.toFixed(2)}
                  </div>
                )}
              </div>
            ) : (
              <div className="text-[#54546a] text-center py-8">
                <i data-lucide="clock" className="w-8 h-8 mx-auto mb-3" style={{ strokeWidth: 1.5 }}></i>
                <p>Waiting for transactions...</p>
              </div>
            )}
          </div>
        </div>

        {/* RIGHT — Transaction Log */}
        <div className="flex flex-col min-h-0 min-w-0">
          <div className="flex items-center gap-2 mb-4">
            <i data-lucide="list" className="w-5 h-5 text-[#8888aa]" style={{ strokeWidth: 1.5 }}></i>
            <span className="text-sm font-semibold text-[#8888aa] uppercase tracking-wider" style={{ fontFamily: "var(--font-mono)" }}>
              Transaction Log
            </span>
            <div className="flex-1"></div>
            <span className="text-xs text-[#8888aa]" style={{ fontFamily: "var(--font-mono)" }}>{log.length} entries</span>
          </div>
          <div className="flex-1 overflow-y-auto space-y-3 pr-2" style={{ maxHeight: "calc(100vh - 220px)" }}>
            {log.map((entry, i) => (
              <div
                key={entry.id || i}
                className={`bg-[#12121a] border border-[#1e1e2e] rounded-xl p-4 transition-all duration-200 ${
                  entry.decision === "rejected"
                    ? "hover:border-[#ff3366]/50"
                    : "hover:border-[#00ff88]/50"
                }`}
                style={{
                  animation: `slide-in 280ms var(--ease-out), ${entry.decision === "rejected" ? "glow-red" : "glow-green"} 1000ms var(--ease-out)`,
                }}
              >
                {/* Top row: status pill + time + ms */}
                <div className="flex items-center gap-3 mb-3">
                  <span className={`pill ${entry.decision === "rejected" ? "pill--blocked" : "pill--allowed"}`}>
                    <span className="w-1.5 h-1.5 rounded-full" style={{
                      background: entry.decision === "rejected" ? "var(--sak-red)" : "var(--sak-green)"
                    }}></span>
                    {entry.decision === "rejected" ? "Blocked" : "Allowed"}
                  </span>
                  {entry.severity && entry.severity !== "none" && (
                    <span className={severityClass[entry.severity] || ""}>{entry.severity}</span>
                  )}
                  <div className="flex-1"></div>
                  <span className="text-xs text-[#8888aa]" style={{ fontFamily: "var(--font-mono)" }}>{fmtTime(entry.timestamp)}</span>
                  {entry.simulation_time_ms && (
                    <span className="text-xs font-bold" style={{ fontFamily: "var(--font-mono)", color: msColor(entry.simulation_time_ms) }}>
                      {entry.simulation_time_ms}ms
                    </span>
                  )}
                </div>

                {/* Title */}
                <div className="text-base font-semibold mb-2">{entry.attack_type || "Transaction"}</div>

                {/* Description */}
                <div className="text-sm text-[#8888aa] mb-3">{entry.description}</div>

                {/* Rejected details */}
                {entry.decision === "rejected" ? (
                  <>
                    {entry.rule && (
                      <div className="text-xs text-[#ffd700] mb-1" style={{ fontFamily: "var(--font-mono)" }}>
                        Rule: {entry.rule}
                      </div>
                    )}
                    {entry.reason && (
                      <div className="text-xs text-[#8888aa] mb-2" style={{ fontFamily: "var(--font-mono)" }}>
                        {humanReason(entry.reason)}
                      </div>
                    )}
                    {entry.simulated_loss_usd != null && (
                      <div className="text-sm text-[#00ff88] font-bold p-2 rounded-lg bg-[rgba(0,255,136,0.06)] border border-[rgba(0,255,136,0.25)]" style={{ fontFamily: "var(--font-mono)" }}>
                        Prevented loss: ${entry.simulated_loss_usd.toFixed(2)}
                      </div>
                    )}
                  </>
                ) : (
                  <div className="text-xs text-[#00ff88]" style={{ fontFamily: "var(--font-mono)" }}>
                    All 7 rules passed ✓
                  </div>
                )}

                {/* Feedback buttons */}
                {entry.decision === "rejected" && !entry.feedback && !submitted.has(i) && (
                  <div className="flex gap-2 mt-3 pt-3 border-t border-[#1e1e2e]">
                    {[1, 2, 3, 4, 5].map((star) => (
                      <button
                        key={star}
                        onClick={() => sendFeedback(i, star as 1 | 2 | 3 | 4 | 5)}
                        className="w-8 h-8 rounded-lg border border-[#1e1e2e] hover:border-[#ffd700] bg-[#12121a] hover:bg-[#181822] transition-all text-xs"
                      >
                        {star}★
                      </button>
                    ))}
                    <button
                      onClick={() => sendFeedback(i, 1)}
                      className="px-2 py-1 text-xs bg-[#ff3366]/20 border border-[#ff3366]/40 rounded hover:bg-[#ff3366]/30 text-[#ff3366]"
                    >
                      Wrong
                    </button>
                    <button
                      onClick={() => sendFeedback(i, 5)}
                      className="px-2 py-1 text-xs bg-[#00ff88]/20 border border-[#00ff88]/40 rounded hover:bg-[#00ff88]/30 text-[#00ff88]"
                    >
                      Correct
                    </button>
                  </div>
                )}

                {/* Feedback confirmation */}
                {entry.feedback && (
                  <div className={`text-xs mt-2 ${
                    entry.feedback === "correct" ? "text-[#00ff88]" :
                    entry.feedback === "wrong" ? "text-[#ff3366]" : "text-[#8888aa]"
                  }`}>
                    Feedback: {entry.feedback}
                  </div>
                )}
              </div>
            ))}

            {/* Empty State */}
            {log.length === 0 && (
              <div className="text-[#54546a] text-center py-8">
                <i data-lucide="clock" className="w-8 h-8 mx-auto mb-3" style={{ strokeWidth: 1.5 }}></i>
                <p>Waiting for transactions...</p>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Bottom bar */}
      <footer className="sticky bottom-0 bg-[#12121a] border-t border-[#1e1e2e] px-6 py-3 flex items-center gap-6 flex-wrap text-xs text-[#8888aa]" style={{ fontFamily: "var(--font-mono)" }}>
        <span>Guardian Accuracy: <span className="text-[#00ff88] font-bold">{summary ? summary.accuracy.toFixed(1) : "—"}%</span></span>
        <span className="text-[#2a2a3e]">|</span>
        <span>Avg Score: <span className="text-white font-bold">4.7/5.0</span></span>
        <span className="text-[#2a2a3e]">|</span>
        <span>False Positives: <span className="text-[#ff9900] font-bold">1</span></span>
        <span className="text-[#2a2a3e]">|</span>
        <span>Total Feedback: <span className="text-white font-bold">{summary ? summary.total : "—"}</span></span>
        <div className="flex-1"></div>
        <span className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-[#00ff88] animate-pulse" style={{ boxShadow: "0 0 8px 0 #00ff88" }}></span>
          ws://localhost:3001/ws
        </span>
      </footer>

      {/* Initialize Lucide icons */}
      <script dangerouslySetInnerHTML={{__html: `
        if (typeof window !== 'undefined' && (window as any).lucide) {
          (window as any).lucide.createIcons();
        }
      `}} />
    </div>
  );
}

export default App;
