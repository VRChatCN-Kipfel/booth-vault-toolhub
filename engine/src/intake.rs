//! 混乱目录一键入库：扫描 → 逐个判定身份 → 分类归档 → 汇总无法判定项。
//!
//! 与 `organize` / `search` 的分工：#54 指出既有两条链路都假设「输入已是单个文件、
//! 且用户已知道该走哪条路」，而真实场景是**手上有一坨来源混杂的目录**，需要工具
//! 来完成「逐个判断」这件事本身。本模块只做判定与编排，归档动作一律复用
//! `organize_archive`，不新写归档逻辑。
//!
//! 三端（CLI / MCP / GUI）只做参数映射与渲染。判据落在这里而非各端，是因为
//! `expand_directories` 曾只存在于 GUI 的 `commands.rs`（#54 现状盘点第 2 条），
//! 导致 CLI / MCP 够不到目录输入——同一份判定写两处必然再次分叉。
//!
//! 图标注入不设参数：`organize::default_icon_fn` 已在 engine 内做平台分派
//! （Windows 写三件套、macOS 走 Finder、其余 no-op），CLI 与 GUI 传的就是它。
//! 若在此再开一个 `icon_fn` 参数，MCP 那份历史 no-op 实现就会成为三端行为
//! 分叉的入口；固化到 engine 是「同一输入三端一致」的直接保障。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;

use crate::clean::sanitize;
use crate::organize::{OrganizeOptions, default_icon_fn, is_sidecar_name, organize_archive};
use crate::score::DescWeight;

/// `Unknown` 项移入的固定子目录名（根下一层，不递归嵌套）。
///
/// 命名须同时避开三处：`classify` 的 53 个类目名（均为纯中文或 `3D` 开头）、
/// `organize::LEGACY_PREFIX`（`旧版本_`），以及 `audit::ID_DIR_RE` 的 `^\d{7}`——
/// 下划线开头既不会被认成类目、也不会被 `scan_library` 误认成商品目录。
pub const UNKNOWN_DIR: &str = "_待处理";

/// 视为「按名检索」候选的压缩包扩展名（小写，不含点）。
///
/// 比 `integrity::CHECKABLE_EXTS`（zip / unitypackage）宽：结构校验只对能解析的
/// 格式成立，但「按名检索」对 rar / 7z 同样有意义（评分走标题与资源名，
/// `names_for_score` 对不认识的格式返回 `None`，等价于跳过验真）。
const ARCHIVE_EXTS: &[&str] = &["zip", "unitypackage", "rar", "7z", "tar", "gz", "tgz"];

/// 单个文件的身份判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeKind {
    /// 文件名含 7 位商品 ID → 直接走 organize。
    WithId,
    /// 无 ID 的压缩包 → 走 search 链路（检索 + 评分）。
    SearchByName,
    /// 已位于 `ID_名称/` 目录结构内 → 跳过（幂等）。
    AlreadyArchived,
    /// 旁路文件（封面/图标/ini/系统文件）→ 跳过。
    Sidecar,
    /// 无法判定 → 按 `OnUnknown` 策略处置。
    Unknown,
}

impl IntakeKind {
    /// 稳定标识（三端 JSON 同名字段，不得各写一份）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WithId => "with-id",
            Self::SearchByName => "search-by-name",
            Self::AlreadyArchived => "already-archived",
            Self::Sidecar => "sidecar",
            Self::Unknown => "unknown",
        }
    }
}

/// 单项执行结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeStatus {
    /// 已归档进库。
    Archived,
    /// 跳过（已在库 / 旁路文件 / 已存在 / 按策略静默）。
    Skipped,
    /// 仅报告，原地未动（dry-run 与 `OnUnknown::Report` 共用）。
    Reported,
    /// 已移入 `_待处理`。
    Moved,
    /// 检索命中歧义，按 `AmbiguousPolicy::Ask` 未自动归档。
    Ambiguous,
    /// 执行失败（网络 / 权限 / 目标冲突）。
    Failed,
}

