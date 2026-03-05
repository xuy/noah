#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

use crate::agent::tool_router::ToolRouter;

/// Register platform-specific tools with the router.
pub fn register_platform_tools(router: &mut ToolRouter) {
    #[cfg(target_os = "macos")]
    macos::register_tools(router);

    #[cfg(target_os = "windows")]
    windows::register_tools(router);

    #[cfg(target_os = "linux")]
    linux::register_tools(router);
}
