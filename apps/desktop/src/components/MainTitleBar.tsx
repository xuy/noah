import { useSessionStore } from "../stores/sessionStore";

const isMac = navigator.platform.startsWith("Mac");

export function MainTitleBar() {
  const sidebarOpen = useSessionStore((s) => s.sidebarOpen);
  const toggleSidebar = useSessionStore((s) => s.toggleSidebar);
  const toggleSettings = useSessionStore((s) => s.toggleSettings);
  const settingsOpen = useSessionStore((s) => s.settingsOpen);
  // On macOS, pad left to clear native traffic lights (76px).
  // Title bar is outside the zoom container, so no compensation needed.
  const paddingLeft = isMac ? 76 : 12;

  return (
    <div
      className="flex items-center justify-between h-[28px] pr-3 flex-shrink-0 select-none"
      style={{ paddingLeft }}
      data-tauri-drag-region=""
    >
      {/* Left: Sidebar toggle (next to traffic lights on Mac) */}
      <button
        onClick={toggleSidebar}
        title={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
        className="flex items-center justify-center w-7 h-7 rounded-md text-text-muted hover:text-text-primary hover:bg-bg-tertiary/50 transition-colors cursor-pointer"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
          <path d="M5.5 2.5V13.5" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>

      {/* Right: Settings */}
      <button
        onClick={toggleSettings}
        className={`
          flex items-center justify-center w-7 h-7 rounded-lg
          transition-colors duration-150 cursor-pointer
          ${
            settingsOpen
              ? "bg-accent-blue/15 text-accent-blue"
              : "text-text-muted hover:text-text-secondary hover:bg-bg-tertiary/50"
          }
        `}
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M5.7 1.5H8.3L8.8 3.1L10.3 3.9L11.9 3.4L13.2 5.6L11.9 6.8V7.2L13.2 8.4L11.9 10.6L10.3 10.1L8.8 10.9L8.3 12.5H5.7L5.2 10.9L3.7 10.1L2.1 10.6L0.8 8.4L2.1 7.2V6.8L0.8 5.6L2.1 3.4L3.7 3.9L5.2 3.1L5.7 1.5Z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" />
          <circle cx="7" cy="7" r="1.8" stroke="currentColor" strokeWidth="1.1" />
        </svg>
      </button>
    </div>
  );
}
