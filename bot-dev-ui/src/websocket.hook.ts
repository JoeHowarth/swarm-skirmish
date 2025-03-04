import { useState, useRef, useEffect } from "react";

// Add proper type definitions for your data structures
export interface JournalEntry {
  timestamp: string;
  bot_id?: number;
  client_msg?: any;
  server_msg?: any;
  // Add other fields as needed
}

export interface BotLog {
  [botId: string]: string[];
}

export function useGameData() {
    const [connected, setConnected] = useState(false);
    const [journalEntries, setJournalEntries] = useState<JournalEntry[]>([]);
    const [botLogs, setBotLogs] = useState<BotLog>({});
    const [activeBots, setActiveBots] = useState<number[]>([]);
    const wsRef = useRef<WebSocket | null>(null);
  
    // Connect to WebSocket server
    useEffect(() => {
      const ws = new WebSocket('ws://localhost:8080');
      
      ws.onopen = () => {
        console.log('Connected to log monitor');
        setConnected(true);
      };
      
      ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        
        if (data.type === 'journal') {
          setJournalEntries(prev => [...prev, data.entry]);
          
          // Track active bots
          if (data.entry.bot_id && !activeBots.includes(data.entry.bot_id)) {
            setActiveBots(prev => [...prev, data.entry.bot_id]);
          }
          
        } else if (data.type === 'botlog') {
          const { bot_id, content } = data;
          setBotLogs(prev => ({
            ...prev,
            [bot_id]: [...(prev[bot_id] || []), content]
          }));
        }
      };
      
      ws.onclose = () => {
        console.log('Disconnected from log monitor');
        setConnected(false);
      };
      
      wsRef.current = ws;
      return () => ws.close();
    }, []);
  
    return {
      connected,
      journalEntries,
      botLogs,
      activeBots
    };
  }