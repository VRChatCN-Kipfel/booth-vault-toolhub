#![cfg(windows)]

//! Windows Shell 专属模块。
//!
//! 仅支持 Windows；macOS/Linux 上该 crate 退化（见 Cargo.toml feature `windows-shell`）。

pub mod attributes;
pub mod folder_icon;
