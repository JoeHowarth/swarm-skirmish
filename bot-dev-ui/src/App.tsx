import { useState, useRef, useEffect } from "react";
import { Action, ActionEnvelope, ActionStatus, JournalEntry } from "./types";

export default function App() {
  const { connected, botJournals } = useGameData();

  return (
    <div className="app">
      <header>
        <h1>Swarm Skirmish UI</h1>
        <div
          className={`connection-status ${
            connected ? "connected" : "disconnected"
          }`}
        >
          {connected ? "Connected" : "Disconnected"}
        </div>
      </header>

      <main className="bot-container">
        {Object.keys(botJournals).length === 0 ? (
          <div className="no-bots">No bot data available yet...</div>
        ) : (
          Object.entries(botJournals).map(([botId, entries]) => (
            <BotCard key={botId} botId={botId} entries={entries} />
          ))
        )}
      </main>

      <style>{`
        body {
          background-color: #222;
          margin: 0;
          padding: 0;
          color: #333;
        }
        
        .app {
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
            Oxygen, Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
          max-width: 1200px;
          margin: 0 auto;
          padding: 20px;
          color: #f5f5f7;
        }

        header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 20px;
          padding-bottom: 10px;
          border-bottom: 1px solid #444;
        }

        h1 {
          color: #f5f5f7;
          margin: 0;
        }

        .connection-status {
          padding: 6px 12px;
          border-radius: 4px;
          font-weight: bold;
        }

        .connected {
          background-color: #d4edda;
          color: #155724;
        }

        .disconnected {
          background-color: #f8d7da;
          color: #721c24;
        }

        .bot-container {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
          gap: 20px;
        }

        .no-bots {
          grid-column: 1 / -1;
          text-align: center;
          padding: 40px;
          background-color: #333;
          border-radius: 8px;
          color: #f5f5f7;
        }
        
        .bot-card {
          background-color: #2d2d2d;
          border-radius: 8px;
          box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
          padding: 16px;
          overflow: hidden;
          color: #f5f5f7;
        }

        .bot-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 16px;
          padding-bottom: 8px;
          border-bottom: 1px solid #444;
        }

        .bot-header h2 {
          margin: 0;
          font-size: 1.5rem;
          color: #f5f5f7;
        }

        .team-badge {
          padding: 4px 8px;
          border-radius: 4px;
          font-size: 0.8rem;
          font-weight: bold;
        }

        .team-badge.player {
          background-color: #0066cc;
          color: white;
        }

        .team-badge.enemy {
          background-color: #cc3300;
          color: white;
        }

        .bot-state,
        .action-history {
          margin-bottom: 16px;
        }

        h3 {
          font-size: 1.1rem;
          margin-top: 0;
          margin-bottom: 8px;
          color: #f5f5f7;
        }

        .state-item {
          margin-bottom: 8px;
          display: flex;
          flex-wrap: wrap;
          color: #e0e0e0;
        }

        .label {
          font-weight: bold;
          margin-right: 8px;
          min-width: 80px;
          color: #a0a0a0;
        }

        .value {
          color: #f5f5f7;
        }

        .items-list {
          margin: 0;
          padding-left: 20px;
          color: #e0e0e0;
        }

        .action-list {
          list-style: none;
          padding: 0;
          margin: 0;
          max-height: 200px;
          overflow-y: auto;
          background-color: #333;
          border-radius: 4px;
        }

        .action-item {
          padding: 8px;
          margin-bottom: 6px;
          border-radius: 4px;
          background-color: #3a3a3a;
          border-left: 4px solid #6c757d;
          color: #e0e0e0;
        }

        .action-item.success {
          border-left-color: #28a745;
        }

        .action-item.failure {
          border-left-color: #dc3545;
        }

        .action-item.inprogress {
          border-left-color: #17a2b8;
        }

        .action-details {
          display: flex;
          justify-content: space-between;
        }

        .action-type {
          color: #f5f5f7;
        }

        .action-status {
          color: #a0a0a0;
        }

        .action-id {
          font-size: 0.8rem;
          color: #a0a0a0;
          margin-top: 4px;
        }

        .no-actions {
          color: #a0a0a0;
          font-style: italic;
          padding: 8px;
        }
      `}</style>
    </div>
  );
}

