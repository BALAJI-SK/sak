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

function App() {
  const [log, setLog] = useState<LogEntry[]>([]);
  const [allowed, setAllowed] = useState(0);
  const [blocked, setBlocked] = useState(0);
  const [summary, setSummary] = useState<FeedbackSummary | null>(null);
  const [submitted, setSubmitted] = useState<Set<number>>(new Set());

  useEffect(() => {
    const ws = new WebSocket("ws://localhost:3001/ws");

    ws.onmessage = (e) => {
      try {
        const entry: LogEntry = JSON.parse(e.data);
        setLog((prev) => [{ ...entry, feedback: undefined }, ...prev].slice(0, 50));
        if (entry.decision === "allowed") {
          setAllowed((v) => v + 1);
        } else {
          setBlocked((v) => v + 1);
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

  return (
    <div className="min-h-screen bg-gray-950 text-white font-sans antialiased">
      <div className="max-w-7xl mx-auto p-6">
        {/* Header */}
        <header className="mb-8">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h1 className="text-4xl font-bold bg-gradient-to-r from-green-400 to-emerald-300 bg-clip-text text-transparent tracking-tight">
                SAK Guardian
              </h1>
              <p className="text-gray-500 text-sm mt-2 tracking-wide">
                Live safety log — every transaction simulated before signing
              </p>
            </div>
            <div className="flex items-center gap-2 px-4 py-2 bg-gray-900 border border-gray-800 rounded-full">
              <div className="w-2 h-2 bg-green-400 rounded-full animate-pulse"></div>
              <span className="text-xs text-gray-400">System Active</span>
            </div>
          </div>
        </header>

        {/* Feedback Summary Panel */}
        {summary && (
          <div className="mb-8 bg-gray-900 border border-gray-800 rounded-2xl p-6 relative overflow-hidden">
            <div className="absolute top-0 right-0 w-32 h-32 bg-blue-400/5 rounded-full blur-3xl"></div>
            <h2 className="text-lg font-semibold text-white mb-4 flex items-center gap-2 relative">
              <i data-lucide="bar-chart-3" className="w-5 h-5 text-gray-400" style={{strokeWidth: 1.5}}></i>
              Feedback Summary
            </h2>
            <div className="grid grid-cols-3 gap-4">
              <div className="text-center">
                <div className="text-3xl font-bold text-green-400">{summary.correct}</div>
                <div className="text-xs text-gray-500 mt-1">Correct</div>
              </div>
              <div className="text-center">
                <div className="text-3xl font-bold text-red-400">{summary.wrong}</div>
                <div className="text-xs text-gray-500 mt-1">Wrong</div>
              </div>
              <div className="text-center">
                <div className="text-3xl font-bold text-blue-400">{summary.accuracy.toFixed(1)}%</div>
                <div className="text-xs text-gray-500 mt-1">Accuracy</div>
              </div>
            </div>
          </div>
        )}

        {/* Stats Cards */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
          <div className="relative overflow-hidden bg-gray-900 border border-gray-800 rounded-2xl p-6 group hover:border-green-700/50 transition-all duration-300">
            <div className="absolute top-0 right-0 w-32 h-32 bg-green-400/5 rounded-full blur-3xl group-hover:bg-green-400/10 transition-all"></div>
            <div className="flex items-center gap-3 mb-4 relative">
              <div className="p-2 bg-green-950 border border-green-800 rounded-lg">
                <i data-lucide="check-circle" className="w-5 h-5 text-green-400" style={{strokeWidth: 1.5}}></i>
              </div>
              <span className="text-gray-500 text-sm font-medium">Allowed</span>
            </div>
            <div className="text-5xl font-bold text-white mb-1" id="allowed-count">{allowed}</div>
            <div className="text-xs text-gray-500">Transactions approved</div>
          </div>

          <div className="relative overflow-hidden bg-gray-900 border border-gray-800 rounded-2xl p-6 group hover:border-red-700/50 transition-all duration-300">
            <div className="absolute top-0 right-0 w-32 h-32 bg-red-400/5 rounded-full blur-3xl group-hover:bg-red-400/10 transition-all"></div>
            <div className="flex items-center gap-3 mb-4 relative">
              <div className="p-2 bg-red-950 border border-red-800 rounded-lg">
                <i data-lucide="x-circle" className="w-5 h-5 text-red-400" style={{strokeWidth: 1.5}}></i>
              </div>
              <span className="text-gray-500 text-sm font-medium">Blocked</span>
            </div>
            <div className="text-5xl font-bold text-white mb-1" id="blocked-count">{blocked}</div>
            <div className="text-xs text-gray-500">Threats prevented</div>
          </div>

          <div className="relative overflow-hidden bg-gray-900 border border-gray-800 rounded-2xl p-6 group hover:border-blue-700/50 transition-all duration-300">
            <div className="absolute top-0 right-0 w-32 h-32 bg-blue-400/5 rounded-full blur-3xl group-hover:bg-blue-400/10 transition-all"></div>
            <div className="flex items-center gap-3 mb-4 relative">
              <div className="p-2 bg-blue-950 border border-blue-800 rounded-lg">
                <i data-lucide="target" className="w-5 h-5 text-blue-400" style={{strokeWidth: 1.5}}></i>
              </div>
              <span className="text-gray-500 text-sm font-medium">Accuracy</span>
            </div>
            <div className="text-5xl font-bold text-white mb-1" id="accuracy-score">
              {summary ? `${summary.accuracy.toFixed(1)}%` : "—%"}
            </div>
            <div className="text-xs text-gray-500">AI decision accuracy</div>
          </div>
        </div>

        {/* Transaction Log */}
        <div>
          <h2 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
            <i data-lucide="activity" className="w-5 h-5 text-gray-400" style={{strokeWidth: 1.5}}></i>
            Transaction Log
          </h2>
          <div className="space-y-3">
            {log.map((entry, i) => (
              <div
                key={entry.id || i}
                className={`flex items-start gap-4 p-4 rounded-xl border transition-all duration-200 ${
                  entry.decision === "rejected"
                    ? "border-red-800/50 bg-red-950/20 hover:bg-red-950/30"
                    : "border-green-800/50 bg-green-950/20 hover:bg-green-950/30"
                }`}
              >
                <div className={`flex-shrink-0 w-12 h-12 rounded-lg flex items-center justify-center ${
                  entry.decision === "rejected" ? "bg-red-950 border border-red-800" : "bg-green-950 border border-green-800"
                }`}>
                  <i data-lucide={entry.decision === "rejected" ? "shield-alert" : "shield-check"}
                     className={`w-6 h-6 ${entry.decision === "rejected" ? "text-red-400" : "text-green-400"}`}
                     style={{strokeWidth: 1.5}}
                   ></i>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1 flex-wrap">
                    <span className={`font-semibold ${entry.decision === "rejected" ? "text-red-400" : "text-green-400"}`}>
                      {entry.decision === "rejected" ? "BLOCKED" : "ALLOWED"}
                    </span>
                    {entry.severity && entry.severity !== "none" && (
                      <span className={`text-xs px-2 py-0.5 rounded-full ${
                        entry.severity === "critical" ? "bg-red-950 text-red-400 border border-red-800" :
                        entry.severity === "high" ? "bg-orange-950 text-orange-400 border border-orange-800" :
                        entry.severity === "medium" ? "bg-yellow-950 text-yellow-400 border border-yellow-800" :
                        "bg-gray-950 text-gray-400 border border-gray-800"
                      }`}>
                        {entry.severity}
                      </span>
                    )}
                    {entry.attack_type && (
                      <span className="text-gray-400 text-xs">{entry.attack_type}</span>
                    )}
                    <span className="text-gray-600 text-xs ml-auto">{entry.timestamp}</span>
                  </div>
                  {entry.rule && (
                    <div className="text-yellow-400 text-sm mb-1">Rule: {entry.rule}</div>
                  )}
                  {entry.reason && (
                    <div className="text-gray-300 text-sm mb-1">{entry.reason}</div>
                  )}
                  {entry.description && (
                    <div className="text-gray-500 text-xs">{entry.description}</div>
                  )}
                  {(entry.simulated_loss_usd || entry.simulation_time_ms) && (
                    <div className="flex gap-4 mt-2 text-xs text-gray-600">
                      {entry.simulated_loss_usd && (
                        <span className="text-red-400/70">
                          Potential loss: ${entry.simulated_loss_usd.toFixed(2)}
                        </span>
                      )}
                      {entry.simulation_time_ms && (
                        <span>
                          Detected in {entry.simulation_time_ms}ms
                        </span>
                      )}
                    </div>
                  )}
                </div>
                <div className="flex-shrink-0 flex gap-2">
                  {/* Feedback Buttons - only show if not yet submitted */}
                  {entry.decision === "rejected" && !entry.feedback && !submitted.has(i) && (
                    <div className="flex gap-2">
                      {[1, 2, 3, 4, 5].map((star) => (
                        <button
                          key={star}
                          onClick={() => sendFeedback(i, star as 1 | 2 | 3 | 4 | 5)}
                          className="w-8 h-8 rounded-lg border border-gray-700 hover:border-yellow-600 bg-gray-900 hover:bg-gray-800 transition-all text-xs"
                        >
                          {star}★
                        </button>
                      ))}
                      <button
                        onClick={() => sendFeedback(i, 1)}
                        className="px-2 py-1 text-xs bg-red-900 border border-red-700 rounded hover:bg-red-800 text-red-300"
                      >
                        Wrong
                      </button>
                      <button
                        onClick={() => sendFeedback(i, 5)}
                        className="px-2 py-1 text-xs bg-green-900 border border-green-700 rounded hover:bg-green-800 text-green-300"
                      >
                        Correct
                      </button>
                    </div>
                  )}

                  {/* Show confirmation after feedback */}
                  {entry.feedback && (
                    <span className={`text-xs ml-auto ${
                      entry.feedback === "correct" ? "text-green-400" :
                      entry.feedback === "wrong" ? "text-red-400" : "text-gray-500"
                    }`}>
                      Feedback: {entry.feedback}
                    </span>
                  )}
                </div>
              </div>
            ))}

            {/* Empty State */}
            {log.length === 0 && (
              <div className="text-gray-600 text-center py-8">
                <i data-lucide="clock" className="w-8 h-8 mx-auto mb-3 text-gray-700" style={{strokeWidth: 1.5}}></i>
                <p>Waiting for transactions...</p>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Initialize Lucide icons */}
      <script dangerouslySetInnerHTML={{__html: `
        if (typeof lucide !== 'undefined') {
          lucide.createIcons();
        }
      `}} />
    </div>
  );
}

export default App;
