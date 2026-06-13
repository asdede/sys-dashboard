//! CPU and RAM sampling via the `sysinfo` crate.
//!
//! Why a struct instead of a free function?
//! -----------------------------------------
//! `sysinfo::System` owns a kernel snapshot. CPU percentage is computed
//! by *diffing* the current snapshot against the previous one - so we
//! must keep the same instance alive across samples to get a meaningful
//! number. The first call to `refresh_cpu_usage()` returns 0.0 for every
//! core because there is no previous snapshot to diff against; we
//! "prime" the pump in [`SystemMonitor::new`] so the first real sample
//! the frontend asks for is already correct.

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Owns the long-lived `sysinfo::System` and exposes a single `sample()`
/// entry point.
pub struct SystemMonitor {
    inner: System,
}

impl SystemMonitor {
    pub fn new() -> Self {
        // `RefreshKind` is a bit-set saying which kernel facts we want
        // sysinfo to load. We only need CPU + memory; skipping the rest
        // (processes, disks, networks, components) is a meaningful win
        // because process scanning is by far the most expensive op.
        //
        // Note: sysinfo 0.32 spells the empty constructor `new()`. It
        // was renamed to `nothing()` in 0.33+. If you bump the crate
        // version, also rename this call.
        let kinds = RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything());

        let mut inner = System::new_with_specifics(kinds);

        // Prime the CPU sampler. After we've slept long enough, the next
        // call to `refresh_cpu_usage()` will produce a correct delta.
        // `MINIMUM_CPU_UPDATE_INTERVAL` is the smallest gap sysinfo
        // promises is meaningful (~200 ms on Linux).
        inner.refresh_cpu_usage();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        inner.refresh_cpu_usage();

        Self { inner }
    }

    /// Take a fresh sample. Returns
    /// `(cpu_percent_0_to_100, ram_used_bytes, ram_total_bytes)`.
    pub fn sample(&mut self) -> (f32, u64, u64) {
        // Both refreshes are cheap counter reads (no syscalls per
        // process). At 1 Hz we will not notice them.
        self.inner.refresh_cpu_usage();
        self.inner.refresh_memory();

        // `global_cpu_usage()` is the average of all logical cores.
        // If you ever want per-core bars, iterate `self.inner.cpus()`.
        let cpu = self.inner.global_cpu_usage();
        let ram_used = self.inner.used_memory();
        let ram_total = self.inner.total_memory();

        (cpu, ram_used, ram_total)
    }
}
