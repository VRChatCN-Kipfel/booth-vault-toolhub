#![cfg(windows)]

//! Windows Shell 专属模块。
//!
//! 仅支持 Windows；macOS/Linux 上该 crate 退化（见 Cargo.toml feature `windows-shell`）。
//!
//! 模块：
//! - `attributes`：文件属性位（H+S 同设、清 0x80、READONLY）
//! - `icon`：封面 → 多尺寸 ICO（正方形画布居中）
//! - `refresh`：Explorer 图标刷新（SHGetSetFolderCustomSettings + SHChangeNotify）
//! - `folder_icon`：三件套编排 + 完整性契约 + 失败回滚

pub mod attributes;
pub mod folder_icon;
pub mod icon;
pub mod refresh;
