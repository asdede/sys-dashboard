//! NVIDIA GPU utilisation + VRAM via NVML.
//!
//! NVML lives in `libnvidia-ml.so.1`, which ships with the proprietary
//! NVIDIA driver (`akmod-nvidia` from rpmfusion on Fedora 43). The
//! `nvml-wrapper` crate does **not** link against it at compile time -
//! it `dlopen`s it at runtime - so the binary still builds and starts
//! on machines with no NVIDIA hardware. We just get an `Err` from
//! `Nvml::init()` and turn that into "GPU N/A" on the frontend.
//!
//! The graceful-degradation pattern is worth internalising: any time you
//! reach out to optional hardware, prefer
//!
//! ```text
//! Option<Handle>  +  Result<Sample, _>::ok()
//! ```
//!
//! over panicking. That way one missing capability never takes the
//! whole dashboard down.

use nvml_wrapper::Nvml;

/// One snapshot worth of GPU information for the primary device.
pub struct GpuStats {
    /// 0..=100. Compute/graphics engine utilisation ("% of GPU busy").
    pub utilization_percent: f32,
    pub vram_used_bytes: u64,
    pub vram_total_bytes: u64,
}

pub struct GpuMonitor {
    /// `None` means NVML failed to initialise. We never retry - if the
    /// driver disappears mid-run we'll keep returning `None` until the
    /// user restarts the app, which on a desktop widget is fine.
    nvml: Option<Nvml>,
}

impl GpuMonitor {
    pub fn new() -> Self {
        // `Nvml::init()` returns Result<Nvml, NvmlError>. We log the
        // failure to stderr (visible when running `tauri dev`) and
        // collapse to None. `.ok()` would also work but we want the
        // diagnostic line.
        let nvml = match Nvml::init() {
            Ok(n) => Some(n),
            Err(e) => {
                eprintln!(
                    "[gpu] NVML init failed: {e}. GPU monitoring disabled."
                );
                None
            }
        };

        Self { nvml }
    }

    /// Returns Some on a successful read of device 0, None otherwise.
    /// Per-call failures (e.g. transient `Unknown` errors from NVML)
    /// also collapse to None so the next tick can try again.
    pub fn sample(&mut self) -> Option<GpuStats> {
        // `?` on Option short-circuits to None when nvml is absent.
        let nvml = self.nvml.as_ref()?;

        // device_by_index(0) -> primary GPU. Multi-GPU support would
        // loop over `nvml.device_count()?` and aggregate or pick.
        let device = nvml.device_by_index(0).ok()?;

        // utilization_rates() returns { gpu: u32, memory: u32 } where
        // each is 0..=100. We only surface gpu% here; memory% from NVML
        // means "memory bus busy", which is a different and noisier
        // signal than VRAM occupancy.
        let util = device.utilization_rates().ok()?;

        // memory_info() gives the actually-useful VRAM occupancy in
        // bytes (free + used + total).
        let mem = device.memory_info().ok()?;

        Some(GpuStats {
            utilization_percent: util.gpu as f32,
            vram_used_bytes: mem.used,
            vram_total_bytes: mem.total,
        })
    }
}