impl IntakeStatus {
    /// 稳定标识（三端 JSON 同名字段）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Archived => "archived",
            Self::Skipped => "skipped",
            Self::Reported => "reported",
            Self::Moved => "moved",
            Self::Ambiguous => "ambiguous",
            Self::Failed => "failed",
        }
    }
}

/// `Unknown` 的处置策略（三端一致，默认 `Report`：零副作用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnUnknown {
    /// 仅报告，原地不动。
    Report,
    /// 移入指定目录（CLI 固定传 `root/_待处理`）。
    MoveTo(PathBuf),
    /// 不做任何动作。**报告里仍列该项**——见 `IntakeReport::to_command_json`。
    Skip,
}

/// 解析 `--on-unknown` 取值。`move` 的目标固定为 `root/UNKNOWN_DIR`。
pub fn parse_on_unknown(value: &str, root: &Path) -> Result<OnUnknown, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "report" => Ok(OnUnknown::Report),
        "skip" => Ok(OnUnknown::Skip),
        "move" => Ok(OnUnknown::MoveTo(root.join(UNKNOWN_DIR))),
        other => Err(format!(
            "未知的 --on-unknown 取值: {other}（可选 report | move | skip）"
        )),
    }
}

/// 检索命中歧义时的策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguousPolicy {
    /// 默认：不自动归档，只报告（`--yes` 不放行歧义项）。
    Ask,
    /// 静默跳过（报告里仍列）。
    Skip,
    /// 显式逃生阀：强制按最高分归档（`--on-ambiguous force`）。
    Force,
}

/// 解析 `--on-ambiguous` 取值。
pub fn parse_ambiguous(value: &str) -> Result<AmbiguousPolicy, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "ask" => Ok(AmbiguousPolicy::Ask),
        "skip" => Ok(AmbiguousPolicy::Skip),
        "force" => Ok(AmbiguousPolicy::Force),
        other => Err(format!(
            "未知的 --on-ambiguous 取值: {other}（可选 ask | skip | force）"
        )),
    }
}

/// 扫描与执行的共同配置。
#[derive(Debug, Clone)]
pub struct IntakeOptions {
    /// 归档根目录（库根）。
    pub out_root: PathBuf,
    pub on_unknown: OnUnknown,
    pub on_ambiguous: AmbiguousPolicy,
    pub dry_run: bool,
    /// 是否已获显式确认（CLI `--yes` / MCP `yes`）。
    ///
    /// 未确认时**只有 `WithId` 项会执行**：那类项的归属由文件名里的 7 位 ID 直接
    /// 决定，误配风险接近于零。`SearchByName` 要联网猜身份，猜错就是错归档，故未
    /// 确认时只出判定结论、不联网、不动盘——这正是 #54「CLI 需 `--yes` 才继续」
    /// 的落地，也让 `yes` 不至于变成一个死参数（本仓有过先例）。
    pub confirm: bool,
    pub cookie: Option<String>,
    pub keep_failed_downloads: bool,
    /// 联网项之间的间隔（0.5~0.8s，三端统一，不得绕过）。
    pub rate_limit_secs: f64,
    /// 按名检索时的 description 加权（三端同源，取 `config::desc_weight` 的结果）。
    pub desc: DescWeight,
}

/// 单个文件的判定项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeItem {
    pub path: PathBuf,
    pub kind: IntakeKind,
    /// 命中的 7 位 ID（仅 `WithId` / `AlreadyArchived` 有值）。
    pub id: Option<String>,
    /// 判定依据（供汇总与排查）。
    pub reason: String,
}

/// 判定 + 执行后的单项条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeEntry {
    pub item: IntakeItem,
    pub status: IntakeStatus,
    pub message: String,
    pub target_dir: PathBuf,
}

/// 整次 intake 的报告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeReport {
    pub root: PathBuf,
    pub dry_run: bool,
    pub entries: Vec<IntakeEntry>,
}

impl IntakeReport {
    /// 指定状态的条目数。
    pub fn count(&self, status: IntakeStatus) -> usize {
        self.entries.iter().filter(|e| e.status == status).count()
    }

