import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as commands from "./lib/tauri-commands";
import { useSession } from "./hooks/useSession";
import { SessionBar } from "./components/SessionBar";
import { ChatPanel } from "./components/ChatPanel";
import { ActionApproval } from "./components/ActionApproval";
import { ChangeLog } from "./components/ChangeLog";
import { SessionHistory } from "./components/SessionHistory";
import { DebugPanel } from "./components/DebugPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdateBanner } from "./components/UpdateBanner";
import { SetupScreen } from "./components/SetupScreen";
import { useDebugStore, type DebugEvent } from "./stores/debugStore";

const WINDOW_TITLES = [
  "Noah \u2014 Your Trusted Support",
  "Noah \u2014 The \u201CComputer\u201D Guy",
  "Noah \u2014 Have You Tried Turning It Off?",
  "Noah \u2014 No Ticket Required",
  "Noah \u2014 I Won\u2019t Judge Your Browser Tabs",
  "Noah \u2014 Fixing Things Since Forever",
  "Noah \u2014 Like a Friend Who\u2019s Good With Computers",
  "Noah \u2014 Less Jargon, More Fixing",
];

function App() {
  const [needsSetup, setNeedsSetup] = useState<boolean | null>(null);

  // Check for API key on mount.
  useEffect(() => {
    commands.hasApiKey().then((has) => setNeedsSetup(!has));
  }, []);

  // Show nothing while checking.
  if (needsSetup === null) return null;

  // Show setup screen if no API key configured.
  if (needsSetup) {
    return <SetupScreen onComplete={() => setNeedsSetup(false)} />;
  }

  return <MainApp />;
}

function MainApp() {
  // Single hook instance that auto-creates session on mount
  const session = useSession();
  const addEvent = useDebugStore((s) => s.addEvent);
  const toggle = useDebugStore((s) => s.toggle);

  // Set a random cheeky window title on mount.
  useEffect(() => {
    const title = WINDOW_TITLES[Math.floor(Math.random() * WINDOW_TITLES.length)];
    getCurrentWindow().setTitle(title).catch(() => {});
  }, []);

  // Listen for debug-log events from the Rust backend.
  useEffect(() => {
    const unlisten = listen<DebugEvent>("debug-log", (e) => {
      addEvent(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [addEvent]);

  // Cmd+D / Ctrl+D keyboard shortcut to toggle debug panel.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "d") {
        e.preventDefault();
        toggle();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [toggle]);

  return (
    <div className="flex flex-col h-screen bg-bg-primary text-text-primary">
      <UpdateBanner />
      <SessionBar session={session} />
      <ChatPanel />
      <DebugPanel />
      <SettingsPanel />
      <ActionApproval />
      <ChangeLog />
      <SessionHistory />
    </div>
  );
}

export default App;
