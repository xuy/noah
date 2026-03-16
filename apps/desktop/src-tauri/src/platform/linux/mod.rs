pub mod apps;
pub mod diagnostics;
pub mod network;
pub mod performance;
pub mod printer;

use crate::agent::tool_router::ToolRouter;

pub fn register_tools(router: &mut ToolRouter) {
    // Network (5)
    router.register(Box::new(network::LinuxNetworkInfo));
    router.register(Box::new(network::LinuxPing));
    router.register(Box::new(network::LinuxDnsCheck));
    router.register(Box::new(network::LinuxHttpCheck));
    router.register(Box::new(network::LinuxFlushDns));

    // Performance (5)
    router.register(Box::new(performance::LinuxSystemInfo));
    router.register(Box::new(performance::LinuxProcessList));
    router.register(Box::new(performance::LinuxDiskUsage));
    router.register(Box::new(performance::LinuxKillProcess));
    router.register(Box::new(performance::LinuxClearCaches));

    // App tools (3)
    router.register(Box::new(apps::LinuxAppList));
    router.register(Box::new(apps::LinuxAppDataLs));
    router.register(Box::new(apps::LinuxClearAppCache));

    // Printer tools (4)
    router.register(Box::new(printer::LinuxPrinterList));
    router.register(Box::new(printer::LinuxPrintQueue));
    router.register(Box::new(printer::LinuxCancelPrintJobs));
    router.register(Box::new(printer::LinuxRestartCups));

    // Diagnostics (4)
    router.register(Box::new(diagnostics::LinuxSystemSummary));
    router.register(Box::new(diagnostics::LinuxReadFile));
    router.register(Box::new(diagnostics::LinuxReadLog));
    router.register(Box::new(diagnostics::ShellRun));
}