    /// 全部无法判定项（`OnUnknown::Skip` 也列出：不静默跳过是本仓红线）。
    pub fn unknowns(&self) -> Vec<&IntakeEntry> {
        self.entries
            .iter()
            .filter(|e| e.item.kind == IntakeKind::Unknown)
            .collect()
    }

    /// 已移动项的目标路径（供人工回收，不引入回滚语义）。
    pub fn moved_paths(&self) -> Vec<&Path> {
        self.entries
            .iter()
            .filter(|e| e.status == IntakeStatus::Moved)
            .map(|e| e.target_dir.as_path())
            .collect()
    }

    /// 是否有失败项（CLI 退出码据此判 1）。
    pub fn has_failures(&self) -> bool {
        self.count(IntakeStatus::Failed) > 0
    }

    /// JSON 信封（三端单点定义，不得各写一份后漂移）。
    pub fn to_command_json(&self) -> serde_json::Value {
        let entries: Vec<serde_json::Value> = self
            .entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.item.path.display().to_string(),
                    "kind": e.item.kind.as_str(),
                    "id": e.item.id.clone().unwrap_or_default(),
                    "reason": &e.item.reason,
                    "status": e.status.as_str(),
                    "message": &e.message,
                    "target_dir": e.target_dir.display().to_string(),
                })
            })
            .collect();
        let unknown: Vec<serde_json::Value> = self
            .unknowns()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.item.path.display().to_string(),
                    "reason": &e.item.reason,
                    "status": e.status.as_str(),
                    "message": &e.message,
                })
            })
            .collect();
        serde_json::json!({
            "command": "intake",
            "root": self.root.display().to_string(),
            "dry_run": self.dry_run,
            "scanned": self.entries.len(),
            "archived": self.count(IntakeStatus::Archived),
            "skipped": self.count(IntakeStatus::Skipped),
            "reported": self.count(IntakeStatus::Reported),
            "moved": self.count(IntakeStatus::Moved),
            "ambiguous": self.count(IntakeStatus::Ambiguous),
            "failed": self.count(IntakeStatus::Failed),
            "moved_paths": self
                .moved_paths()
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<String>>(),
            "unknown": unknown,
            "entries": entries,
        })
    }
}

/// 扫描并判定 `root` 下的每个文件（不联网、不改盘）。
///
/// `opts` 在扫描阶段不参与判定——身份只由文件名与目录结构决定，联网只发生在
/// 执行阶段。参数保留在签名里是为了三端同参同出参：CLI / MCP 把同一份
/// `IntakeOptions` 透传进来，不必为 scan 再造一个只差一个字段的孪生结构。
pub fn intake_scan(root: &Path, opts: &IntakeOptions) -> Vec<IntakeItem> {
    if root.is_file() {
        return vec![classify_file(root)];
    }
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    // 自己的投放目录不参与扫描：否则 `--on-unknown move` 第二次运行会把上次
    // 收进去的项重新判成 Unknown 再搬一遍（原地打转），报告也会把同一个文件
    // 算成两次。判据用规范化路径比较，避免 `_待处理` 与 `_待处理/` 之类的写法差异。
    let skip = match &opts.on_unknown {
        OnUnknown::MoveTo(dir) => dir.canonicalize().ok(),
        _ => None,
    };
    walk_files(root, &mut out, &mut visited, skip.as_deref());
    // 跨平台目录枚举顺序不一致（Windows vs Linux/macOS），报告必须有确定顺序，
    // 否则三端输出无法逐条对拍，测试也会依赖枚举顺序。
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// 递归遍历：目录深入，文件判定。`skip` 为投放目录（含其子树）。
fn walk_files(
    dir: &Path,
    out: &mut Vec<IntakeItem>,
    visited: &mut HashSet<PathBuf>,
    skip: Option<&Path>,
) {
    // `is_dir` 会跟随符号链接和 Windows junction；按规范化路径去重以避免环路
    // （与 `audit::walk_dirs` 同一做法）。不用 `DirEntry::file_type()`：
    // 它不跟随重解析点，对 junction 返回 is_dir=false，会把另一块盘的目录漏掉。
    if let Ok(canonical) = dir.canonicalize()
        && !visited.insert(canonical)
    {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if skip.is_some_and(|s| path.canonicalize().is_ok_and(|c| c == s)) {
                continue;
            }
            walk_files(&path, out, visited, skip);
        } else if path.is_file() {
            out.push(classify_file(&path));
        }
    }
}

