#![cfg(target_os = "macos")]

//! macOS Finder 文件夹图标。
//!
//! 宽幅封面先贴正方形透明画布再交给 NSWorkspace，避免 Finder 缩略图变成居中小图。

pub mod folder_icon;
