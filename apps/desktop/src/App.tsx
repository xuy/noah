import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as commands from "./lib/tauri-commands";
import { useSession } from "./hooks/useSession";
import { ChatPanel } from "./components/ChatPanel";
import { MainTitleBar } from "./components/MainTitleBar";
import { ActionApproval } from "./components/ActionApproval";
import { Sidebar } from "./components/Sidebar";
import { KnowledgeView } from "./components/KnowledgePanel";
import { DebugPanel } from "./components/DebugPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { HealthDashboard } from "./components/HealthDashboard";
import { UpdateBanner } from "./components/UpdateBanner";
import { ProactiveSuggestionBanner } from "./components/ProactiveSuggestionBanner";
import { SessionSummary } from "./components/SessionSummary";
import { useSessionStore } from "./stores/sessionStore";
import { TilePickerScreen } from "./components/TilePickerScreen";
import { SubscribeModal } from "./components/SubscribeModal";
import { useDebugStore, type DebugEvent } from "./stores/debugStore";
import { useConsumerStore } from "./stores/consumerStore";
import { useTheme } from "./hooks/useTheme";
import { useZoom } from "./hooks/useZoom";

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

function extractDeepLinkToken(url: string): string | null {
  try {
    return new URL(url).searchParams.get("token");
  } catch {
    const m = url.match(/[?&]token=([^&]+)/);
    return m && m[1] ? decodeURIComponent(m[1]) : null;
  }
}

function App() {
  const [needsSetup, setNeedsSetup] = useState<boolean | null>(null);
  useTheme(); // Apply saved theme on mount (before setup screen too)

  // First-launch gate: show the TilePicker onboarding only when the
  // user has never interacted with Noah before. With device-first
  // identity, auth is no longer a hard gate — a fresh anonymous
  // device can start a trial immediately from inside MainApp. The
  // tile picker is purely UX (helps seed the first question) and
  // becomes redundant once the user has any chat history.
  //
  // Also ensures a device id exists in the Keychain (no-op on
  // subsequent launches) so backend calls can authenticate even
  // when the user is not signed in.
  useEffect(() => {
    commands.consumerEnsureDeviceId().catch(() => {});
    Promise.all([
      commands.listSessions().catch(() => [] as unknown[]),
      commands.hasApiKey().catch(() => false),
    ])
      .then(([sessions, hasKey]) => {
        const hasPastUsage = Array.isArray(sessions) && sessions.length > 0;
        setNeedsSetup(!hasPastUsage && !hasKey);
      })
      .finally(() => {
        dismissSplash();
      });
  }, []);

  // Global deep-link handler — registered at the app root so it fires
  // regardless of which screen is currently mounted. When a
  // `noah://auth?token=…` URL arrives (from clicking a magic link,
  // or from `open noah://…` in the terminal), finish sign-in and
  // dismiss the gate — don't drop the user back on the tile picker.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onOpenUrl(async (urls) => {
      const url = urls.find((u) => u.startsWith("noah://auth"));
      if (!url) return;
      const token = extractDeepLinkToken(url);
      if (!token) return;
      try {
        await commands.consumerCompleteSignIn(token);
        setNeedsSetup(false);
      } catch {
        // Stale / already-consumed token — leave the current UI alone.
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  // Show nothing while checking (splash is still visible).
  if (needsSetup === null) return null;

  // Show the tile-picker onboarding if no auth configured. It handles
  // both fresh users (pick a problem → sign in) and returning users
  // (tap "Already have an account? Sign in").
  if (needsSetup) {
    return <TilePickerScreen onComplete={() => setNeedsSetup(false)} />;
  }

  return <MainApp />;
}

function MainApp() {
  const zoom = useZoom(); // CSS-based zoom via Cmd+/-/0
  const session = useSession();
  const activeView = useSessionStore((s) => s.activeView);
  const addEvent = useDebugStore((s) => s.addEvent);
  const toggle = useDebugStore((s) => s.toggle);
  const refreshEntitlement = useConsumerStore((s) => s.refresh);
  const subscribeModal = useConsumerStore((s) => s.subscribeModal);
  const closeSubscribeModal = useConsumerStore((s) => s.closeSubscribeModal);

  // Hydrate the consumer entitlement once MainApp mounts.
  useEffect(() => {
    refreshEntitlement();
  }, [refreshEntitlement]);

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
      {/* Title bar — spans full width, sits in macOS overlay region */}
      <MainTitleBar />
      <UpdateBanner />
      <ProactiveSuggestionBanner />

      {/* Body: sidebar + main content */}
      <div className="flex flex-1 min-h-0 relative">
        <Sidebar session={session} />

        {/* Only the main content area zooms — title bar & sidebar stay fixed */}
        <div className="flex flex-col flex-1 min-w-0 origin-top-left" style={{ zoom }}>
          <SessionSummary />
          {activeView === "health" || activeView === "diagnostics" ? (
            <HealthDashboard />
          ) : activeView === "knowledge" ? (
            <KnowledgeView onNewKnowledge={async () => {
              useSessionStore.getState().setActiveView("chat");
              await session.startNewProblem("learn");
            }} />
          ) : activeView === "settings" ? (
            <SettingsPanel />
          ) : (
            <ChatPanel />
          )}
          <DebugPanel />
          <ActionApproval />
        </div>
      </div>
      {subscribeModal && (
        <SubscribeModal
          variant={subscribeModal.variant}
          onDismiss={closeSubscribeModal}
          onCheckoutOpened={() => {
            // Leave the modal open so the user sees a "refreshing..." cue —
            // closing happens when they return to the app and entitlement refreshes.
            refreshEntitlement();
          }}
        />
      )}
    </div>
  );
}

export default App;