/// 单文件身份判定。顺序即优先级（行为契约，改动会影响归档归属）。
fn classify_file(path: &Path) -> IntakeItem {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let base = IntakeItem {
        path: path.to_path_buf(),
        kind: IntakeKind::Unknown,
        id: None,
        reason: String::new(),
    };
    if is_sidecar_name(&name) {
        return IntakeItem {
            kind: IntakeKind::Sidecar,
            reason: format!("旁路文件 {name}（封面/图标/ini/系统文件），不参与归档"),
            ..base
        };
    }
    if let Some((id, dir_name)) = ancestor_id_dir(path) {
        return IntakeItem {
            kind: IntakeKind::AlreadyArchived,
            reason: format!("已位于商品目录 {dir_name}（ID {id}）内，跳过"),
            id: Some(id),
            ..base
        };
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let id = crate::id::extract_id(&stem);
    if !id.is_empty() {
        return IntakeItem {
            kind: IntakeKind::WithId,
            reason: format!("文件名含 7 位商品 ID {id}"),
            id: Some(id),
            ..base
        };
    }
    if is_archive(path) {
        return IntakeItem {
            kind: IntakeKind::SearchByName,
            reason: "无 ID 的压缩包，按名检索 BOOTH".to_string(),
            ..base
        };
    }
    IntakeItem {
        reason: "文件名无 7 位 ID 且非压缩包，无法判定".to_string(),
        ..base
    }
}

/// 文件祖先链上是否存在商品目录（`7 位 ID + 分隔符 + 名称`）。
fn ancestor_id_dir(path: &Path) -> Option<(String, String)> {
    let mut cur = path.parent();
    while let Some(p) = cur {
        if let Some(n) = p.file_name().and_then(|n| n.to_str())
            && let Some(hit) = crate::audit::id_dir_of(n)
        {
            return Some(hit);
        }
        cur = p.parent();
    }
    None
}

/// 扩展名是否压缩包（大小写不敏感）。
fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| ARCHIVE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// 扫描 → 判定 → 执行，一次调用内完成；`dry_run` 只出报告不改盘、不联网。
///
/// 部分失败**不引入回滚**：归档是逐项独立的目录操作，事务语义需要移动日志，
/// 与 AGENTS 第 12 条「无 manifest、纯文件系统推导状态」直接冲突。失败项逐条
/// 报告状态，已移动项列在 `moved_paths` 供人工回收。
pub fn intake_run(client: &Client, root: &Path, opts: &IntakeOptions) -> IntakeReport {
    let items = intake_scan(root, opts);
    let mut entries: Vec<IntakeEntry> = Vec::with_capacity(items.len());
    let mut networked_done = 0usize;
    for item in items {
        let networked = matches!(item.kind, IntakeKind::WithId | IntakeKind::SearchByName);
        // 限速只压在「真的要联网」的项之间，且 dry-run 完全不联网（#54 契约：
        // 判定阶段不联网，联网只发生在执行阶段）。
        if networked && !opts.dry_run && networked_done > 0 {
            crate::download::sleep_rate_limit(opts.rate_limit_secs);
        }
        let entry = match item.kind {
            IntakeKind::Sidecar | IntakeKind::AlreadyArchived => IntakeEntry {
                message: item.reason.clone(),
                item,
                status: IntakeStatus::Skipped,
                target_dir: PathBuf::new(),
            },
            IntakeKind::WithId => run_with_id(client, item, opts),
            IntakeKind::SearchByName => run_by_name(client, item, opts),
            IntakeKind::Unknown => handle_unknown(item, opts),
        };
        if networked && !opts.dry_run {
            networked_done += 1;
        }
        entries.push(entry);
    }
    IntakeReport {
        root: root.to_path_buf(),
        dry_run: opts.dry_run,
        entries,
    }
}

/// `WithId` 项：直接复用 `organize_archive`。
fn run_with_id(client: &Client, item: IntakeItem, opts: &IntakeOptions) -> IntakeEntry {
    let id = item.id.clone().unwrap_or_default();
    if opts.dry_run {
        return IntakeEntry {
            message: format!("[dry-run] 将按 ID {id} 归档到 {}", opts.out_root.display()),
            item,
            status: IntakeStatus::Reported,
            target_dir: PathBuf::new(),
        };
    }
    let oopts = OrganizeOptions {
        out_root: &opts.out_root,
        dry_run: false,
        cookie: opts.cookie.as_deref(),
        keep_failed_downloads: opts.keep_failed_downloads,
    };
    let outcome = organize_archive(client, &item.path, &id, &oopts, default_icon_fn);
    let status = if !outcome.ok {
        IntakeStatus::Failed
    } else if outcome.moved {
        IntakeStatus::Archived
    } else {
        IntakeStatus::Skipped
    };
    IntakeEntry {
        item,
        status,
        message: outcome.message,
        target_dir: outcome.target_dir,
    }
}

/// `SearchByName` 项：检索 → 评分 → 按歧义策略决定是否归档。
fn run_by_name(client: &Client, mut item: IntakeItem, opts: &IntakeOptions) -> IntakeEntry {
    if opts.dry_run {
        return IntakeEntry {
            message: "[dry-run] 将按名检索 BOOTH 后归档".to_string(),
            item,
            status: IntakeStatus::Reported,
            target_dir: PathBuf::new(),
        };
    }
    // 未获显式确认：只出判定结论，不联网（判定阶段不联网是 #54 契约）。
    if !opts.confirm {
        return IntakeEntry {
            message: "无 ID 的压缩包，需按名检索；未确认（--yes / yes=true），本次不联网、未归档"
                .to_string(),
            item,
            status: IntakeStatus::Reported,
            target_dir: PathBuf::new(),
        };
    }
    let outcome = pick_by_name(client, &item.path, opts);
    let forced = match (&outcome.hit, &outcome.ambiguous_hit) {
        (None, Some(h)) if opts.on_ambiguous == AmbiguousPolicy::Force => Some(h),
        _ => None,
    };
    if let Some(h) = forced {
        item.id = Some(h.item.id.clone());
        return archive_by_id(
            client,
            item,
            &h.item.id,
            opts,
            "按名检索命中歧义，已按 --on-ambiguous force 归档",
        );
    }
    if let Some(h) = outcome.hit {
        item.id = Some(h.item.id.clone());
        return archive_by_id(client, item, &h.item.id, opts, "");
    }
    if let Some(h) = outcome.ambiguous_hit {
        item.id = Some(h.item.id.clone());
        let status = if opts.on_ambiguous == AmbiguousPolicy::Skip {
            IntakeStatus::Skipped
        } else {
            IntakeStatus::Ambiguous
        };
        return IntakeEntry {
            message: format!(
                "检索命中歧义（候选 {} 与次优分差 <30），未自动归档；确认后可用 --on-ambiguous force，或改用 booth organize <文件> --id 指定",
                h.item.id
            ),
            item,
            status,
            target_dir: PathBuf::new(),
        };
    }
    item.reason = "按名检索无命中，无法判定".to_string();
    handle_unknown(item, opts)
}

/// 命中后归档（按名链路与强制歧义共用）。
fn archive_by_id(
    client: &Client,
    item: IntakeItem,
    id: &str,
    opts: &IntakeOptions,
    prefix: &str,
) -> IntakeEntry {
    let oopts = OrganizeOptions {
        out_root: &opts.out_root,
        dry_run: false,
        cookie: opts.cookie.as_deref(),
        keep_failed_downloads: opts.keep_failed_downloads,
    };
    let outcome = organize_archive(client, &item.path, id, &oopts, default_icon_fn);
    let status = if !outcome.ok {
        IntakeStatus::Failed
    } else if outcome.moved {
        IntakeStatus::Archived
    } else {
        IntakeStatus::Skipped
    };
    let message = if prefix.is_empty() {
        outcome.message
    } else {
        format!("{prefix}；{}", outcome.message)
    };
    IntakeEntry {
        item,
        status,
        message,
        target_dir: outcome.target_dir,
    }
}

/// 按名解析：三轮候选（原候选 → 同义词 → kana 变体）由 engine 单点实现。
///
/// 不复用 `score_and_pick` 自己拼一轮，是因为 `resolve_by_name` 额外守着
/// 「名称真的命中」与「不歧义」两条判据（#71 的教训：只判 `is_some()` 会把无关
/// 商品当命中并错归档）。歧义候选由它单独回传，供 `--on-ambiguous force` 逃生阀使用。
fn pick_by_name(
    client: &Client,
    path: &Path,
    opts: &IntakeOptions,
) -> crate::search::ResolveOutcome {
    let fname = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let candidates = crate::clean::sanitize_query(&fname);
    let names = crate::unitypackage::names_for_score(path);
    crate::search::resolve_by_name(
        client,
        &candidates,
        names.as_deref(),
        crate::kana::MAX_VARIANTS,
        crate::synonyms::MAX_SYNONYMS,
        |id| {
            crate::fetch::fetch_item(client, id)
                .ok()
                .map(|i| crate::search::CanonInfo {
                    name: i.name,
                    description: i.description,
                })
        },
        opts.desc,
    )
}

/// `Unknown` 项按策略处置。
///
/// `Skip` 只跳过**动作**，条目仍在报告里（`reason` 标注策略）——「不静默跳过」
/// 是本仓红线，且 #54 明确要求「无论哪种策略，报告里必须列出全部 Unknown 及其
/// reason」。CLI 的人类可读输出另按状态过滤，避免刷屏。
fn handle_unknown(item: IntakeItem, opts: &IntakeOptions) -> IntakeEntry {
    match &opts.on_unknown {
        OnUnknown::Report => IntakeEntry {
            message: format!("[报告] 原地不动：{}", item.reason),
            item,
            status: IntakeStatus::Reported,
            target_dir: PathBuf::new(),
        },
        OnUnknown::Skip => IntakeEntry {
            message: format!("[跳过] 按 --on-unknown skip：{}", item.reason),
            item,
            status: IntakeStatus::Skipped,
            target_dir: PathBuf::new(),
        },
        OnUnknown::MoveTo(dir) => {
            if opts.dry_run {
                return IntakeEntry {
                    message: format!("[dry-run] 将移入 {}", dir.display()),
                    item,
                    status: IntakeStatus::Reported,
                    target_dir: dir.clone(),
                };
            }
            match move_to(&item.path, dir) {
                Ok(dest) => IntakeEntry {
                    message: format!("已移入 {}", dest.display()),
                    item,
                    status: IntakeStatus::Moved,
                    target_dir: dest,
                },
                Err(e) => IntakeEntry {
                    item,
                    status: IntakeStatus::Failed,
                    message: e,
                    target_dir: PathBuf::new(),
                },
            }
        }
    }
}

/// 移入目标目录（根下一层，摊平，不保留源目录层级）。
fn move_to(path: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("创建 {} 失败: {e}", dest_dir.display()))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| sanitize(n, 120))
        .unwrap_or_else(|| "unnamed".to_string());
    let dest = unique_dest(dest_dir, &name);
    if std::fs::rename(path, &dest).is_ok() {
        return Ok(dest);
    }
    std::fs::copy(path, &dest).map_err(|e| format!("移动/复制失败 {}: {e}", dest.display()))?;
    // 跨盘回退：复制成功后删原件才是「移动」语义；删除失败仍算移动成功，
    // 但原件还在，报告里的目标路径就是唯一线索，故照常返回。
    if std::fs::remove_file(path).is_err() {
        return Ok(dest);
    }
    Ok(dest)
}

