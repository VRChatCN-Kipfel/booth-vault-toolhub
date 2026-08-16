# AGENTS.md

> 本文件是唯一开发者指南：工程规范、行为契约、架构、技术选型、警告事项。
> 面向所有在本仓库工作的 agent 与开发者。任务目标与范围见 `README.md`。

## 工作区结构

```
booth-vault-toolhub/
├── README.md              ← 项目总览（功能/架构/使用/构建）
├── AGENTS.md              ← 本文件（规则 + 行为契约 + 警告）
├── .gitignore
├── engine/                ← 核心引擎 crate（三端共享单一事实源）
├── shell_win/             ← Windows Shell 图标三件套 crate（仅 Windows）
├── booth-mcp/             ← MCP stdio server crate
├── gui/                   ← Tauri v2 + React 19 桌面应用
└── skills/booth/          ← Agent 技能包（SKILL.md + MCP 配置）
```

## 语言与技术栈

- Rust（MSRV 1.88）+ React/TypeScript + Tauri v2。
- 三端（CLI/MCP/GUI）共享 `engine` crate，行为一致，避免双引擎漂移。
- GUI 的 `gui/src-tauri/src/commands.rs` 只做编排（async command + Channel 进度 + 协作式取消），不含业务逻辑。

## 架构铁律

- 引擎与界面分离：业务逻辑只进 `engine`，绝不埋进 GUI 进程。
- CLI 是 agent 能力的"官道"：只要 CLI 在，SKILL.md 技能、agent 调用方式原样保留。
- 平台门控：Windows 专属逻辑收敛在 `shell_win`（`#[cfg(windows)]` + feature `windows-shell`）；macOS/Linux 关闭该 feature 退化为无图标功能。

```
┌────────────────────────────────────────────┐
│  GUI (Tauri+React)   CLI (clap)  MCP (rmcp)  │
│  └─────────────┬───────────────────────────┘ │
│                ▼                             │
│   ┌──────────────────────────────┐           │
│   │         engine crate          │          │
│   └──────────────┬───────────────┘           │
│                  ▼                           │
│   ┌──────────────────────────────┐           │
│   │      shell_win crate          │          │
│   └──────────────────────────────┘           │
└────────────────────────────────────────────┘
```

## 常用命令

```bash
cargo build                 # 构建
cargo test                  # 单测（纯函数层必须先行）
cargo clippy --all-targets --all-features   # lint（提交前必须过）
cargo fmt --check           # 格式检查
cargo run --bin booth -- <download|organize|search|audit> --help
cd gui && npm run tauri dev # GUI 开发
```

- 提交前必须跑：`cargo fmt` + `cargo clippy` + `cargo test`。
- 若无法确定正确命令，先问用户，而不是猜。

## 协作与流程

- **勤派 subagent**：代码库探索、并行研究、可独立拆分的子任务，优先用 Task 工具派发，主 agent 专注主流程；完成后汇总精简结论。

## 编码规范

- **禁止添加注释**，除非是警告事项/行为契约说明（描述"为什么"而非"抄自哪"）。
- 模仿现有代码风格；新代码遵循 crate 内已有约定。
- 提交信息用**中文**，格式：`<type>: <中文描述>`（type: feat/fix/docs/refactor/test），一行简明。
- 硬编码用户路径（如 `G:\Lin_File\BOOTH`）一律参数化，不得写死。
- 不提交密钥、Cookie、token；`.gitignore` 覆盖 `cookie*`、`*.part`、`desktop.ini`、`manifest_*.json`。

## 行为契约（线上调优结果，改动须重新验证）

> 这些数字/规则是线上验证过的行为契约。改动会影响整理/搜索结果准确性，须重新验证后才能改。

### 评分与清洗

- 评分权重：名称归一化命中 `+100`、规范名命中 `+100`、词级部分匹配(≥3字符) `+20`、BOOTH 次序微权 `max(0, 10-idx*2)`、过长惩罚 `-10`。
- 歧义阈值：最优与次优分差 `<30` 判歧义；同名不同价必报歧义。
- 单结果也须名称命中，否则解 .unitypackage 资源名二次验真。
- 3 处环视正则（ID 边界×2、驼峰拆词）用 `fancy-regex`，不得改用标准 `regex` 改变行为。
- `sanitize_query` 7 层候选策略（下划线→空格/去括号/驼峰拆词/纯日文主体/去版本号/去尾部中文/最长 ASCII 段/去 VRChat 停用词），逐条实现不得"优化"。
- 评分不偏置免费（免费偏置会把付费商品错配到同名免费兄弟）——除非明确要求 prefer_free。
- `CATEGORY_MAP` 已去重固化（53 键），任何修改必须同步 GUI/CLI/MCP 三端。

