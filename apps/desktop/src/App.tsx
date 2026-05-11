import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { onOpenUrl, getCurrent as getCurrentDeepLink } from "@tauri-apps/plugin-deep-link";
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
import { TrialBanner } from "./components/TrialBanner";
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

function extractDeepLinkToken(url: string, param: string): string | null {
  try {
    return new URL(url).searchParams.get(param);
  } catch {
    const m = url.match(new RegExp(`[?&]${param}=([^&]+)`));
    return m && m[1] ? decodeURIComponent(m[1]) : null;
  }
}

function App() {
  const [needsSetup, setNeedsSetup] = useState<boolean | null>(null);
  useTheme(); // Apply saved theme on mount (before setup screen too)

  // First-launch gate: show the TilePicker onboarding only when the
  // user is *both* unsigned-in AND has no chat history. Either signal
  // alone is enough to declare "this is a returning user, skip tiles":
  //   • Session token present → user has signed in on this device,
  //     even if the local journal is empty (fresh install / dev build
  //     alongside the shipping app / reset journal).
  //   • Journal has any prior session → user has chatted before, even
  //     anonymously on the device-id trial.
  // Without this, a user who reinstalls or runs the dev build with an
  // empty journal lands back on the 8-tile picker after sign-in,
  // which feels broken — they already onboarded.
  useEffect(() => {
    commands.consumerEnsureDeviceId().catch(() => {});
    // Use a sentinel so an errored probe fails *open* (skip tiles).
    // Stranding the user on the gate is worse than skipping it once.
    Promise.all([
      commands.consumerHasSession().catch(() => null),
      commands.listSessions().catch(() => null),
    ])
      .then(([hasSession, sessions]) => {
        const knownReturning =
          hasSession === true || (sessions != null && sessions.length > 0);
        const probeFailed = hasSession === null || sessions === null;
        setNeedsSetup(!knownReturning && !probeFailed);
      })
      .finally(() => {
        dismissSplash();
      });
  }, []);

  // Global deep-link handler. Two URL shapes:
  //
  //   noah://auth?token=…           — magic-link sign-in
  //   noah://subscribed?session_id=… — return from Stripe Checkout
  //
  // Two delivery paths cover both cold-start and warm-start cases:
  //   • onOpenUrl — fires when the URL arrives while Noah is running.
  //   • getCurrent — returns the URL that *launched* Noah, when the
  //     user clicked the link before Noah was running. Without this,
  //     a cold-start magic-link click leaves the user on the tile
  //     picker, which feels broken.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const handleUrls = async (urls: string[]) => {
      const authUrl = urls.find((u) => u.startsWith("noah://auth"));
      if (authUrl) {
        const token = extractDeepLinkToken(authUrl, "token");
        if (!token) return;
        try {
          await commands.consumerCompleteSignIn(token);
          // Refresh entitlement so MainApp's banners/billing reflect
          // the signed-in state on first paint, not after a poll.
          await useConsumerStore.getState().refresh();
          setNeedsSetup(false);
        } catch (err) {
          // Surface to console so users can pull a log if it happens
          // again — silent failures here are why "click magic link →
          // stuck on tiles" was so hard to diagnose.
          console.error("[noah] complete-sign-in failed", err);
        }
        return;
      }
      const subUrl = urls.find((u) => u.startsWith("noah://subscribed"));
      if (subUrl) {
        const sid = extractDeepLinkToken(subUrl, "session_id");
        if (!sid) return;
        try {
          const ent = await commands.consumerConfirmCheckout(sid);
          const consumer = useConsumerStore.getState();
          if (ent) consumer.setEntitlement(ent);
          consumer.refresh();
          consumer.closeSubscribeModal();
          setNeedsSetup(false);
        } catch (err) {
          console.error("[noah] confirm-checkout failed", err);
        }
      }
    };
    // Drain any URL that launched the app (cold-start path).
    getCurrentDeepLink()
      .then((urls) => {
        if (urls && urls.length > 0) handleUrls(urls);
      })
      .catch(() => {});
    // Subscribe for URLs delivered while running (warm-start path).
    onOpenUrl(handleUrls)
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
      <TrialBanner />
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
