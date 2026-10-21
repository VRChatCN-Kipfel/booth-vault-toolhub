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

    /// 下载失败时保留 .part 供取证（默认关闭：清理并上报；无续传能力）。
    #[arg(long, global = true)]
    keep_failed: bool,

    #[command(subcommand)]
    command: Command,
}

/// `booth search` 的全部参数。
///
/// 拆成独立结构体只为让 `Command` 各变体体积接近（clippy `large_enum_variant`）。
/// 语义不变：`files`（按名归档）与 `query`（纯检索）互斥。
#[derive(clap::Args)]
struct SearchArgs {
    /// 待整理的文件路径。
    #[arg(required_unless_present = "query", conflicts_with = "query")]
    files: Vec<PathBuf>,
    /// 通用检索词（只搜索、不整理）。与 `files` 互斥。
    #[arg(long, short = 'q')]
    query: Option<String>,
    /// 排序：new / popularity / liked / price_asc / price_desc（仅 `--query`）。
    #[arg(long)]
    sort: Option<String>,
    /// 商品类型：digital / physical（仅 `--query`）。
    #[arg(long = "type")]
    kind: Option<String>,
    /// R-18 处理：include / exclude / only（仅 `--query`，默认 include）。
    #[arg(long)]
    adult: Option<String>,
    /// 标签过滤，可多次（仅 `--query`）。
    #[arg(long)]
    tag: Vec<String>,
    /// 分类 slug，需日语原文如 `3D衣装`（仅 `--query`）。
    #[arg(long)]
    category: Option<String>,
    /// 最低价（日元，仅 `--query`）。
    #[arg(long)]
    min_price: Option<i64>,
    /// 最高价（日元，仅 `--query`）。
    #[arg(long)]
    max_price: Option<i64>,
    /// 仅在售（仅 `--query`）。
    #[arg(long)]
    in_stock: bool,
    /// 排除词，可多次；本地后置过滤（仅 `--query`）。
    #[arg(long)]
    exclude: Vec<String>,
    /// 页码，从 1 起（仅 `--query`）。
    #[arg(long, default_value_t = 1)]
    page: u32,
    /// 最多返回多少条（仅 `--query`）。
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// 跳过内容缓存，强制联网（仅 `--query`）。
    #[arg(long)]
    no_cache: bool,
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
    /// 本地文件（无 ID）按名搜索 BOOTH 后整理；或 `--query` 通用检索。
    ///
    /// 两种模式互斥：给 `files` 走「按名归档」老链路；给 `--query` 走纯检索、
    /// 只输出结果不做整理。混用直接报错，避免语义含糊。
    ///
    /// 检索轴整体装箱：本枚举其余变体都很小，若把十几个过滤字段直接摊在
    /// `SearchComplete` 上，会让整个 `Command` 的栈占用由最大变体决定
    /// （clippy `large_enum_variant`）。箱子换来的是每个 `Command` 值都变轻。
    Search(Box<SearchArgs>),
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
    /// 列出归档库存（ID / 标题 / 类目 / 路径）。走持久化索引，稳态秒开。
    Library {
        /// 归档根目录。
        #[arg(long)]
        base: Option<PathBuf>,
        /// 忽略索引强制重扫（索引漏掉的变化用这条兜底）。
        #[arg(long)]
        refresh: bool,
    },
    /// 预览压缩包内条目（zip / unitypackage），不联网。
    Preview {
        /// 压缩包路径。
        #[arg(required = true)]
        archive: Vec<PathBuf>,
        /// 每个包最多列出多少条（0 = 不限）。
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
    /// 检测 BOOTH Cookie 是否可用。只发一个探针请求，不触发任何下载。
    CookieCheck {
        /// 直接给 Cookie 串（缺省读配置）。
        /// 以下形态均自动识别，无需手动整理：`k=v; k=v` 整串、
        /// DevTools 表格整块复制（Application → Cookies）、`Cookie:` 请求头整行
        /// 或整块 Request Headers、cURL 命令（Copy as cURL）、每行一条的多行文本。
        /// 分析类 cookie（_ga 等）自动剔除。
        #[arg(long)]
        cookie: Option<String>,
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
