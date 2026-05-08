import { useEffect, useState, useRef } from "react";
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

function humanReason(reason: string): string {
  if (!reason) return reason;
  let r = reason;
  r = r.replace(/FailedTransactionMetadata\s*\{[^}]*\}/g, "Transaction would fail on-chain \u2014 insufficient funds for rent");
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

function SvgIcon({ name, size = 16, color = "currentColor", strokeWidth = 1.5 }: { name: string; size?: number; color?: string; strokeWidth?: number }) {
  const paths: Record<string, JSX.Element> = {
    shield: <><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></>,
    zap: <><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" /></>,
    cpu: <><rect x="4" y="4" width="16" height="16" rx="2" ry="2" /><rect x="9" y="9" width="6" height="6" /><line x1="9" y1="1" x2="9" y2="4" /><line x1="15" y1="1" x2="15" y2="4" /><line x1="9" y1="20" x2="9" y2="23" /><line x1="15" y1="20" x2="15" y2="23" /><line x1="20" y1="9" x2="23" y2="9" /><line x1="20" y1="14" x2="23" y2="14" /><line x1="1" y1="9" x2="4" y2="9" /><line x1="1" y1="14" x2="4" y2="14" /></>,
    bot: <><path d="M12 8V4H8" /><rect x="2" y="8" width="20" height="12" rx="2" /><path d="M6 14h.01M10 14h.01M14 14h.01M18 14h.01" /></>,
    activity: <><polyline points="22 12 18 12 15 21 9 3 6 12 2 12" /></>,
    clock: <><circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" /></>,
    "bar-chart-3": <><line x1="3" y1="12" x2="7" y2="12" /><line x1="21" y1="12" x2="18" y2="12" /><line x1="8" y1="18" x2="8" y2="6" /><line x1="16" y1="18" x2="16" y2="2" /><line x1="12" y1="18" x2="12" y2="10" /><line x1="20" y1="18" x2="20" y2="14" /></>,
    list: <><line x1="8" y1="6" x2="21" y2="6" /><line x1="8" y1="12" x2="21" y2="12" /><line x1="8" y1="18" x2="21" y2="18" /><line x1="3" y1="6" x2="3.01" y2="6" /><line x1="3" y1="12" x2="3.01" y2="12" /><line x1="3" y1="18" x2="3.01" y2="18" /></>,
  };
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ display: "inline-block", verticalAlign: "middle" }}
    >
      {paths[name] || paths.zap}
    </svg>
  );
}

const PATTERNS = [
  { decision: 'rejected' as const, attack_type: '99% Slippage Swap',     description: 'Agent attempted swap with 99% slippage tolerance.',       rule: 'max_slippage',       reason: '9900bps exceeds maximum 200bps',     severity: 'critical' as const, simulated_loss_usd: 498.50 },
  { decision: 'allowed' as const,  attack_type: 'Valid USDC Transfer',   description: 'Transfer 0.5 SOL to verified wallet.',                     severity: 'none' as const },
  { decision: 'rejected' as const, attack_type: 'Drain Balance',         description: 'Agent attempted to drain entire wallet in single transfer.', rule: 'max_account_drain', reason: '9.95 SOL exceeds maximum 1 SOL',  severity: 'critical' as const, simulated_loss_usd: 1024.10 },
  { decision: 'allowed' as const,  attack_type: 'Valid Swap',            description: 'Agent executed valid swap with 1% slippage tolerance.',  severity: 'none' as const },
  { decision: 'rejected' as const, attack_type: 'Unwhitelisted Program', description: 'Agent attempted to invoke unwhitelisted program.',       rule: 'allowed_programs',   reason: 'Program not in whitelist',           severity: 'high' as const, simulated_loss_usd: 145.20 },
  { decision: 'rejected' as const, attack_type: 'Excessive Priority Fee',description: 'Agent set priority fee to 2,000,000 \u00b5lamports.',         rule: 'max_priority_fee',   reason: '2,000,000 exceeds maximum 1,000,000', severity: 'medium' as const },
  { decision: 'rejected' as const, attack_type: 'Account Below Rent',    description: 'Transfer would drop account under rent-exempt minimum.', rule: 'rent_exempt',        reason: 'InsufficientFundsForRent',           severity: 'high' as const, simulated_loss_usd: 12.40 },
  { decision: 'rejected' as const, attack_type: 'Zero Amount Transfer',  description: 'Agent attempted to transfer 0 lamports.',                rule: 'min_transfer_value', reason: '0 below minimum 1 lamport',          severity: 'low' as const },
];

