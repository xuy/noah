import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as commands from "./lib/tauri-commands";
import { useSession } from "./hooks/useSession";
import { SessionBar } from "./components/SessionBar";
import { ChatPanel } from "./components/ChatPanel";
import { ActionApproval } from "./components/ActionApproval";
import { Sidebar } from "./components/Sidebar";
import { KnowledgeView } from "./components/KnowledgePanel";
import { DebugPanel } from "./components/DebugPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdateBanner } from "./components/UpdateBanner";
import { ProactiveSuggestionBanner } from "./components/ProactiveSuggestionBanner";
import { SessionSummary } from "./components/SessionSummary";
import { useSessionStore } from "./stores/sessionStore";
import { SetupScreen } from "./components/SetupScreen";
import { useDebugStore, type DebugEvent } from "./stores/debugStore";
import { useTheme } from "./hooks/useTheme";

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

function dismissSplash() {
  const splash = document.getElementById("splash");
  if (splash) {
    splash.classList.add("fade-out");
    setTimeout(() => splash.remove(), 300);
  }
}

function App() {
  const [needsSetup, setNeedsSetup] = useState<boolean | null>(null);
  useTheme(); // Apply saved theme on mount (before setup screen too)

  // Check for API key on mount.
  useEffect(() => {
    commands.hasApiKey().then((has) => {
      setNeedsSetup(!has);
      dismissSplash();
    });
  }, []);

  // Show nothing while checking (splash is still visible).
  if (needsSetup === null) return null;

  // Show setup screen if no API key configured.
  if (needsSetup) {
    return <SetupScreen onComplete={() => setNeedsSetup(false)} />;
  }

  return <MainApp />;
}

function MainApp() {
  const session = useSession();
  const activeView = useSessionStore((s) => s.activeView);
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
    <div className="flex h-screen bg-bg-primary text-text-primary">
      {/* Left sidebar */}
      <Sidebar session={session} />

      {/* Main content */}
      <div className="flex flex-col flex-1 min-w-0">
        <UpdateBanner />
        <ProactiveSuggestionBanner />
        <SessionBar />
        <SessionSummary />
        {activeView === "knowledge" ? <KnowledgeView /> : <ChatPanel />}
        <DebugPanel />
        <SettingsPanel />
        <ActionApproval />
      </div>
    </div>
  );
}

export default App;
