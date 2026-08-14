# AGENTS.md

> 本文件定义在本工作区工作的**工程规则**（怎么做）。方案、目标、技术选型见 `SOLUTION.md`——两者不混写。
> 新会话/环境切换：先读 `SOLUTION.md` §11 会话交接清单 → §1~§10，再回本文件。

## 工作区结构

```
booth-vault-toolhub/      ← 本工作区根（统一重写项目，旧名"新建文件夹 (7)"）
├── AGENTS.md              ← 规则（本文件）
├── SOLUTION.md            ← 方案文档（只读参考，禁止与规则混写）
├── .gitignore             ← 已排除两个旧仓库
├── booth-keeper/          ← 旧：PySide6 桌面向（只读参考，勿改）
└── booth-free-collector/  ← 旧：Python CLI/Skills（只读参考，勿改）
```

- 两个旧仓库为**只读参考**，除非显式要求，不得修改其内容。
- 新建代码一律进入统一项目（engine/gui/shell_win，见 SOLUTION.md M0）。
- 本工作区当前**不是** git 仓库；新建统一项目仓库时，把 `AGENTS.md`、`SOLUTION.md` 一并提交。

## 文档职责边界（重要）

- `AGENTS.md` = 规则：命令、规范、铁律、禁止项。
- `SOLUTION.md` = 方案：背景、架构、技术选型、路线图、血泪坑清单。
- 两者不得互相渗透：规则不入方案，方案细节（如坑的具体坐标）不入规则。

## 语言与技术栈

- Rust（MSRV 1.88，rmcp 3.0 要求）+ React/TypeScript + Tauri v2。
- 复用现有 Python 实现时，以 `SOLUTION.md` §8 血泪坑清单为准逐条复刻，不得自作主张简化。
- 迁移基线："legacy 完整实现 + 统一版入口结构"（见 SOLUTION.md §7）。

## 工具链状态（2026-08-15 实测，接手时先核对）

- **Rust 已装**：cargo 1.97.1 / rustup（`RUSTUP_HOME=D:\rustup`、`CARGO_HOME=D:\cargo`，PATH 已含 `D:\cargo\bin`），MSRV 1.88 满足。
- **MSVC 编译器可用**：cl.exe/link.exe 在 `D:\VS2022\VC\Tools\MSVC\14.44.35207`（19.44）；**Windows SDK 10.0.26100 在 C 盘 `Program Files (x86)\Windows Kits\10\`**。
- **VS 安装器整体"失败"**（DiagnosticsHub 服务无法配置，错误 1939/1603），但编译所需文件全部落盘，已通过补 `SxS\VC7` 注册表让 rustc 找到 MSVC（见 SOLUTION.md §11.5），`cargo build` 已实测可用。**不要重跑 VS 安装器去"修复"**，无必要。
- **Node 已可用**：`node` v24.12.0、`npm` 11.18.0。
- 根目录**已 `git init`**（分支 master，尚未提交）：待首提交 `AGENTS.md`/`SOLUTION.md`/`.gitignore`。

## 常用命令

```bash
cargo build                 # 构建
cargo test                  # 单测（纯函数层必须先行）
cargo clippy --all-targets --all-features   # lint（提交前必须过）
cargo fmt --check           # 格式检查
cargo tauri dev             # GUI 开发（需已建 gui crate）
cargo run --bin booth -- <download|organize|search|audit> --help
```

- 提交前必须跑：`cargo fmt` + `cargo clippy` + `cargo test`。
- 若无法确定正确命令，先问用户，而不是猜。

## 协作与流程

- **勤派 subagent**：代码库探索、并行研究、可独立拆分的子任务，优先用 Task 工具派发 subagent，主 agent 专注主流程，避免重复劳动与上下文膨胀；完成后汇总精简结论。

## 编码规范

- **禁止添加注释**，除非是血泪坑记录（移植自 Python 的坑注释必须保留，注明原因）。
- 模仿现有代码风格；新代码遵循 crate 内已有约定。
- 提交信息用**中文**，格式：`<type>: <中文描述>`（type: feat/fix/docs/refactor/test），一行简明。
- 硬编码用户路径（如 `G:\Lin_File\BOOTH`）一律参数化，不得写死。
- 不提交密钥、Cookie、token；`.gitignore` 覆盖 `cookie*`、`*.part`、`desktop.ini`、`manifest_*.json`。

## 移植铁律（不得违反）

> 判断原则：**行为契约一致，实现缺陷必修**。行为契约（评分权重、清洗策略、歧义阈值、目录结构）以线上验证结果为准，逐项实现并测试；实现缺陷（死参数、死循环、个人环境硬编码、不可达逻辑）必须主动修复，不得照抄。修复清单见 SOLUTION.md §7.1「已修复的原实现缺陷」。

1. 装饰 Unicode 过滤、正方形画布 ICO、H+S 同设、写前清 0x80、完整性契约、PIDL 生命周期、desktop.ini 编码契约——见 SOLUTION.md §8.1，逐条实现并测试，缺一不可。
2. 3 处环视正则用 `fancy-regex`，不得改用标准 `regex` 后改变行为。
3. 评分权重、歧义阈值、sanitize_query 7 层策略按行为契约实现，不得"优化"（数字是线上调优结果，改动需重新验证）。
4. 限速策略（0.5~0.8s）GUI/CLI/MCP 三端统一，不得绕过。
5. 统一 CLI 必须提供结构化输出（JSON）与语义化退出码，MCP 依赖它判断成败。
6. CATEGORY_MAP 已去重固化，任何修改必须同步三端。
7. 移植中发现原实现缺陷时，修复后必须记录到 SOLUTION.md §7.1，并在代码中不标注"抄自某处"（注释只描述行为本身）。

## 依赖管理

- 大版本锁定：`rmcp = "3"`、`reqwest = "0.13"`、`tauri = "2"`、`image = "0.25"`、`windows = "0.62"`。
- 升级依赖前先读官方迁移指南（如 rmcp 3.x 有 breaking 记录）。
- 新增依赖需说明理由，避免无谓膨胀。
- **优先用成熟库，不重复造轮子**；对不了解的库/生态/版本，先用 doko 查官方文档实证再决策，不得凭记忆或猜。

## 测试要求

- 纯函数层（分类/清洗/评分/解析）**单测先行**，与 Python 旧实现输出 diff 对拍。
- Windows Shell 模块改动后必须实机验证（Explorer 大图标预览 + 图标缓存刷新）。
- GUI 视觉验收用离屏截图对比旧版，不依赖肉眼。

## 网络与资料

- 国内环境：网页检索默认用 **Bing**（`dokobot read --local 'https://www.bing.com/search?q=...'`），避免百度。
- 网络代理为**配置项**，不得硬编码个人代理地址。优先级：配置文件 `proxy` > 环境变量 `HTTPS_PROXY`（仅标准通道，无缺省回退值）> reqwest 系统默认（Windows 读系统代理注册表）。配置文件在用户目录与应用目录两处均支持。
- 对不确定的技术点，先用 doko 打开官方文档页细读，不得凭记忆下结论。

## 安全

- 代码不得暴露或记录密钥/Cookie；隐私数据仅存本地用户目录。
- 未登录状态要防"假文件"：校验魔数（头 256 字节 HTML 前缀），不得信任响应体表面类型。