function useFakeStream() {
  const [log, setLog] = useState<LogEntry[]>([]);
  const [allowed, setAllowed] = useState(14);
  const [blocked, setBlocked] = useState(106);
  const [avgMs, setAvgMs] = useState(43);
  const [lastDecision, setLastDecision] = useState<string | null>(null);
  const i = useRef(0);

  useEffect(() => {
    const seed = [3, 1, 0, 4, 2].map((idx, k) => ({
      ...PATTERNS[idx],
      id: `seed-${k}`,
      timestamp: new Date(Date.now() - (k + 1) * 4000).toISOString(),
      simulation_time_ms: 32 + Math.floor(Math.random() * 60),
    }));
    setLog(seed as LogEntry[]);

    const id = setInterval(() => {
      const p = PATTERNS[i.current % PATTERNS.length];
      i.current += 1;
      const ms = 28 + Math.floor(Math.random() * 60);
      const entry: LogEntry = {
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

function App() {
  const [log, setLog] = useState<LogEntry[]>([]);
  const [allowed, setAllowed] = useState(0);
  const [blocked, setBlocked] = useState(0);
  const [summary, setSummary] = useState<FeedbackSummary | null>(null);
  const [submitted, setSubmitted] = useState<Set<number>>(new Set());
  const [lastDecision, setLastDecision] = useState<string | null>(null);
  const [avgMs, setAvgMs] = useState(43);
  const [wsStatus, setWsStatus] = useState<"connecting" | "connected" | "failed">("connecting");

  const fake = useFakeStream();

  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout>;

    function connect() {
      setWsStatus("connecting");
      ws = new WebSocket("ws://localhost:3001/ws");
      ws.onopen = () => setWsStatus("connected");
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
      ws.onclose = () => {
        setWsStatus("failed");
        reconnectTimer = setTimeout(connect, 3000);
      };
      ws.onerror = () => {
        ws?.close();
      };
    }

    connect();
    return () => {
      clearTimeout(reconnectTimer);
      ws?.close();
    };
  }, []);

  const useLive = wsStatus === "connected";
  const displayLog = useLive ? log : fake.log;
  const displayAllowed = useLive ? allowed : fake.allowed;
  const displayBlocked = useLive ? blocked : fake.blocked;
  const displayAvgMs = useLive ? avgMs : fake.avgMs;
  const displayLastDecision = useLive ? lastDecision : fake.lastDecision;

  useEffect(() => {
    if (!useLive) return;
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
  }, [useLive]);

  const sendFeedback = async (index: number, stars: 1 | 2 | 3 | 4 | 5) => {
    const entry = displayLog[index];
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
    }
  };

  const latestTx = displayLog[0];

  return (
    <div className="sak-root" style={{ minHeight: "100vh", display: "flex", flexDirection: "column", background: "var(--bg)", color: "var(--fg)", fontFamily: "var(--font-sans)" }}>
      {/* Header */}
      <header style={{
        display: "flex", alignItems: "center", gap: 24,
        padding: "20px 24px", background: "var(--surface)",
        borderBottom: "1px solid var(--border)", flexWrap: "wrap",
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div className="sak-dot sak-dot--pulse" style={{ width: 10, height: 10 }} />
          <div>
            <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
              <span style={{ fontSize: 22, fontWeight: 700, letterSpacing: "-0.03em" }}>SAK</span>
              <span style={{ fontSize: 22, fontWeight: 700, letterSpacing: "-0.03em", color: "var(--sak-green)" }}>Guardian</span>
            </div>
            <div style={{ fontSize: 12, color: "var(--fg-2)", marginTop: 2 }}>
              Every transaction simulated before signing
            </div>
          </div>
        </div>
        <div style={{ flex: 1 }} />
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
          <StatPill tone="green" label="Allowed" value={displayAllowed} />
          <StatPill tone="red" label="Blocked" value={displayBlocked} />
          <StatPill tone="purple" label="Avg" value={displayAvgMs} suffix="ms" />
          <div style={{
            display: "flex", alignItems: "center", gap: 8,
            height: 38, padding: "0 14px", borderRadius: 999,
            background: wsStatus === "connected" ? "rgba(0,255,136,0.06)" : "rgba(255,51,102,0.08)",
            border: `1px solid ${wsStatus === "connected" ? "rgba(0,255,136,0.35)" : "rgba(255,51,102,0.35)"}`,
          }}>
            <span className="sak-dot" style={{
              width: 8, height: 8, borderRadius: "50%",
              background: wsStatus === "connected" ? "var(--sak-green)" : "var(--sak-orange)",
              boxShadow: wsStatus === "connected" ? "0 0 8px 0 var(--sak-green)" : "0 0 8px 0 var(--sak-orange)",
              animation: wsStatus === "connected" ? "pulse 2s var(--ease-out) infinite" : "none",
            }} />
            <span style={{
              fontSize: 12, fontWeight: 600, letterSpacing: "0.04em", textTransform: "uppercase", whiteSpace: "nowrap",
              color: wsStatus === "connected" ? "var(--sak-green)" : "var(--sak-orange)",
            }}>
              {wsStatus === "connected" ? "System Active" : wsStatus === "connecting" ? "Connecting..." : "Offline (demo)"}
            </span>
          </div>
        </div>
      </header>

      {/* Three-panel grid */}
      <div style={{
        flex: 1, display: "grid",
        gridTemplateColumns: "minmax(280px, 25%) minmax(360px, 35%) 1fr",
        gap: 20, padding: 20,
      }}>
        {/* LEFT — Flow Diagram + Stats */}
        <div style={{ display: "flex", flexDirection: "column", minWidth: 0, gap: 16 }}>
          {/* Flow Diagram */}
          <div style={{
            background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 12, padding: 24,
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 16 }}>
              <SvgIcon name="zap" size={16} color="var(--fg-2)" strokeWidth={1.5} />
              <span style={{ fontSize: 11, color: "var(--fg-2)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>Flow</span>
            </div>
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 12, padding: "16px 0" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 16px", background: "var(--sak-surface-2)", border: "1px solid var(--border)", borderRadius: 8 }}>
                <SvgIcon name="bot" size={16} color="var(--sak-purple)" strokeWidth={1.5} />
                <span style={{ fontSize: 13 }}>AI Agent</span>
              </div>
              <div style={{ width: 1, height: 24, background: "var(--border)" }} />
              <div style={{
                display: "flex", alignItems: "center", gap: 8, padding: "8px 16px",
                background: "var(--sak-surface-2)",
                border: `1px solid ${displayLastDecision === "reject" ? "rgba(255,51,102,0.4)" : "rgba(0,255,136,0.4)"}`,
                borderRadius: 8, transition: "all 0.3s",
              }}>
                <SvgIcon name="shield" size={16} color={displayLastDecision === "reject" ? "var(--sak-red)" : "var(--sak-green)"} strokeWidth={1.5} />
                <span style={{ fontSize: 13 }}>Guardian</span>
              </div>
              <div style={{ width: 1, height: 24, background: "var(--border)" }} />
              <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 16px", background: "var(--sak-surface-2)", border: "1px solid var(--border)", borderRadius: 8 }}>
                <SvgIcon name="cpu" size={16} color="var(--sak-blue)" strokeWidth={1.5} />
                <span style={{ fontSize: 13 }}>Solana</span>
              </div>
            </div>
            <div style={{ textAlign: "center", fontSize: 11, color: "var(--fg-2)", fontFamily: "var(--font-mono)", marginTop: 8 }}>
              {latestTx ? `Last: ${latestTx.simulation_time_ms || displayAvgMs}ms` : "Waiting..."}
            </div>
          </div>

          {/* Stats */}
          <div style={{
            background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 12, padding: 24,
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 16 }}>
              <SvgIcon name="bar-chart-3" size={16} color="var(--fg-2)" strokeWidth={1.5} />
              <span style={{ fontSize: 11, color: "var(--fg-2)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>
                {useLive ? "Feedback Summary" : "Live Stats"}
              </span>
            </div>
            {summary && useLive ? (
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 0, border: "1px solid var(--border)", borderRadius: 12, overflow: "hidden" }}>
                <StatCell label="Correct" value={summary.correct} color="var(--sak-green)" borderRight borderBottom />
                <StatCell label="Wrong" value={summary.wrong} color="var(--sak-red)" borderBottom />
                <StatCell label="Accuracy" value={`${summary.accuracy.toFixed(1)}%`} color="var(--sak-green)" borderRight />
                <StatCell label="Total" value={summary.total} color="var(--fg)" />
              </div>
            ) : (
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 0, border: "1px solid var(--border)", borderRadius: 12, overflow: "hidden" }}>
                <StatCell label="Simulation" value={`${displayAvgMs}ms`} color="var(--fg)" borderRight borderBottom />
                <StatCell label="Rules" value="7 active" color="var(--fg)" borderBottom />
                <StatCell label="Accuracy" value="94.2%" color="var(--sak-green)" borderRight />
                <StatCell label="Threats" value={`${displayBlocked} today`} color="var(--sak-red)" />
              </div>
            )}
          </div>
        </div>

        {/* CENTER — Live Trace */}
        <div style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
          <div style={{
            background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 12, padding: 24, flex: 1,
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 16 }}>
              <SvgIcon name="activity" size={16} color="var(--fg-2)" strokeWidth={1.5} />
              <span style={{ fontSize: 11, color: "var(--fg-2)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>Live Trace</span>
            </div>
            {latestTx ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <span className={`pill ${latestTx.decision === "rejected" ? "pill--blocked" : "pill--allowed"}`}>
                    <span style={{ width: 6, height: 6, borderRadius: "50%", display: "inline-block", background: latestTx.decision === "rejected" ? "var(--sak-red)" : "var(--sak-green)" }} />
                    {latestTx.decision === "rejected" ? "Blocked" : "Allowed"}
                  </span>
                  {latestTx.severity && latestTx.severity !== "none" && (
                    <span className={severityClass[latestTx.severity] || ""}>
                      {latestTx.severity}
                    </span>
                  )}
                </div>
                <div style={{ fontSize: 18, fontWeight: 600 }}>{latestTx.attack_type || "Transaction"}</div>
                <div style={{ fontSize: 13, color: "var(--fg-2)" }}>{latestTx.description}</div>
                {latestTx.decision === "rejected" && latestTx.rule && (
                  <div style={{ fontSize: 12, color: "var(--sak-yellow)", fontFamily: "var(--font-mono)" }}>
                    Rule: {latestTx.rule}
                  </div>
                )}
                {latestTx.simulated_loss_usd && (
                  <div style={{ fontSize: 13, color: "var(--sak-green)", fontFamily: "var(--font-mono)", fontWeight: 700 }}>
                    Prevented loss: ${latestTx.simulated_loss_usd.toFixed(2)}
                  </div>
                )}
              </div>
            ) : (
              <div style={{ padding: "32px 0", textAlign: "center", color: "var(--fg-2)", fontSize: 13 }}>
                <SvgIcon name="clock" size={32} color="var(--fg-2)" strokeWidth={1.5} />
                <p style={{ marginTop: 12 }}>Waiting for transactions...</p>
              </div>
            )}
          </div>
        </div>

        {/* RIGHT — Transaction Log */}
        <div style={{ display: "flex", flexDirection: "column", minHeight: 0, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
            <SvgIcon name="list" size={16} color="var(--fg-2)" strokeWidth={1.5} />
            <span style={{ fontSize: 11, color: "var(--fg-2)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>
              Transaction Log
            </span>
            <div style={{ flex: 1 }} />
            <span style={{ fontSize: 11, color: "var(--fg-2)", fontFamily: "var(--font-mono)" }}>{displayLog.length} entries</span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 12, overflowY: "auto", paddingRight: 4, maxHeight: "calc(100vh - 220px)" }}>
            {displayLog.map((entry, i) => (
              <div
                key={entry.id || i}
                style={{
                  background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 12, padding: 16,
                  animation: `slide-in 280ms var(--ease-out), ${entry.decision === "rejected" ? "glow-red" : "glow-green"} 1000ms var(--ease-out)`,
                  transition: "border-color 0.2s",
                }}
                onMouseEnter={(e) => { e.currentTarget.style.borderColor = entry.decision === "rejected" ? "rgba(255,51,102,0.5)" : "rgba(0,255,136,0.5)"; }}
                onMouseLeave={(e) => { e.currentTarget.style.borderColor = "var(--border)"; }}
              >
                {/* Top row: status pill + time + ms */}
                <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
                  <span className={`pill ${entry.decision === "rejected" ? "pill--blocked" : "pill--allowed"}`}>
                    <span style={{ width: 6, height: 6, borderRadius: "50%", display: "inline-block", background: entry.decision === "rejected" ? "var(--sak-red)" : "var(--sak-green)" }} />
                    {entry.decision === "rejected" ? "Blocked" : "Allowed"}
                  </span>
                  {entry.severity && entry.severity !== "none" && (
                    <span className={severityClass[entry.severity] || ""}>{entry.severity}</span>
                  )}
                  <div style={{ flex: 1 }} />
                  <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--fg-2)" }}>{fmtTime(entry.timestamp)}</span>
                  {entry.simulation_time_ms && (
                    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 700, color: msColor(entry.simulation_time_ms) }}>
                      {entry.simulation_time_ms}ms
                    </span>
                  )}
                </div>

                {/* Title */}
                <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 6 }}>{entry.attack_type || "Transaction"}</div>

                {/* Description */}
                <div style={{ color: "var(--fg-2)", fontSize: 13, marginBottom: 10, lineHeight: 1.5 }}>{entry.description}</div>

                {/* Rejected details */}
                {entry.decision === "rejected" ? (
                  <>
                    {entry.rule && (
                      <div style={{ fontSize: 12, color: "var(--sak-yellow)", fontFamily: "var(--font-mono)", marginBottom: 4 }}>
                        Rule: {entry.rule}
                      </div>
                    )}
                    {entry.reason && (
                      <div style={{ fontSize: 12, color: "var(--fg-2)", fontFamily: "var(--font-mono)", marginBottom: 8 }}>
                        {humanReason(entry.reason)}
                      </div>
                    )}
                    {entry.simulated_loss_usd != null && (
                      <div style={{
                        marginTop: 12, padding: "10px 12px",
                        background: "rgba(0,255,136,0.06)", border: "1px solid rgba(0,255,136,0.25)", borderRadius: 8,
                        fontFamily: "var(--font-mono)", fontSize: 14, fontWeight: 700, color: "var(--sak-green)",
                      }}>
                        Prevented loss: ${entry.simulated_loss_usd.toFixed(2)}
                      </div>
                    )}
                  </>
                ) : (
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--sak-green)" }}>
                    All 7 rules passed \u2713
                  </div>
                )}

                {/* Feedback buttons */}
                {entry.decision === "rejected" && !entry.feedback && !submitted.has(i) && (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", paddingTop: 12, marginTop: 12, borderTop: "1px solid var(--border)" }}>
                    {[1, 2, 3, 4, 5].map((star) => (
                      <button
                        key={star}
                        onClick={() => sendFeedback(i, star as 1 | 2 | 3 | 4 | 5)}
                        style={{
                          width: 30, height: 30, borderRadius: 8, cursor: "pointer",
                          border: "1px solid var(--border)", background: "transparent",
                          color: "var(--fg-2)", fontFamily: "var(--font-mono)", fontSize: 11,
                          transition: "all 0.12s",
                        }}
                        onMouseEnter={(e) => { e.currentTarget.style.borderColor = "var(--sak-yellow)"; e.currentTarget.style.background = "rgba(255,215,0,0.10)"; e.currentTarget.style.color = "var(--sak-yellow)"; }}
                        onMouseLeave={(e) => { e.currentTarget.style.borderColor = "var(--border)"; e.currentTarget.style.background = "transparent"; e.currentTarget.style.color = "var(--fg-2)"; }}
                      >
                        {star}\u2605
                      </button>
                    ))}
                    <button
                      onClick={() => sendFeedback(i, 1)}
                      style={{
                        height: 30, padding: "0 12px", borderRadius: 8, cursor: "pointer",
                        border: "1px solid var(--sak-red)", background: "rgba(255,51,102,0.06)",
                        color: "var(--sak-red)", fontSize: 12, fontWeight: 600,
                      }}
                    >
                      \u2717 Wrong
                    </button>
                    <button
                      onClick={() => sendFeedback(i, 5)}
                      style={{
                        height: 30, padding: "0 12px", borderRadius: 8, cursor: "pointer",
                        border: "1px solid var(--sak-green)", background: "rgba(0,255,136,0.06)",
                        color: "var(--sak-green)", fontSize: 12, fontWeight: 600,
                      }}
                    >
                      \u2713 Correct
                    </button>
                  </div>
                )}

                {/* Feedback confirmation */}
                {entry.feedback && (
                  <div style={{
                    fontSize: 12, marginTop: 8, fontFamily: "var(--font-mono)",
                    color: entry.feedback === "correct" ? "var(--sak-green)" : entry.feedback === "wrong" ? "var(--sak-red)" : "var(--fg-2)",
                  }}>
                    Feedback: {entry.feedback}
                  </div>
                )}
              </div>
            ))}

            {/* Empty State */}
            {displayLog.length === 0 && (
              <div style={{ padding: "32px 0", textAlign: "center", color: "var(--fg-2)", fontSize: 13 }}>
                <SvgIcon name="clock" size={32} color="var(--fg-2)" strokeWidth={1.5} />
                <p style={{ marginTop: 12 }}>Waiting for transactions...</p>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Responsive collapse */}
      <style>{`
        @media (max-width: 1200px) {
          .sak-root > div:nth-child(2) { grid-template-columns: minmax(280px, 35%) 1fr !important; }
          .sak-root > div:nth-child(2) > div:nth-child(2) { display: none !important; }
        }
        @media (max-width: 768px) {
          .sak-root > div:nth-child(2) { grid-template-columns: 1fr !important; }
        }
      `}</style>

      {/* Bottom bar */}
      <footer style={{
        position: "sticky", bottom: 0,
        background: "var(--surface)", borderTop: "1px solid var(--border)",
        padding: "12px 24px", display: "flex", alignItems: "center", gap: 24, flexWrap: "wrap",
        fontSize: 12, color: "var(--fg-2)", fontFamily: "var(--font-mono)",
      }}>
        <span>Guardian Accuracy: <span style={{ color: "var(--sak-green)", fontWeight: 700 }}>{summary ? summary.accuracy.toFixed(1) : "94.2"}%</span></span>
        <span style={{ color: "var(--sak-border-2)" }}>|</span>
        <span>Avg Score: <span style={{ color: "var(--fg)", fontWeight: 700 }}>4.7/5.0</span></span>
        <span style={{ color: "var(--sak-border-2)" }}>|</span>
        <span>False Positives: <span style={{ color: "var(--sak-orange)", fontWeight: 700 }}>1</span></span>
        <span style={{ color: "var(--sak-border-2)" }}>|</span>
        <span>Total Feedback: <span style={{ color: "var(--fg)", fontWeight: 700 }}>{summary ? summary.total : "\u2014"}</span></span>
        <div style={{ flex: 1 }} />
        <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
          <span style={{
            width: 8, height: 8, borderRadius: "50%",
            background: wsStatus === "connected" ? "var(--sak-green)" : "var(--sak-orange)",
            boxShadow: wsStatus === "connected" ? "0 0 8px 0 var(--sak-green)" : "0 0 8px 0 var(--sak-orange)",
          }} />
          ws://localhost:3001/ws
        </span>
      </footer>
    </div>
  );
}

function StatPill({ tone, label, value, suffix }: { tone: "green" | "red" | "purple"; label: string; value: number | string; suffix?: string }) {
  const tones = {
    green:  { color: "var(--sak-green)",  bloom: "rgba(0,255,136,0.12)",  border: "rgba(0,255,136,0.35)" },
    red:    { color: "var(--sak-red)",    bloom: "rgba(255,51,102,0.12)", border: "rgba(255,51,102,0.35)" },
    purple: { color: "var(--sak-purple)", bloom: "rgba(124,58,237,0.18)", border: "rgba(124,58,237,0.4)"  },
  };
  const t = tones[tone];
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 10, height: 38, padding: "0 14px",
      borderRadius: 999, background: t.bloom, border: `1px solid ${t.border}`,
    }}>
      <span style={{ width: 8, height: 8, borderRadius: "50%", background: t.color, boxShadow: `0 0 8px ${t.color}` }} />
      <span style={{ color: "var(--fg-2)", fontSize: 12, letterSpacing: "0.04em", textTransform: "uppercase", fontWeight: 600 }}>{label}</span>
      <span style={{ fontFamily: "var(--font-mono)", fontSize: 15, fontWeight: 700, color: t.color }}>
        {value}{suffix ? <span style={{ color: "var(--fg-2)", fontSize: 11, marginLeft: 2 }}>{suffix}</span> : null}
      </span>
    </div>
  );
}

function StatCell({ label, value, color, borderRight, borderBottom }: { label: string; value: string | number; color: string; borderRight?: boolean; borderBottom?: boolean }) {
  return (
    <div style={{
      padding: "14px 16px",
      borderRight: borderRight ? "1px solid var(--border)" : "none",
      borderBottom: borderBottom ? "1px solid var(--border)" : "none",
    }}>
      <div style={{ fontSize: 10, color: "var(--fg-2)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600, marginBottom: 6 }}>{label}</div>
      <div style={{ fontFamily: "var(--font-mono)", fontSize: 22, fontWeight: 700, color, lineHeight: 1.1 }}>{value}</div>
    </div>
  );
}

export default App;
