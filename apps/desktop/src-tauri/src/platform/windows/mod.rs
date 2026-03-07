pub mod apps;
pub mod diagnostics;
pub mod network;
pub mod performance;
pub mod printer;

use crate::agent::tool_router::ToolRouter;
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Create a `std::process::Command` that won't flash a console window.
pub fn hidden_cmd(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Create a `tokio::process::Command` that won't flash a console window.
pub fn hidden_async_cmd(program: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Register all Windows tools with the tool router.
pub fn register_tools(router: &mut ToolRouter) {
    // Network tools
    router.register(Box::new(network::WinNetworkInfo));
    router.register(Box::new(network::WinPing));
    router.register(Box::new(network::WinDnsCheck));
    router.register(Box::new(network::WinHttpCheck));
    router.register(Box::new(network::WinFlushDns));

    // Printer tools
    router.register(Box::new(printer::WinPrinterList));
    router.register(Box::new(printer::WinPrintQueue));
    router.register(Box::new(printer::WinCancelPrintJobs));
    router.register(Box::new(printer::WinRestartSpooler));

    // Performance tools
    router.register(Box::new(performance::WinSystemInfo));
    router.register(Box::new(performance::WinProcessList));
    router.register(Box::new(performance::WinDiskUsage));
    router.register(Box::new(performance::WinKillProcess));
    router.register(Box::new(performance::WinClearCaches));

    // App tools
    router.register(Box::new(apps::WinAppList));
    router.register(Box::new(apps::WinAppLogs));
    router.register(Box::new(apps::WinAppDataLs));
    router.register(Box::new(apps::WinClearAppCache));
    router.register(Box::new(apps::WinMoveFile));

    // Diagnostic tools
    router.register(Box::new(diagnostics::WinSystemSummary));
    router.register(Box::new(diagnostics::WinReadFile));
    router.register(Box::new(diagnostics::WinReadLog));
    router.register(Box::new(diagnostics::ShellRun));

    // Windows-specific tools
    router.register(Box::new(diagnostics::WinStartupPrograms));
    router.register(Box::new(diagnostics::WinEmptyRecycleBin));
    router.register(Box::new(diagnostics::WinDisableStartupProgram));
    router.register(Box::new(diagnostics::WinFindFile));
    router.register(Box::new(diagnostics::WinServiceList));
    router.register(Box::new(diagnostics::WinRestartService));
}