### 限速与输出

- 限速 0.5~0.8s，GUI/CLI/MCP 三端统一，不得绕过（防 BOOTH 风控）。
- 统一 CLI 必须提供结构化输出（`--json`）与语义化退出码（0 成功/1 有失败/2 致命），MCP 依赖它判断成败。
- 网络代理为**配置项**，不得硬编码个人代理地址。优先级：配置文件 `proxy` > `HTTPS_PROXY`（无缺省回退）> reqwest 系统默认（Windows 读系统代理注册表）。

## 警告事项（血泪坑，防回归）

### Windows Shell / 图标

1. **装饰 Unicode 目录名 → Explorer 永久拒读 desktop.ini**：逐字符过滤 emoji（0x1F300-0x1F9FF、0x2000-0x27BF、0x2B0-0x2FF、0x2070-0x209F）及 Mn/Me/Cn 类，保留 ASCII/中日韩/全角，截断 80。
2. **宽幅封面非正方形 ICO**：先贴 `max(w,h)` 透明正方形画布居中再保存，6 尺寸 256/128/64/48/32/16。
3. **desktop.ini/ico 必须 H+S 同设**（HIDDEN 0x02 + SYSTEM 0x04），只设 H 会被 Explorer 拒读。
4. **写前清 0x80=NORMAL 属性**，否则只读/系统文件覆写失败；失败时清理残缺 ini 并回滚。
5. **完整性契约**：ico 存在且 >1KB、ini 含 `IconResource=.folder_icon.ico`、文件夹 READONLY 位，三件套自检不过即清理重写。
6. **PIDL 生命周期**：`SHParseDisplayName` → `SHChangeNotify(SHCNE_UPDATEITEM, SHCNF_IDLIST)` → `CoTaskMemFree`。
7. **desktop.ini 编码契约**：写为 UTF-8 无 BOM；audit 三编码读回兼容（utf-16→utf-8→gbk）。
8. **SHChangeNotify 事件码**：`SHCNE_UPDATEITEM(0x2000)` + `SHCNE_UPDATEDIR(0x1000)` 兜底；**Win11 刷新不可靠，优先 `SHGetSetFolderCustomSettings`**（FCS_FORCEWRITE|FCSM_ICONFILE）。

### 数据 / 命名

9. 版本号保留：`extract_version_tag` 输出 `Ver_x.y`，整理名不得丢版本（内部文件名保持原文件名）。
10. 单结果也须名称命中，否则解 .unitypackage 资源名验真（zip→gzip→tar→pathname）。
11. HTML 解析绑定 BOOTH DOM（`data-product-id`），booth 改版即崩；保留原样正则防行为漂移，JSON 接口作降级兜底。

### 幂等 / 归档

12. 无 manifest，纯文件系统推导状态：存在+非空+非 HTML 伪装即有效，扫描幂等。
13. **假文件魔数校验**：头 256 字节 lstrip 后 `<!doctype`/`<html` 即判伪（未登录返回伪装 zip/png 的登录页 HTML）。
14. **移动后属性丢失 → 图标失效**：copy 后重补属性；跨盘移动保留 mtime。
15. 空目录链清理：跳过隐藏文件（desktop.ini/Thumbs.db/.DS_Store），walk-up 清理 max 6 级，root 不删。
16. macOS NFD 归一化：去重/比对前 `unicode-normalization` 归一化。

### 安装器 vendor 模板 / PATH 契约

17. `gui/src-tauri/wix/main.wxs` 与 `gui/src-tauri/nsis/installer.nsi` **vendor 自 tauri-bundler v2.11.5**：
    升级 Tauri 大版本必须对照官方模板同步合并（文件头有警告注释），否则 MSI/NSIS 打包可能失效。
    模板一律 **ASCII**：Tauri 渲染输出无 BOM，makensis/candle 按 ANSI 读，中文会乱码。
