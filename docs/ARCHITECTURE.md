# booth-vault-toolhub 架构与开发指南

> 本文档面向希望理解、扩展或接手本仓库的开发者与 Agent。
> 与 README.md（总览）、AGENTS.md（规则与行为契约）互补，本文侧重「系统如何运转」。
> 更新日期：2026-08-17

## 一、系统总览

booth-vault-toolhub 是统一的 BOOTH 素材工具链：下载免费商品、按 ID 归档整理、按文件名搜索、巡检文件夹完整性。技术栈为 **Rust（Tauri v2 + React 19）**，由两个历史 Python 项目（booth-keeper 桌面端、booth-free-collector 下载器）合并重写而来。

核心设计是**三端共享单一引擎**：

```
┌────────────────────────────────────────────────┐
│  GUI (Tauri + React)    CLI (clap)    MCP (rmcp) │
│  └──────────────────────┬───────────────────────┘
│                         ▼
│            ┌────────────────────────┐
│            │      engine crate      │  业务逻辑唯一事实源
│            └───────────┬────────────┘
│                        ▼
│            ┌────────────────────────┐
│            │  shell_win (Windows)   │  平台专属能力
│            └────────────────────────┘
└────────────────────────────────────────────────┘
```

- **engine**：全部业务逻辑（下载/整理/搜索/巡检/配置/网络），跨平台，是唯一事实源。
- **CLI / MCP / GUI**：仅做薄封装与交互适配，不承载业务逻辑。
- **shell_win**：Windows 专属的文件夹图标三件套（`#[cfg(windows)]`），其他平台不编译。
- **平台门控**：Windows 逻辑收敛在 shell_win；非 Windows 平台退化为无图标功能。

## 二、仓库布局

```
booth-vault-toolhub/
├── README.md          ← 项目总览（功能/构建/使用）
├── AGENTS.md          ← 唯一开发者指南（规范/契约/警告事项）
├── docs/              ← 本文档（架构与开发指南）
├── engine/            ← 核心引擎 crate
├── shell_win/         ← Windows 文件夹图标三件套（仅 Windows）
├── booth-mcp/         ← MCP stdio server crate
├── gui/               ← Tauri v2 + React 19 桌面应用
│   ├── src/           ← React 前端
│   └── src-tauri/     ← Rust 后端（Tauri commands）
└── skills/booth/      ← Agent 技能包（SKILL.md + MCP 配置）
```

## 三、engine 模块指南

模块清单（`engine/src/lib.rs`），职责与关键入口：

| 模块 | 职责 | 关键接口 |
|------|------|----------|
| `config` | 应用配置：加载/字段级合并/代理三态解析 | `load_config` / `resolve_proxy` / `default_rate_limit_secs` |
| `session` | 构建 HTTP 会话（Cookie Jar、代理、UA） | `make_session` |
| `http` | HTTP 封装：403/429 指数退避、重试上限 | `get`（退避逻辑，封顶 `MAX_BACKOFF_SECS`） |
| `download` | 免费商品下载：断点续传、假文件校验、限速 | `download_one` |
| `id` | 7 位商品 ID 解析（散链/裸 ID/店铺 URL） | `parse_discrete` |
| `organize` | 归档整理：压缩包 → 按 ID 分类目录 + 封面/图标 | `organize_archive` / `default_icon_fn` |
| `search` | 按名搜索 BOOTH + 店铺翻页抓取 ID | `search_booth` / `crawl_item_ids` |
| `clean` | 文件名清洗（查询候选策略） | `sanitize_query` |
| `norm` | 名称归一化（macOS NFD 兼容） | — |
| `classify` | 商品分类（`CATEGORY_MAP`，53 键固化） | — |
| `score` | 搜索结果评分与歧义判定 | — |
| `cover` | 封面文件识别（HTML 伪装判定） | `looks_html` |
| `audit` | 完整性巡检：三件套缺失检测与修复建议 | `audit_tree_with_fix` / `scan_library` |
| `fetch` | 抓取与响应解析（BOOTH DOM `data-product-id`） | — |
| `update` | 工具自更新检查 | `check_update` |

**扩展准则**：新业务逻辑一律进 engine 对应模块；若属新领域，新建模块并在 `lib.rs` 声明。GUI/MCP 不直接实现业务。

## 四、三端接入点

### CLI（`engine/src/bin/booth/commands.rs`）

- clap 子命令：`download` / `organize` / `search` / `audit` / `update` 等。
- 契约：统一提供 `--json` 结构化输出 + 语义化退出码（见第五节），供 MCP 与脚本可靠判断成败。

### MCP（`booth-mcp/src/tools.rs`）

