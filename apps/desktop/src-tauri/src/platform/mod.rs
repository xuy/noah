#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(any(test, target_os = "macos"))]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::Path;

use crate::agent::tool_router::ToolRouter;

/// Register platform-specific tools with the router.
/// `db_path` is passed so tools like disk_audit can read cached scan results.
pub fn register_platform_tools(router: &mut ToolRouter, db_path: Option<&Path>) {
    #[cfg(target_os = "macos")]
    macos::register_tools(router, db_path);

    #[cfg(target_os = "windows")]
    windows::register_tools(router);

    #[cfg(target_os = "linux")]
    linux::register_tools(router);
}