// Bot Card Component to display individual bot information
function BotCard({
  botId,
  entries,
}: {
  botId: string;
  entries: JournalEntry[];
}) {
  // Get the latest state information
  const latestState = getLatestBotState(entries);

  // Get action history
  const actionHistory = getActionHistory(entries);

  return (
    <div className="bot-card">
      <div className="bot-header">
        <h2>Bot #{botId}</h2>
        <div className="bot-team">
          {latestState.team && (
            <span className={`team-badge ${latestState.team.toLowerCase()}`}>
              {latestState.team}
            </span>
          )}
        </div>
      </div>

      <div className="bot-state">
        <h3>Current State</h3>
        {latestState.position && (
          <div className="state-item">
            <span className="label">Position:</span>
            <span className="value">
              ({latestState.position.x}, {latestState.position.y})
            </span>
          </div>
        )}

        {latestState.tick !== undefined && (
          <div className="state-item">
            <span className="label">Last Tick:</span>
            <span className="value">{latestState.tick}</span>
          </div>
        )}

        {latestState.items && Object.keys(latestState.items).length > 0 && (
          <div className="state-item">
            <span className="label">Items:</span>
            <ul className="items-list">
              {Object.entries(latestState.items).map(([item, count]) => (
                <li key={item}>
                  {item}: {count}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      <div className="action-history">
        <h3>Action History</h3>
        {actionHistory.length === 0 ? (
          <div className="no-actions">No actions recorded yet</div>
        ) : (
          <ul className="action-list">
            {actionHistory.map((action, index) => (
              <li
                key={index}
                className={`action-item ${action.status?.toLowerCase() || ""}`}
              >
                <div className="action-details">
                  <span className="action-type">
                    {formatAction(action.action)}
                  </span>
                  <span className="action-status">
                    {action.status || "Pending"}
                  </span>
                </div>
                <div className="action-id">ID: {action.id}</div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

// Helper interface for bot state
interface BotState {
  team?: string;
  position?: { x: number; y: number };
  tick?: number;
  items?: Record<string, number>;
}

// Helper interface for action history
interface ActionHistoryItem {
  id: number;
  action: Action;
  status: ActionStatus;
  timestamp: string;
}

// Helper function to extract the latest state for a bot
function getLatestBotState(entries: JournalEntry[]): BotState {
  const state: BotState = {};

  // Process entries in reverse to find the latest state
  for (let i = entries.length - 1; i >= 0; i--) {
    const entry = entries[i];

    if (entry.server_msg && "ServerUpdate" in entry.server_msg) {
      const update = entry.server_msg.ServerUpdate.response;

      // Only set properties that haven't been set yet
      if (update.team && state.team === undefined) {
        state.team = update.team;
      }

      if (update.position && state.position === undefined) {
        let [x, y] = update.position;
        state.position = { x, y };
      }

      if (update.tick !== undefined && state.tick === undefined) {
        state.tick = update.tick;
      }

      if (
        update.items &&
        Object.keys(update.items).length > 0 &&
        state.items === undefined
      ) {
        state.items = update.items;
      }

      // If we've found all the state we need, break
      if (
        state.team &&
        state.position &&
        state.tick !== undefined &&
        state.items
      ) {
        break;
      }
    }
  }

  return state;
}

// Helper function to extract action history
function getActionHistory(entries: JournalEntry[]): ActionHistoryItem[] {
  const actionHistory: ActionHistoryItem[] = [];
  const actionResults: Record<number, ActionStatus> = {};

  // First pass: collect action results
  entries.forEach((entry: JournalEntry) => {
    if (
      entry.server_msg &&
      "ServerUpdate" in entry.server_msg &&
      entry.server_msg.ServerUpdate.response.action_result
    ) {
      const result = entry.server_msg.ServerUpdate.response.action_result;
      actionResults[result.id] = result.status;
    }
  });

  // Second pass: collect actions and match with results
  entries.forEach((entry) => {
    if (
      entry.client_msg &&
      "BotMsg" in entry.client_msg &&
      entry.client_msg.BotMsg.msg.actions
    ) {
      const actions = entry.client_msg.BotMsg.msg.actions;
      actions.forEach((action: ActionEnvelope) => {
        actionHistory.push({
          id: action.id,
          action: action.action,
          status: actionResults[action.id],
          timestamp: entry.timestamp,
        });
      });
    }
  });

  // Sort by timestamp (newest first)
  return actionHistory.sort(
    (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
  );
}

// Helper function to format action for display
function formatAction(action: Action): string {
  if (!action) return "Unknown";

  // Use type guards to check the shape of the action
  if ("MoveDir" in action) {
    return `Move ${action.MoveDir}`;
  }

  if ("MoveTo" in action) {
    return `Move to (${action.MoveTo[0]}, ${action.MoveTo[1]})`;
  }

  return JSON.stringify(action);
}

export function useGameData() {
  const [connected, setConnected] = useState(false);
  const [botJournals, setBotJournals] = useState<
    Record<string, JournalEntry[]>
  >({});
  const pollingIntervalRef = useRef<number | null>(null);

  useEffect(() => {
    // Function to fetch journal data
    const fetchJournals = async () => {
      try {
        const response = await fetch("http://localhost:3000/journals");

        if (!response.ok) {
          throw new Error(`HTTP error! Status: ${response.status}`);
        }

        const data = (await response.json()) as Record<string, JournalEntry[]>;
        console.log(data);
        setBotJournals(data);
        setConnected(true);
      } catch (error) {
        console.error("Failed to fetch journals:", error);
        setConnected(false);
      }
    };

    // Initial fetch
    fetchJournals();

    // Set up polling interval (100ms)
    pollingIntervalRef.current = window.setInterval(fetchJournals, 1000);

    // Clean up interval on unmount
    return () => {
      if (pollingIntervalRef.current !== null) {
        clearInterval(pollingIntervalRef.current);
      }
    };
  }, []);

  return {
    connected,
    botJournals,
  };
}
