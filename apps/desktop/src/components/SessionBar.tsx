import { useSessionStore } from "../stores/sessionStore";
import { useDebugStore } from "../stores/debugStore";

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
  const toggleDebug = useDebugStore((s) => s.toggle);
  const debugOpen = useDebugStore((s) => s.isOpen);
  const debugCount = useDebugStore((s) => s.events.length);
  const toggleHistory = useSessionStore((s) => s.toggleHistory);
  const historyOpen = useSessionStore((s) => s.historyOpen);

  return (
    <header className="flex items-center justify-between px-4 py-2 bg-bg-secondary border-b border-border-primary select-none"
      data-tauri-drag-region=""
    >
      {/* Left: Logo and title */}
      <div className="flex items-center gap-3" data-tauri-drag-region="">
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 rounded-lg bg-accent-blue flex items-center justify-center">
            <svg
              width="16"
              height="16"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M8 1L2 4V8C2 11.31 4.55 14.36 8 15C11.45 14.36 14 11.31 14 8V4L8 1Z"
                fill="white"
                fillOpacity="0.9"
              />
              <path
                d="M7 5H9V9H7V5ZM7 10H9V12H7V10Z"
                fill="#3b82f6"
              />
            </svg>
          </div>
          <span className="text-sm font-semibold tracking-wide text-text-primary">
            ITMan
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
                ? "bg-accent-blue/20 text-accent-blue"
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
          onClick={toggleDebug}
          title="Toggle debug panel (⌘D)"
          className={`
            flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs
            transition-colors duration-150 cursor-pointer
            ${
              debugOpen
                ? "bg-accent-purple/20 text-accent-purple"
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
              d="M7 1.5C5.067 1.5 3.5 3.067 3.5 5V5.5L1.5 7V8.5H3.5V9C3.5 10.933 5.067 12.5 7 12.5C8.933 12.5 10.5 10.933 10.5 9V8.5H12.5V7L10.5 5.5V5C10.5 3.067 8.933 1.5 7 1.5Z"
              stroke="currentColor"
              strokeWidth="1.2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <circle cx="5.5" cy="6.5" r="0.75" fill="currentColor" />
            <circle cx="8.5" cy="6.5" r="0.75" fill="currentColor" />
          </svg>
          Debug
          {debugCount > 0 && (
            <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-accent-purple text-[10px] text-white font-medium">
              {debugCount > 99 ? "99" : debugCount}
            </span>
          )}
        </button>

        <button
          onClick={toggleChangeLog}
          className={`
            flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs
            transition-colors duration-150 cursor-pointer
            ${
              changeLogOpen
                ? "bg-accent-blue/20 text-accent-blue"
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
            <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-accent-blue text-[10px] text-white font-medium">
              {changesCount}
            </span>
          )}
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
