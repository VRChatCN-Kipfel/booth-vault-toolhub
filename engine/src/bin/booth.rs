//! booth CLI：统一入口（download / organize / search / audit + shell 图标命令）。
//!
//! 支持 `--json` 结构化输出（MCP 依赖）与语义化退出码：
//!   0 = 全部成功
//!   1 = 有失败项（部分失败）
//!   2 = 参数错误 / 致命错误

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[path = "booth/commands.rs"]
mod commands;

#[derive(Parser)]
#[command(
    name = "booth",
    about = "BOOTH 素材统一管理：download / organize / search / audit / version-audit / library"
)]
struct Cli {
    /// 结构化输出（JSON）。
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 整店/散链下载免费商品（需 Cookie 拉文件）。
    Download {
        /// 店铺 URL/子域名，或含 /items/<id> 的散链。
        shop: Option<String>,
        /// 散链链接/裸 ID。
        #[arg(long)]
        items: Vec<String>,
        /// 输出根目录（默认读配置 download_root）。
        #[arg(long)]
        out: Option<PathBuf>,
        /// 只检查不实际下载。
        #[arg(long)]
        dry_run: bool,
        /// 最多处理 N 个商品（0 = 不限）。
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// BOOTH 登录 Cookie：原始串 / cookies.txt / 存串文件。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 本地压缩包（文件名含 7 位 ID）按 ID 整理归档。
    Organize {
        /// BOOTH archive file(s)。
        #[arg(required = true)]
        archive: Vec<PathBuf>,
        /// 输出根目录。
        #[arg(long)]
        out: Option<PathBuf>,
        /// 强制指定商品 ID（文件名无 ID 时用）。
        #[arg(long)]
        id: Option<String>,
        /// 只检查不实际移动/下载。
        #[arg(long)]
        dry_run: bool,
        /// BOOTH 登录 Cookie（补全商品页其他免费版本）。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 本地文件（无 ID）按名搜索 BOOTH 后整理。
    Search {
        /// 待整理的文件路径。
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// 归档根目录。
        #[arg(long)]
        base_dir: Option<PathBuf>,
        /// 只搜索不实际整理。
        #[arg(long)]
        dry_run: bool,
        /// 强制指定 BOOTH 商品 ID（跳过搜索）。
        #[arg(long)]
        id: Option<String>,
        /// BOOTH 登录 Cookie。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 全库文件夹图标三件套完整性巡检 + 自动修复。
    Audit {
        /// 巡检根目录。
        #[arg(long)]
        base: Option<PathBuf>,
        /// 只扫描不修复。
        #[arg(long)]
        dry_run: bool,
        /// 不自动修复。
        #[arg(long)]
        no_fix: bool,
    },
    /// 文件夹图标三件套命令（set/reset/audit 单目录）。
    Shell {
        #[command(subcommand)]
        command: ShellCmd,
    },
    /// 检查工具自身是否有新版本（GitHub Releases）。
    UpdateCheck {
        /// 使用配置/环境代理（默认直连，规避代理失败）。
        #[arg(long)]
        proxy: bool,
    },
    /// 联网比对免费文件名版本，报告可补全项；`--fix` 补免费文件（需 Cookie）。
    VersionAudit {
        /// 巡检根目录。
        #[arg(long)]
        base: Option<PathBuf>,
        /// 对可更新项补免费文件。
        #[arg(long)]
        fix: bool,
        /// BOOTH 登录 Cookie（`--fix` 时需要）。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 列出归档库存（ID / 标题 / 类目 / 路径）。
    Library {
        /// 归档根目录。
        #[arg(long)]
        base: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ShellCmd {
    /// 设置三件套：booth shell set <cover> <folder>
    Set { cover: PathBuf, folder: PathBuf },
    /// 清理三件套：booth shell reset <folder>
    Reset { folder: PathBuf },
    /// 单目录自检：booth shell audit <folder>
    Audit { folder: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = commands::run(cli);
    ExitCode::from(code)
}
