import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSession } from "./hooks/useSession";
import { SessionBar } from "./components/SessionBar";
import { ChatPanel } from "./components/ChatPanel";
import { ActionApproval } from "./components/ActionApproval";
import { ChangeLog } from "./components/ChangeLog";
import { SessionHistory } from "./components/SessionHistory";
import { DebugPanel } from "./components/DebugPanel";
import { useDebugStore, type DebugEvent } from "./stores/debugStore";

function App() {
  // Single hook instance that auto-creates session on mount
  const session = useSession();
  const addEvent = useDebugStore((s) => s.addEvent);
  const toggle = useDebugStore((s) => s.toggle);

  // Listen for debug-log events from the Rust backend.
  useEffect(() => {
    const unlisten = listen<DebugEvent>("debug-log", (e) => {
      addEvent(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [addEvent]);

  // Cmd+D keyboard shortcut to toggle debug panel.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "d") {
        e.preventDefault();
        toggle();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [toggle]);

  return (
    <div className="flex flex-col h-screen bg-bg-primary text-text-primary">
      <SessionBar session={session} />
      <ChatPanel />
      <DebugPanel />
      <ActionApproval />
      <ChangeLog />
      <SessionHistory />
    </div>
  );
}

export default App;