18. 三个 booth 二进制**不**挂 `bundle.externalBin`（GUI 编译期强制 sidecar 文件存在，与 stage-cli 时序冲突），
    由 `beforeBundleCommand` 钩子 `gui/scripts/stage-cli.mjs` 生成：
    - MSI fragment `gui/src-tauri/wix/generated.wxs`（绝对路径），经 `featureRefs` 挂 `External` feature；
    - NSIS `{project_out}/nsis/generated.nsh` + 把 EnVar.dll 预置到 `{project_out}/nsis/plugins/`。
    **路径基准**：tauri-cli 打包前 `set_current_dir(dirs.tauri)`（切到 src-tauri），故
    `template`/`fragmentPaths`/`nsis.template` 均相对 src-tauri 解析。
19. **四可选组件契约**（booth CLI / booth MCP / booth Shell / Add to user PATH，默认全选）：
    PATH 写**用户级 HKCU**（perMachine 安装也写当前用户），卸载按原生机制清理
    （MSI `Environment` 表 `Action=set Part=last` / NSIS `EnVar::DeleteValue`），
    重装幂等不重复追加（NSIS 已实测）。改动须重新实测安装/卸载/重装。

## 已修复的原实现缺陷（记录备查，不得回退）

| 缺陷 | 修复 |
|---|---|
| `prefer_free` 死参数（从未生效） | 实现语义：同分候选免费项 +5 加成 |
| Range 下载空 .part 死循环 | 用探针拿 total、total==0 建空文件、每块独立重试 |
| Range 下载 `max_retry` 死参数 | 每块真实重试 |
| Range 下载无探针反复重下 | 先发 `Range: bytes=0-0` 验证 Content-Range |
| 代理个人硬编码 `127.0.0.1:20122` | 改为配置三态（配置 > 环境变量 > 系统默认） |

## 依赖管理

- 大版本锁定：`rmcp = "3"`、`reqwest = "0.13"`、`tauri = "2"`、`image = "0.25"`、`windows = "0.62"`。
- 升级依赖前先读官方迁移指南（如 rmcp 3.x 有 breaking 记录）。
- 新增依赖需说明理由，避免无谓膨胀。
- **优先用成熟库，不重复造轮子**；对不了解的库/生态/版本，先用 doko 查官方文档实证再决策，不得凭记忆或猜。

## 测试要求

- 纯函数层（分类/清洗/评分/解析）**单测先行**，与旧实现输出 diff 对拍（golden tests 在 `engine/tests/`）。
- Windows Shell 模块改动后必须实机验证（Explorer 大图标预览 + 图标缓存刷新）。
- GUI 视觉验收用离屏截图对比旧版，不依赖肉眼。

## 网络与资料

- 国内环境：网页检索默认用 **Bing**（`dokobot read --local 'https://www.bing.com/search?q=...'`），避免百度。
- 对不确定的技术点，先用 doko 打开官方文档页细读，不得凭记忆下结论。

## 安全

- 代码不得暴露或记录密钥/Cookie；隐私数据仅存本地用户目录。
- 未登录状态要防"假文件"：校验魔数（头 256 字节 HTML 前缀），不得信任响应体表面类型。

## 里程碑与验收

| 阶段 | 内容 | 状态 |
|---|---|---|
| M1 | 引擎纯函数层（分类/清洗/评分/解析） | ✅ |
| M2 | 网络层（会话/下载/搜索解析） | ✅ |
| M3 | Windows Shell 图标三件套 | ✅ |
| M4 | 统一 CLI + JSON + 退出码 | ✅ |
| M5 | MCP server | ✅ |
| M6 | Tauri + React GUI | ✅ |
| M7 | 收敛发布（技能/打包/回归） | 进行中 |

验收标准：
- [ ] engine 层在 Windows/macOS/Linux 三平台测试全绿。
- [ ] 同一输入，CLI/MCP/GUI 三端输出与行为一致（单一事实源）。
- [ ] 三主题六配色与旧版视觉对齐（截图对比）。
- [ ] 文件夹图标三件套 + Explorer 刷新实机验证通过。
- [ ] agent 技能切换入口后全流程可用（下载/整理/搜索/巡检）。
- [ ] 退出码 + JSON 输出可供 MCP 可靠判断成败。
