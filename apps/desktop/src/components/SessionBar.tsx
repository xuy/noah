import { useSessionStore } from "../stores/sessionStore";
import { NoahIcon } from "./NoahIcon";

interface SessionBarProps {
  session: {
    isActive: boolean;
    elapsed: string;
    endSession: () => Promise<void>;
    createSession: () => Promise<void>;
  };
}

export function SessionBar({ session }: SessionBarProps) {
  const { isActive, elapsed, endSession, createSession } = session;
  const toggleChangeLog = useSessionStore((s) => s.toggleChangeLog);
  const changeLogOpen = useSessionStore((s) => s.changeLogOpen);
  const changesCount = useSessionStore((s) => s.changes.length);
  const toggleHistory = useSessionStore((s) => s.toggleHistory);
  const historyOpen = useSessionStore((s) => s.historyOpen);
  const toggleSettings = useSessionStore((s) => s.toggleSettings);
  const settingsOpen = useSessionStore((s) => s.settingsOpen);

  return (
    <header className="flex items-center justify-between px-4 py-2 bg-bg-secondary border-b border-border-primary select-none"
      data-tauri-drag-region=""
    >
      {/* Left: Logo and title */}
      <div className="flex items-center gap-3" data-tauri-drag-region="">
        <div className="flex items-center gap-2">
          <NoahIcon className="w-7 h-7 rounded-lg" alt="Noah" />
          <span className="text-sm font-semibold tracking-wide text-text-primary">
            Noah
          </span>
        </div>

        {/* Status indicator */}
        <div className="flex items-center gap-1.5 ml-2">
          <div
            className={`w-2 h-2 rounded-full ${
              isActive ? "bg-status-active" : "bg-status-idle"
            }`}
          />
          <span className="text-xs text-text-secondary">
            {isActive ? "Active" : "Idle"}
          </span>
        </div>

        {/* History button */}
        <button
          onClick={toggleHistory}
          title="Session history"
          className={`
            flex items-center gap-1.5 px-2 py-1 rounded-md text-xs ml-2
            transition-colors duration-150 cursor-pointer
            ${
              historyOpen
                ? "bg-accent-green/20 text-accent-green"
                : "text-text-secondary hover:text-text-primary hover:bg-bg-tertiary"
            }
          `}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M7 3.5V7L9.5 9.5"
              stroke="currentColor"
              strokeWidth="1.2"
              strokeLinecap="round"
            />
            <circle
              cx="7"
              cy="7"
              r="5.5"
              stroke="currentColor"
              strokeWidth="1.2"
            />
          </svg>
          History
        </button>
      </div>

      {/* Center: Timer */}
      <div className="flex items-center" data-tauri-drag-region="">
        <span className="text-xs font-mono text-text-muted tabular-nums">
          {elapsed}
        </span>
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-2">
        <button
          onClick={toggleChangeLog}
          className={`
            flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs
            transition-colors duration-150 cursor-pointer
            ${
              changeLogOpen
                ? "bg-accent-green/20 text-accent-green"
                : "text-text-secondary hover:text-text-primary hover:bg-bg-tertiary"
            }
          `}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M3 4H11M3 7H9M3 10H7"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
          Changes
          {changesCount > 0 && (
            <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-accent-green text-[10px] text-white font-medium">
              {changesCount}
            </span>
          )}
        </button>

        <button
          onClick={toggleSettings}
          title="Settings"
          className={`
            flex items-center justify-center w-7 h-7 rounded-md
            transition-colors duration-150 cursor-pointer
            ${
              settingsOpen
                ? "bg-accent-green/20 text-accent-green"
                : "text-text-secondary hover:text-text-primary hover:bg-bg-tertiary"
            }
          `}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M5.7 1.5H8.3L8.8 3.1L10.3 3.9L11.9 3.4L13.2 5.6L11.9 6.8V7.2L13.2 8.4L11.9 10.6L10.3 10.1L8.8 10.9L8.3 12.5H5.7L5.2 10.9L3.7 10.1L2.1 10.6L0.8 8.4L2.1 7.2V6.8L0.8 5.6L2.1 3.4L3.7 3.9L5.2 3.1L5.7 1.5Z"
              stroke="currentColor"
              strokeWidth="1.1"
              strokeLinejoin="round"
            />
            <circle cx="7" cy="7" r="1.8" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </button>

        {isActive ? (
          <button
            onClick={endSession}
            className="px-2.5 py-1 rounded-md text-xs text-accent-red hover:bg-accent-red/10 transition-colors duration-150 cursor-pointer"
          >
            End Session
          </button>
        ) : (
          <button
            onClick={createSession}
            className="px-2.5 py-1 rounded-md text-xs text-accent-green hover:bg-accent-green/10 transition-colors duration-150 cursor-pointer"
          >
            New Session
          </button>
        )}
      </div>
    </header>
  );
}
