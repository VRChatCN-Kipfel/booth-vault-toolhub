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
    /// description 加权：说明文中每个命中词的加分（缺省读配置，配置缺省 = 关闭）。
    #[arg(long)]
    desc_word_bonus: Option<i32>,
    /// description 加权：最多计几个命中词（缺省读配置，未给时按 3）。
    #[arg(long)]
    desc_max_words: Option<usize>,
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
    /// HTTP 内容缓存：查看条目数或清空。
    ///
    /// 缓存键含登录态作用域，不同账号/匿名各存一份；`clear` 是清掉污染条目的唯一入口。
    Cache {
        #[command(subcommand)]
        command: CacheCmd,
    },
    /// 查看商品资料的可核验视图（来源指纹 + 连续原文摘录）。
    ///
    /// `description` 是**不可信外部数据**。本命令把正文与 `source_hash`（内容漂移
    /// 检测）和 `excerpt`（是否原文连续片段）一起给出，供上层判断该不该采信，
    /// 而不是把一段无从核验的文本直接丢给 agent。
    Inspect {
        /// 7 位 BOOTH 商品 ID。
        id: String,
        /// 摘录围绕的检索词，可多次；不给则取正文首部（此时 excerpt_hit=false）。
        #[arg(long)]
        term: Vec<String>,
        /// 摘录最多多少字符。
        #[arg(long, default_value_t = engine::evidence::DEFAULT_EXCERPT_CHARS)]
        excerpt_chars: usize,
        /// BOOTH 登录 Cookie。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 混乱目录一键入库：扫描 → 逐个判定身份 → 分类归档 → 汇总无法判定项。
    ///
    /// 与 `organize` / `search` 的区别：后两者假设「输入已是单个文件、且已知道
    /// 该走哪条路」；本命令吃一坨来源混杂的目录，替用户完成「逐个判断」这件事。
    /// 判定阶段不联网，联网只发生在执行阶段；`--dry-run` 只出报告、不改盘、不联网。
    Intake {
        /// 待入库的目录或文件（可多个；目录递归展开）。
        #[arg(required = true)]
        dirs: Vec<PathBuf>,
        /// 归档根目录（默认读配置 download_root）。
        #[arg(long)]
        out: Option<PathBuf>,
        /// 只出报告，不移动、不归档、不联网。
        #[arg(long)]
        dry_run: bool,
        /// 无法判定项的处置：report（默认，原地不动）| move（移入 <out>/_待处理）| skip。
        #[arg(long, default_value = "report")]
        on_unknown: String,
        /// 检索命中歧义时的处置：ask（默认，不自动归档）| skip | force（显式逃生阀）。
        #[arg(long, default_value = "ask")]
        on_ambiguous: String,
        /// 显式确认：不加时歧义项一律不自动归档，加了也只放行无歧义项。
        #[arg(long)]
        yes: bool,
        /// BOOTH 登录 Cookie（补全商品页其他免费版本）。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 错位检测：报告「已入库但分类与官方不符」的商品；`--fix` 强制重归档。
    ///
    /// 与 `intake` 严格分工：intake 判「尚未入库」的散落文件（文件域），本命令判
    /// 「已入库但放错类目」的 ID 目录（目录域）。两者输入域不重叠，可串联使用。
    Mismatch {
        /// 库根目录（默认读配置 download_root）。
        #[arg(long)]
        base: Option<PathBuf>,
        /// 强制重归档到官方分类（既有内容就地留档为「旧版本_<时间戳>」）。
        #[arg(long)]
        fix: bool,
        /// 只处理这些 ID（`--fix` 时生效；缺省全量）。
        #[arg(long)]
        id: Vec<String>,
        /// BOOTH 登录 Cookie。
        #[arg(long)]
        cookie: Option<String>,
    },
    /// 关注/已购清单：`watch` 等降价补货，`own` 等作者更新。
    Watch(CollectionsArgs),
    /// 已购素材清单（与 watch 共用存储，语义偏重「作者发布新版本」）。
    Own(CollectionsArgs),
}

#[derive(Subcommand)]
enum CacheCmd {
    /// 显示缓存条目数。
    Stats,
    /// 清空全部缓存条目。
    Clear,
}

/// 关注/已购清单的全部参数（两个子命令共用）。
#[derive(clap::Args)]
struct CollectionsArgs {
    /// 操作：add / remove / list / check。
    pub op: String,
    /// 商品 ID 或链接（add/remove 必填，可多个）。
    #[arg(value_delimiter = ',')]
    pub ids: Vec<String>,
    /// 单次 check 最多检查多少项（默认 30，防一次跑太久）。
    #[arg(long, default_value_t = 30)]
    pub limit: usize,
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
