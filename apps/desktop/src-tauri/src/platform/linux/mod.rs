pub mod diagnostics;
pub mod network;
pub mod performance;

use crate::agent::tool_router::ToolRouter;

pub fn register_tools(router: &mut ToolRouter) {
    // Network (5)
    router.register(Box::new(network::LinuxNetworkInfo));
    router.register(Box::new(network::LinuxPing));
    router.register(Box::new(network::LinuxDnsCheck));
    router.register(Box::new(network::LinuxHttpCheck));
    router.register(Box::new(network::LinuxFlushDns));

    // Performance (4)
    router.register(Box::new(performance::LinuxSystemInfo));
    router.register(Box::new(performance::LinuxProcessList));
    router.register(Box::new(performance::LinuxDiskUsage));
    router.register(Box::new(performance::LinuxKillProcess));

    // Diagnostics (4)
    router.register(Box::new(diagnostics::LinuxSystemSummary));
    router.register(Box::new(diagnostics::LinuxReadFile));
    router.register(Box::new(diagnostics::LinuxReadLog));
    router.register(Box::new(diagnostics::ShellRun));
}