- rmcp stdio server，工具与 CLI 命令一一对应，JSON 输入输出与 CLI 一致。
- 复用 `engine` 全部逻辑，限速策略在 engine 内部保证，MCP 层不绕过。

### GUI（`gui/src-tauri/src/commands.rs` + React 前端）

- Tauri command 仅做编排：入参校验 → 调用 engine → 通过 `Channel<ProgressEvent>` 流式回传进度。
- **长任务模式**：立即返回 `task_id`，工作放入后台线程；前端用 `TaskRegistry` + `AtomicBool` 协作式取消。
- 前端页面：下载（LinksPage）、整理（DragDropPage）、搜索（SearchPage）、巡检（AuditPage）、设置（SettingsPage）。

## 五、行为契约（节选自 AGENTS.md，改动须重新验证）

- **退出码**：`0` 成功 / `1` 有失败 / `2` 致命。
- **限速**：0.5~0.8s 间隔，三端统一，不得绕过（防 BOOTH 风控）。
- **代理三态**：配置文件 `proxy` > 环境变量 `HTTPS_PROXY`（无缺省回退）> reqwest 系统默认。禁止硬编码个人代理。
- **评分权重**：名称命中 +100 / 规范名命中 +100 / 词级部分匹配 +20 / 过长惩罚 −10；歧义阈值分差 <30。
- **完整性契约**：归档目录须 cover.jpg + .folder_icon.ico + desktop.ini 三件套齐全，巡检可自动修复。
- **假文件校验**：头 256 字节魔数判定 HTML 伪装（未登录返回登录页伪装文件）。
- **HTML 绑定**：解析依赖 BOOTH DOM `data-product-id`，BOOTH 改版即崩；JSON 接口作降级兜底。
- **文件编码**：desktop.ini 写 UTF-8 无 BOM；模板文件保持 ASCII（makensis/candle 按 ANSI 读取）。

## 六、关键决策记录（ADR）

| # | 决策 | 理由 |
|---|------|------|
| 1 | 三端共享 engine crate | 避免 CLI/MCP/GUI 行为漂移，单一事实源，一处修改三端生效 |
| 2 | 自更新用 HTML 重定向法（GET `/releases/latest` 解析 302） | 不消耗 GitHub API 配额，规避 60 次/小时限流；API 兜底 + 403/429 指数退避 |
| 3 | 403 退避仅限 GitHub 域 | BOOTH 域 403 为 Cloudflare 风控/登录页伪装，重试无益反增批量延迟 |
| 4 | 退避封顶 `MAX_BACKOFF_SECS=32` | 杜绝 Retry-After 极端值导致的无限 sleep |
| 5 | Windows 图标逻辑收敛 shell_win crate + cfg 门控 | 平台差异隔离，engine 保持跨平台纯净 |
| 6 | 评分不偏置免费（除非 `prefer_free`） | 免费偏置会把付费商品错配到同名免费兄弟 |
| 7 | 三二进制不挂 `bundle.externalBin`，由 `stage-cli.mjs` 钩子生成安装器片段 | 规避编译期 sidecar 强制存在与 stage-cli 时序冲突 |
| 8 | 无 manifest，纯文件系统推导归档状态 | 存在+非空+非 HTML 伪装即有效，扫描幂等 |

## 七、开发工作流

### 常用命令

```bash
cargo build                        # 构建
cargo test                         # 测试（纯函数层单测先行）
cargo clippy --all-targets --all-features   # lint（提交前必须过，CI 用 -D warnings）
cargo fmt --check                  # 格式检查
cargo run --bin booth -- <download|organize|search|audit> --help
cd gui && npm run tauri dev        # GUI 开发
```

### 提交规范

- 提交信息用**中文**：`<type>: <中文描述>`（feat/fix/docs/refactor/test），一行简明。
- 提交前必须通过：`cargo fmt` + `cargo clippy` + `cargo test`。
- 不提交密钥、Cookie、token（`.gitignore` 已覆盖 `cookie*`、`*.part` 等）。
- 硬编码用户路径一律参数化，禁止写死。

### CI 门禁（`.github/workflows/ci.yml`）

- `encoding`：文件编码检查（BOM/非 UTF-8）。
- `windows`：x64 / x86 / arm64 三架构构建 + 测试。
- `portable`：Linux 与 macOS 的 engine 层测试（Linux 失败不阻塞，macOS 为正式门禁）。
- 合入 master 走 PR + review，不直推。

### 里程碑

M1–M6 已完成（引擎纯函数层 → 网络层 → Shell 图标 → CLI → MCP → GUI）；M7 收敛发布进行中（技能包/打包/回归）。
