//! Real-hardware monitors. Each submodule owns a single concern:
//!
//!   * [`cpu_ram`] - CPU% and memory usage via the `sysinfo` crate.
//!   * [`gpu`]     - NVIDIA GPU utilisation and VRAM via NVML.
//!
//! Both are written so they can be polled cheaply once per second.

pub mod cpu_ram;
pub mod gpu;
