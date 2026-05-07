import { useEffect, useState } from "react";
import "./index.css";

interface LogEntry {
  timestamp: string;
  decision: "allowed" | "rejected";
  rule?: string;
  reason?: string;
  description?: string;
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
    <div className="min-h-screen bg-gray-950 text-green-400 font-mono p-6">
      <header className="mb-6">
        <h1 className="text-3xl font-bold text-green-300">SAK-1 Guardian</h1>
        <p className="text-gray-500 text-sm mt-1">
          Live safety log — every transaction simulated before signing
        </p>
      </header>

      {/* Feedback Summary Panel */}
      {summary && (
        <div className="mb-6 p-4 bg-gray-900 border border-gray-800 rounded">
          <h2 className="text-lg font-bold text-green-300 mb-2">Feedback Summary</h2>
          <div className="flex gap-4">
            <div>
              <span className="text-green-400 font-bold">{summary.correct}</span>
              <span className="text-gray-500 ml-2">Correct</span>
            </div>
            <div>
              <span className="text-red-400 font-bold">{summary.wrong}</span>
              <span className="text-gray-500 ml-2">Wrong</span>
            </div>
            <div>
              <span className="text-blue-400 font-bold">{summary.accuracy.toFixed(1)}%</span>
              <span className="text-gray-500 ml-2">Accuracy</span>
            </div>
          </div>
        </div>
      )}

      <div className="flex gap-4 mb-6">
        <div className="px-4 py-2 bg-green-950 border border-green-800 rounded">
          <span className="text-green-400 font-bold">{allowed}</span>
          <span className="text-gray-500 ml-2">Allowed</span>
        </div>
        <div className="px-4 py-2 bg-red-950 border border-red-800 rounded">
          <span className="text-red-400 font-bold">{blocked}</span>
          <span className="text-gray-500 ml-2">Blocked</span>
        </div>
      </div>

      <div className="space-y-2">
        {log.map((entry, i) => (
          <div
            key={i}
            className={`flex items-start gap-4 p-3 rounded border text-sm ${
              entry.decision === "rejected"
                ? "border-red-800 bg-red-950/30"
                : "border-green-800 bg-green-950/30"
            }`}
          >
            <span
              className={
                entry.decision === "rejected" ? "text-red-400" : "text-green-400"
              }
            >
              {entry.decision === "rejected" ? "BLOCKED" : "ALLOWED"}
            </span>
            <span className="text-gray-500">{entry.timestamp}</span>
            {entry.rule && (
              <span className="text-yellow-400">rule: {entry.rule}</span>
            )}
            {entry.reason && (
              <span className="text-gray-300">{entry.reason}</span>
            )}
            {entry.description && (
              <span className="text-gray-500">{entry.description}</span>
            )}

            {/* Feedback Buttons - only show if not yet submitted */}
            {!entry.feedback && !submitted.has(i) && (
              <div className="flex gap-2 ml-auto">
                {[1, 2, 3, 4, 5].map((star) => (
                  <button
                    key={star}
                    onClick={() => sendFeedback(i, star as 1 | 2 | 3 | 4 | 5)}
                    className="px-2 py-1 text-xs bg-gray-800 border border-gray-700 rounded hover:bg-gray-700"
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
              <span className={`ml-auto text-xs ${
                entry.feedback === "correct" ? "text-green-400" :
                entry.feedback === "wrong" ? "text-red-400" : "text-gray-500"
              }`}>
                Feedback: {entry.feedback}
              </span>
            )}
          </div>
        ))}
      </div>

      {log.length === 0 && (
        <p className="text-gray-600 mt-8 text-center">
          Waiting for transactions...
        </p>
      )}
    </div>
  );
}

export default App;
