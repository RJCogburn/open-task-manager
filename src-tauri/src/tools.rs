//! The Rust side of each tool, one file per tool, named like the tool's
//! folder in `src/tools/`. Shared code lives outside this folder: the live
//! sampler, GPU readings, logging, the tray, start-up, and running external
//! programs (`programs.rs`).
//!
//! A new tool adds its file here and its commands to the list in `lib.rs`.

pub mod benchmarks;
pub mod connections;
pub mod disk_space;
pub mod disks;
pub mod installed_apps;
pub mod network;
pub mod processes;
pub mod sensors;