/// 目标名冲突时追加 `_1`、`_2`…，绝不覆盖既有文件（覆盖不可逆）。
fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for i in 1..1000u32 {
        let cand = dir.join(format!("{stem}_{i}{ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    dir.join(format!("{stem}_dup{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bvt_intake_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn opts(root: &Path) -> IntakeOptions {
        IntakeOptions {
            out_root: root.to_path_buf(),
            on_unknown: OnUnknown::Report,
            on_ambiguous: AmbiguousPolicy::Ask,
            dry_run: true,
            confirm: true,
            cookie: None,
            keep_failed_downloads: false,
            rate_limit_secs: 0.0,
            desc: DescWeight::OFF,
        }
    }

    fn touch(path: &Path) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn classifies_every_kind() {
        let root = tmpdir("kinds");
        touch(&root.join("1234567_Hair.zip"));
        touch(&root.join("LunariaPaperFan.zip"));
        touch(&root.join("readme.txt"));
        touch(&root.join("cover.jpg"));
        touch(&root.join("desktop.ini"));
        touch(&root.join("3D发型").join("7654321_Pop").join("inner.zip"));

        let items = intake_scan(&root, &opts(&root));
        let kind_of = |name: &str| {
            items
                .iter()
                .find(|i| i.path.ends_with(name))
                .unwrap_or_else(|| panic!("{name} 未判定"))
                .kind
        };
        assert_eq!(kind_of("1234567_Hair.zip"), IntakeKind::WithId);
        assert_eq!(kind_of("LunariaPaperFan.zip"), IntakeKind::SearchByName);
        assert_eq!(kind_of("readme.txt"), IntakeKind::Unknown);
        assert_eq!(kind_of("cover.jpg"), IntakeKind::Sidecar);
        assert_eq!(kind_of("desktop.ini"), IntakeKind::Sidecar);
        assert_eq!(kind_of("inner.zip"), IntakeKind::AlreadyArchived);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_order_is_deterministic() {
        let root = tmpdir("order");
        for n in ["c.zip", "a.zip", "b.zip"] {
            touch(&root.join(n));
        }
        let first: Vec<PathBuf> = intake_scan(&root, &opts(&root))
            .into_iter()
            .map(|i| i.path)
            .collect();
        let second: Vec<PathBuf> = intake_scan(&root, &opts(&root))
            .into_iter()
            .map(|i| i.path)
            .collect();
        assert_eq!(first, second);
        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(first, sorted);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_accepts_single_file_root() {
        let root = tmpdir("single");
        let f = root.join("1234567_Solo.zip");
        touch(&f);
        let items = intake_scan(&f, &opts(&root));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, IntakeKind::WithId);
        assert_eq!(items[0].id.as_deref(), Some("1234567"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_dir_avoids_category_and_legacy_prefix() {
        assert!(
            !crate::classify::CATEGORY_MAP
                .values()
                .any(|v| *v == UNKNOWN_DIR)
        );
        assert!(!UNKNOWN_DIR.starts_with(crate::organize::LEGACY_PREFIX));
        assert!(crate::audit::id_dir_of(UNKNOWN_DIR).is_none());
    }

    #[test]
    fn parse_on_unknown_variants() {
        let root = Path::new("R");
        assert_eq!(parse_on_unknown("report", root).unwrap(), OnUnknown::Report);
        assert_eq!(parse_on_unknown("SKIP", root).unwrap(), OnUnknown::Skip);
        assert_eq!(
            parse_on_unknown("move", root).unwrap(),
            OnUnknown::MoveTo(PathBuf::from("R").join(UNKNOWN_DIR))
        );
        assert!(parse_on_unknown("nope", root).is_err());
    }

    #[test]
    fn parse_ambiguous_variants() {
        assert_eq!(parse_ambiguous("ask").unwrap(), AmbiguousPolicy::Ask);
        assert_eq!(parse_ambiguous("Skip").unwrap(), AmbiguousPolicy::Skip);
        assert_eq!(parse_ambiguous("force").unwrap(), AmbiguousPolicy::Force);
        assert!(parse_ambiguous("maybe").is_err());
    }

    #[test]
    fn dry_run_leaves_disk_untouched() {
        let root = tmpdir("dryrun");
        touch(&root.join("readme.txt"));
        let client = Client::builder().build().unwrap();
        let o = IntakeOptions {
            on_unknown: OnUnknown::MoveTo(root.join(UNKNOWN_DIR)),
            ..opts(&root)
        };
        let report = intake_run(&client, &root, &o);
        assert_eq!(report.count(IntakeStatus::Reported), 1);
        assert_eq!(report.count(IntakeStatus::Moved), 0);
        assert!(root.join("readme.txt").is_file());
        assert!(!root.join(UNKNOWN_DIR).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn move_unknown_flattens_and_renames() {
        let root = tmpdir("move");
        touch(&root.join("sub").join("a.txt"));
        touch(&root.join(UNKNOWN_DIR).join("a.txt"));
        let client = Client::builder().build().unwrap();
        let o = IntakeOptions {
            on_unknown: OnUnknown::MoveTo(root.join(UNKNOWN_DIR)),
            dry_run: false,
            ..opts(&root)
        };
        let report = intake_run(&client, &root, &o);
        assert_eq!(report.count(IntakeStatus::Moved), 1);
        // 摊平到根下一层，且既有同名文件不被覆盖。
        assert!(root.join(UNKNOWN_DIR).join("a_1.txt").is_file());
        assert!(root.join(UNKNOWN_DIR).join("a.txt").is_file());
        assert!(!root.join("sub").join("a.txt").exists());
        assert_eq!(report.moved_paths().len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skip_still_listed_in_report() {
        let root = tmpdir("skip");
        touch(&root.join("readme.txt"));
        let client = Client::builder().build().unwrap();
        let o = IntakeOptions {
            on_unknown: OnUnknown::Skip,
            dry_run: false,
            ..opts(&root)
        };
        let report = intake_run(&client, &root, &o);
        assert_eq!(report.count(IntakeStatus::Skipped), 1);
        // Skip 只跳过动作，报告仍必须列出该项。
        assert_eq!(report.unknowns().len(), 1);
        let json = report.to_command_json();
        assert_eq!(json["unknown"].as_array().unwrap().len(), 1);
        assert_eq!(json["skipped"], 1);
        assert!(root.join("readme.txt").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn report_json_envelope_is_stable() {
        let root = tmpdir("json");
        touch(&root.join("1234567_A.zip"));
        let client = Client::builder().build().unwrap();
        let report = intake_run(&client, &root, &opts(&root));
        let json = report.to_command_json();
        assert_eq!(json["command"], "intake");
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["scanned"], 1);
        assert_eq!(json["entries"][0]["kind"], "with-id");
        assert_eq!(json["entries"][0]["status"], "reported");
        assert_eq!(json["entries"][0]["id"], "1234567");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 未确认时按名检索项必须只报告：一旦真去联网，猜错就是错归档，
    /// 而 `--yes` 若只是摆设，用户永远不会知道这道闸存在。
    #[test]
    fn unconfirmed_name_search_does_not_touch_disk() {
        let root = tmpdir("noconfirm");
        touch(&root.join("LunariaPaperFan.zip"));
        let client = Client::builder().build().unwrap();
        let o = IntakeOptions {
            dry_run: false,
            confirm: false,
            ..opts(&root)
        };
        let report = intake_run(&client, &root, &o);
        assert_eq!(report.count(IntakeStatus::Reported), 1);
        assert_eq!(report.count(IntakeStatus::Failed), 0);
        assert!(root.join("LunariaPaperFan.zip").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sidecar_and_archived_are_skipped_without_network() {
        let root = tmpdir("quiet");
        touch(&root.join("3D发型").join("1234567_X").join("pkg.zip"));
        touch(&root.join("3D发型").join("1234567_X").join("cover.jpg"));
        let client = Client::builder().build().unwrap();
        let o = IntakeOptions {
            dry_run: false,
            ..opts(&root)
        };
        let report = intake_run(&client, &root, &o);
        assert_eq!(report.count(IntakeStatus::Skipped), 2);
        assert_eq!(report.count(IntakeStatus::Failed), 0);
        assert!(!report.has_failures());
        let _ = std::fs::remove_dir_all(&root);
    }
}
