import { useEffect, useState } from "react";
import "./index.css";

interface LogEntry {
  timestamp: string;
  decision: "allowed" | "rejected";
  rule?: string;
  reason?: string;
  description?: string;
}

function App() {
  const [log, setLog] = useState<LogEntry[]>([]);
  const [allowed, setAllowed] = useState(0);
  const [blocked, setBlocked] = useState(0);

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
      } catch {
        // ignore malformed
      }
    };

    return () => ws.close();
  }, []);

  return (
    <div className="min-h-screen bg-gray-950 text-green-400 font-mono p-6">
      <header className="mb-6">
        <h1 className="text-3xl font-bold text-green-300">SAK-1 Guardian</h1>
        <p className="text-gray-500 text-sm mt-1">
          Live safety log — every transaction simulated before signing
        </p>
      </header>

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
