pub mod apps;
pub mod crash_logs;
pub mod diagnostics;
pub mod disk_audit;
pub mod network;
pub mod performance;
pub mod printer;
pub mod wifi;

use crate::agent::tool_router::ToolRouter;

/// Register all macOS tools with the tool router.
pub fn register_tools(router: &mut ToolRouter) {
    // Network tools
    router.register(Box::new(network::MacNetworkInfo));
    router.register(Box::new(network::MacPing));
    router.register(Box::new(network::MacDnsCheck));
    router.register(Box::new(network::MacHttpCheck));
    router.register(Box::new(network::MacFlushDns));

    // Printer tools
    router.register(Box::new(printer::MacPrinterList));
    router.register(Box::new(printer::MacPrintQueue));
    router.register(Box::new(printer::MacCancelPrintJobs));
    router.register(Box::new(printer::MacRestartCups));

    // Performance tools
    router.register(Box::new(performance::MacSystemInfo));
    router.register(Box::new(performance::MacProcessList));
    router.register(Box::new(performance::MacDiskUsage));
    router.register(Box::new(performance::MacKillProcess));
    router.register(Box::new(performance::MacClearCaches));

    // App tools
    router.register(Box::new(apps::MacAppList));
    router.register(Box::new(apps::MacAppLogs));
    router.register(Box::new(apps::MacAppSupportLs));
    router.register(Box::new(apps::MacClearAppCache));
    router.register(Box::new(apps::MacMoveFile));

    // Diagnostic tools
    router.register(Box::new(diagnostics::MacSystemSummary));
    router.register(Box::new(diagnostics::MacReadFile));
    router.register(Box::new(diagnostics::MacReadLog));
    router.register(Box::new(diagnostics::ShellRun));

    // Compound diagnostic tools
    router.register(Box::new(wifi::WifiScan));
    router.register(Box::new(disk_audit::DiskAudit));
    router.register(Box::new(crash_logs::CrashLogReader));
}
