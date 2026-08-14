# BOOTH 一体化重写方案文档（方案 A：Tauri + React + 全 Rust 引擎）

> 项目代号：**booth-vault-toolhub**（统一重写项目，旧工作区名"新建文件夹 (7)"）
> 状态：评估完成，风险点已全部实证核验，待立项实施
> 日期：2026-08-14
> 本文档只描述"做什么、为什么、怎么做"，不包含工程规则。工程规则见 `AGENTS.md`。

---

## 1. 背景与目标

### 1.1 现状：两个同源仓库

| 仓库 | 形态 | 功能 | 核心文件 |
|---|---|---|---|
| `booth-keeper` | Windows 桌面应用（Python 3.13 + PySide6） | BOOTH 资产本地整理：批量链接解析、拖拽分类、实验检索、目录/错位/版本三重巡检、三主题六配色 UI | `booth_core.py`(759行)、`archive_util.py`、`theme.py`(919行)、`pages/*` |
| `booth-free-collector` | AI Agent Skills + CLI（Python 3.10+） | BOOTH 免费商品批量下载归档：整店/散链自动判定、variation 级免费筛选、中文分类归档、Windows 封面图标、假文件校验 | `scripts/booth.py`(707行)、`scripts/booth_common.py`(681行)、`scripts/legacy-20260803/*` |

- 两者核心逻辑**同源**：`booth-keeper/booth_core.py` 是作者从 Agent 技能三合一抽取的共享层，与 `booth_common.py` 高度重叠。
- `booth-free-collector` 本地分支 `patch-1` 已快进合并至 `upstream/main`（booth-toolkit 三合一版本），当前为最新。

### 1.2 目标

将两个项目**重写为一体**：
1. **统一 Rust 引擎**：一份代码同时支撑三种接口——GUI（Tauri + React）、CLI（clap）、MCP server（rmcp）。
2. **保持审美**：三主题六配色、SVG 母题纹样在 React/CSS 下 1:1 复刻且可做得更好。
3. **不输 agent 能力**：CLI/MCP 作为 agent 能力的"官道"保留，`SKILL.md` 技能协议不变，只改入口命令。
4. 把握窗口期：项目尚小（约 1.3 万行 Python），趁早重写比后期渐进迁移（方案 B/C）代价更低。

---

## 2. 方案选择结论

**方案 A（全 Rust 重写）技术上站得住，正式采用。** 三大审计结论：

1. **约 70% 代码可干净平移**：网络层、分类映射、文件名清洗、版本提取、zip/tar 解析、评分选优、归档移动、巡检扫描、配置读写，Rust 生态有直接等价物。
2. **约 10% 是 Windows-only 硬骨头**：`make_folder_icon` 三件套 + SHChangeNotify/PIDL，风险集中但 windows crate 原生支持，可控。
3. **React/CSS 反而更好做**：QSS 全局样式表隔离坑、`@keyframes` 缺失、离屏渲染差异、原生弹窗不跟随主题等问题在 Web 栈下天然消失。

### 2.1 架构铁律：引擎与界面分离

- `booth.py` 这类 CLI 必须保持为**单一事实源引擎**，逻辑绝不埋进 GUI 进程。
- 只要 CLI 在，`SKILL.md` 技能、opencode 等 agent 调用方式原样保留。
- 三个接口共享同一 `engine` crate，行为一致、避免双引擎漂移。

---

## 3. 目标架构

```
┌──────────────────────────────────────────────────────┐
│                     booth-toolkit                     │
│                                                       │
│  ┌───────────┐   ┌───────────┐   ┌───────────┐       │
│  │   GUI     │   │   CLI     │   │ MCP server│       │
│  │ Tauri+React│  │   clap    │   │   rmcp    │       │
│  │ (gui crate)│  │ (bin)     │   │  (bin)    │       │
│  └─────┬─────┘   └─────┬─────┘   └─────┬─────┘       │
│        │               │               │              │
│        └───────────────┼───────────────┘              │
│                        ▼                             │
│  ┌──────────────────────────────────────────┐        │
│  │               engine crate               │        │
│  │  HTTP/解析/图像/清洗/评分/归档/幂等      │        │
│  │  （零平台依赖，三平台共用）               │        │
│  └──────────────────────────────────────────┘        │
│                        │                             │
│                        ▼                             │
│  ┌──────────────────────────────────────────┐        │
│  │              shell_win crate             │        │
│  │  desktop.ini / 属性位 / ICO /            │        │
│  │  SHChangeNotify  (cfg(windows))          │        │
│  └──────────────────────────────────────────┘        │
└──────────────────────────────────────────────────────┘
```

- **crate 拆分**：`engine`（纯逻辑+IO，可测可复用于三端）｜`gui`（Tauri）｜`shell_win`（windows crate，`#[cfg(windows)]` + 可选 feature `windows-shell`）。
- **平台门控**：Windows 专属逻辑单一收敛在 `shell_win`；macOS/Linux 关闭该 feature 即退化为无文件夹图标功能，不影响主流程。
- **异步模型**：GUI 侧用 async command + `spawn_blocking`（CPU 密集图像处理），替代 PySide6 的 8 个 QThread；**顺带补上原版缺失的取消机制**。

---

## 4. 技术选型（已全部实证核验）

| 层 | 选型 | 版本/要点 | 核验状态 |
|---|---|---|---|
| Windows Shell | `windows` crate | 0.62，`SetFileAttributesW`/`SHChangeNotify`/`SHParseDisplayName` 原生绑定；无高层抽象，需手写约 50 行 unsafe FFI | ✅ docs.rs 实测 |
| HTTP | `reqwest` | 0.13.4；`system-proxy` 默认读 `HTTP_PROXY/HTTPS_PROXY`（也读 Windows 系统代理注册表，注意 `.no_proxy()` 控制）；`cookie_store`、`retry` 模块、手动 Range header | ✅ 官方 README |
| HTML 解析 | `scraper` | 0.27，Servo 级 CSS 选择器；booth.pm 为 SSR 页面可直接解析 | ✅ |
| JSON | `serde` / `serde_json` | `#[serde(default)]` 容错字段变动 | ✅ |
| 图像 | `image` | 0.25；`IcoEncoder`+`IcoFrame` 多帧 PNG 压缩 ICO，满足 256px 大图标硬性要求（256 存为 0） | ✅ docs.rs 源码确认 |
| 正则 | `regex` + `fancy-regex` | 标准 regex 不支持环视；3 处 lookaround（ID 边界×2、驼峰拆词）用 fancy-regex（回溯混合引擎，fancy 部分委托 regex 保持线性；输入为短文件名/链接，无 ReDoS） | ✅ 官方页确认 |
| GUI | `tauri` v2 + React | 2.11；拖拽/无边框/主题/async 全内建；`tauri://drag-*` 事件，`dragDropEnabled` 控制 DOM 级拖拽 | ✅ 示例+中文社区实证 |
| CLI | `clap` | 4.6，derive 子命令，argparse 超集 | ✅ |
| MCP | `rmcp` | **3.0.0（2026-07-28 发布）**；`#[tool_router]` 宏用户工具签名不变；`LATEST` 默认 2025-11-25，新协议需显式 `V_2026_07_28`；MSRV 1.88 | ✅ 迁移指南全文细读 |
| Unicode | `unicode-general-category` + `unicode-normalization` | 装饰字符过滤、macOS NFD 归一化 | ✅ |
| 其他 | `percent-encoding`、`html-escape`、`zip`+`tar`+`flate2` | URL quote、HTML 转义、.unitypackage 解包 | ✅ |

**依赖锁定铁律**：`rmcp = "3"`、`reqwest = "0.13"`、`tauri = "2"` 等大版本锁定，跟 release 时先读迁移指南。

---

## 4.1 便捷包装库选型（2026-08-15 doko 调研，Bing + 官方文档实证）

> 原则：避免重复造轮子，优先成熟库与官方插件。下表均已核实版本与兼容性（与 Tauri 2.x / Rust MSRV 1.88 兼容）。

### 系统侧（engine / shell_win）

| 用途 | 选型 | 版本 | 说明 |
|---|---|---|---|
| 系统路径 | `dirs` / `directories` | 6.0 | home/config/data_local；应用级目录用 directories |
| 文件对话框 | `tauri-plugin-dialog`（官方） | 2.7 | 不用 `rfd`（Tauri 下需自己处理消息循环） |
| 文件监控 | `notify` + `notify-debouncer-mini` | 8.2 | 下载完触发整理；配防抖；不得绕过三端统一限速 |
| 回收站 | `trash` | 5.2 | 跨平台移到回收站 |
| 打开/定位 | `open`（引擎侧）/ `tauri-plugin-opener`（GUI） | 5.4 / 2.5 | 系统默认程序打开、资源管理器定位 |
| Windows 杂项 | `win-desktop-utils`（评审后取用） | 0.5.7 | 快捷方式/提权/explorer reveal；下载量小、单人维护，逐 API 评审 |

### Tauri 官方插件（`tauri-plugin-* = "2"` / `@tauri-apps/plugin-*`）

| 插件 | 用途 | 采纳 |
|---|---|---|
| fs | 前端读写文件系统（需 capabilities 配 scope） | 必用 |
| dialog | 原生对话框 | 必用 |
| window-state | 记住窗口大小/位置 | 建议 |
| notification | 系统通知（Windows 仅对已装 app 生效，dev 显示 powershell 图标） | 建议 |
| store | 配置持久化 KV | 建议 |
| opener | 默认程序打开 / revealItemInDir | 建议 |

拖拽接收文件路径**不是插件**，是 Tauri core 能力：`getCurrentWebview().onDragDropEvent` 拿绝对路径（`dragDropEnabled` 默认 true；Windows 想用 HTML5 DnD 才需设 false）。

### 前端（React）

| 方向 | 选型 | 说明 |
|---|---|---|
| 主题（三主题六配色） | CSS variables + styled-components v6（`createTheme` 转 CSS 变量） | 不引重库；Radix Themes 太重不引入 |
| SVG 青海波纹样 | 自写纯函数生成 path（约 30 行） | `d3-shape` 可作备选，不引整包 |
| 拖拽排序 | `@dnd-kit/core` + `@dnd-kit/sortable` | react-dnd 已停更 4 年，弃 |
| 状态管理 | `zustand` 5 | 中大型桌面应用首选 |
| 图标 | `lucide-react` | tree-shaking 干净；不用 react-icons |
| 无边框窗口 | `decorations:false` + `data-tauri-drag-region` | 官方方案，另需 capability `core:window:allow-start-dragging` |

### 文件夹图标三件套：无成熟 crate，定为"蓝本移植 + 自写"

crates.io 检索 `folder icon`（449 条）/`desktop.ini`（10 条）均无成熟可依赖库（desktop-ini 为 GPL 且是 CLI；win-folder-refresh 只刷新不设图标；icon-sys 未发布）。定为 **vendor 摘取改造**：

| 蓝本 | License | 用途 |
|---|---|---|
| `icon-sys`（github.com/ecoates2/icon-sys，未发布 crates.io） | MIT OR Apache-2.0 | **主流程**：ICO 多尺寸（image 0.25）+ H+S 属性 + `SHGetSetFolderCustomSettings` 写 desktop.ini + 唯一文件名刷新策略 |
| `win-folder-refresh`（github.com/goddivor） | MIT | SHChangeNotify 兜底事件序列 |
| `desktop-ini` 0.1.7 | GPL-3.0（**不可并入**） | 仅参考 INI 写入契约 / ACP 编码 |
| `folderikon` / `holdyounger/folder-icon`（Python） | MIT | 整体逻辑参考 |
| 本地 `booth_common.py` `make_folder_icon` | — | **行为基线（铁律）**：完整性契约 / 正方形画布 / audit 三编码回读，逐条复刻 |

**实现策略**：M3 先以 `icon-sys = { git = "https://github.com/ecoates2/icon-sys", rev = "8edd743c624683e22a4951c672cfec2c645d2b0e" }` 快速验证流程（`rev` 可精确锁定 commit，可复现）→ 验证后 **vendor 摘取**进 `shell_win`（文件头注明出处 + 保留 MIT/Apache 版权声明），再按行为基线深度改造。自写规模约 350–550 行，unsafe 约 5–10 处（PIDL 生命周期）。

---

## 5. 风险点核验汇总（2026-08 实证）

| 原风险 | 核验结果 | 落地指引 |
|---|---|---|
| Tauri 拖拽 Windows 路径编码 | 降级为成熟方案 | v2 默认拦截拖拽走 `tauri://drag-*`；`tauri://` 事件无法定位具体 DOM（用相对坐标），需要"拖到拖放区才高亮"时设 `dragDropEnabled:false` 走原生 HTML5 DnD；拖入路径被 Tauri 自动 scope |
| rmcp API churn | 确认但可控 | 3.0 已发布且 conformance 40/40 全过；宏用户无签名改动；锁定 `rmcp = "3"` |
| image ICO PNG 压缩 | 确认可行 | `IcoFrame::as_png()` 多帧；256px PNG 压缩即 Windows Vista+ 大图标预览硬性要求 |
| fancy-regex 性能/安全 | 确认可用 | 输入域受限（文件名/链接），无 ReDoS 实际风险 |
| reqwest 系统代理 | 确认默认行为 | 默认读 env 代理；Windows 下也读系统代理注册表。**代理为配置项，禁止硬编码个人地址**：优先级 配置文件 `proxy` > `HTTPS_PROXY`（无缺省回退）> reqwest 系统默认；必要时 `.no_proxy()` |
| Linux 打包 | 确认复杂度 | Debian 依赖 `libwebkit2gtk-4.1-0`/`libgtk-3-0`；必须用 Ubuntu 22.04/Debian 12 基线构建，建议 Docker/GH Actions；AppImage 有 WebKit 注入包缺失已知问题 |
| macOS/Linux PATH | 新增发现 | GUI 不继承 shell 点文件 `$PATH`，需要时用 `fix-path-env-rs` |

---

## 6. 多平台支持矩阵

| 能力 | Windows | macOS | Linux |
|---|---|---|---|
| desktop.ini + .folder_icon.ico | ✅ 原生（shell_win） | ❌ 不实现 | ❌ 不实现 |
| 属性位 / SHChangeNotify | ✅ 原生 | ❌ | ❌ |
| 256px 大图标预览 | ✅ Explorer 渲染 | ⚠️ 文件可写无预览 | ⚠️ 同左 |
| HTTP/HTML/JSON/图像/正则 | ✅ | ✅ | ✅ |
| Unicode 文件名 | ✅（保留字符/大小写敏感） | ⚠️ NFD 归一化坑 | ✅（字节路径） |
| Tauri GUI | ✅ WebView2 | ✅ WKWebView | ✅ WebKitGTK（打包繁琐） |
| CLI / MCP | ✅ | ✅ | ✅ |

---

## 7. 迁移基线（关键决策）

**统一版相对 legacy 存在功能回退，移植基线取"legacy 完整实现 + 统一版入口结构"：**

| 项 | 取基线 | 原因 |
|---|---|---|
| audit ICO 方形检查 | legacy 版（恢复） | 统一版已删，丢失一项校验 |
| `_ranged_download` 断点续传 | legacy 版 | 统一版无 total 探针、无每块重试、缺头可能死循环 |
| CATEGORY_MAP 重复 key | 显式去重后按最后值固化 | `アバター`(头像→虚拟形象)、`アクセサリー`(饰品→配饰) 依赖 Python 后写覆盖；Rust 编译期会报 duplicate key |
| CLI 入口/子命令结构 | 统一版 | download/organize/search/audit 四子命令为纲 |

### 7.1 已修复的原实现缺陷（移植时发现，主动修复）

> 判断原则（AGENTS.md 移植铁律）：行为契约一致，实现缺陷必修。以下为旧实现中确认的缺陷，Rust 版已修复。修复**不在代码中标注来源**，注释只描述行为本身。

| # | 缺陷 | 位置 | 修复方式 |
|---|---|---|---|
| 1 | `prefer_free` 死参数：签名声明但函数体从未使用，付费/免费偏好从未生效 | `score_and_pick`（统一版 + legacy 相同） | 实现语义：`prefer_free=true` 时同分候选免费项 +5 加成；配单测 `score_prefer_free_tiebreak` |
| 2 | `_ranged_download` 空 `.part`（size=0）死循环 | 统一版 `booth.py` | 用 legacy 健壮版：Range 探针拿 total、total==0 直接建空文件返回、每块独立重试 |
| 3 | `_ranged_download` 的 `max_retry` 参数声明但从未使用 | 统一版 `booth.py` | 每块按 `RANGE_MAX_RETRY` 真实重试 |
| 4 | `_ranged_download` 无 Range 探针，服务端不支持时反复重下 | 统一版 `booth.py` | 先发 `Range: bytes=0-0` 验证 `Content-Range`，无则报错不再死磕 |
| 5 | 代理个人硬编码 `http://127.0.0.1:20122/` 缺省回退 | `make_session`（统一版 + legacy 相同） | 改为配置三态：配置文件 `proxy` > `HTTPS_PROXY`（无缺省回退）> 系统默认；配置文件用户目录与应用于录两处支持 |

**统一版对统一引擎必须补强（MCP 基础设施）：**
1. **结构化输出**：当前 CLI 恒零退出码 + 纯中文 stdout，MCP 无法判断成败 → 加 JSON 输出 + 语义化退出码。
2. **路径参数化**：硬编码默认路径 `G:\Lin_File\BOOTH`（4 处）改为配置注入。
3. **并发安全**：`_json_cache` 模块级共享（原靠 GIL）→ Rust 用锁/异步缓存。
4. **限速策略保持**：串行 sleep 限速（0.5~0.8s 区间）在 GUI 并发场景不得绕过，防封禁。

---

## 8. 血泪坑移植清单（行为契约，必须逐条复刻）

> 本清单全部是**线上验证过的行为契约**（Explorer/BOOTH 交互行为、编码、评分阈值），逐条实现并测试，缺一不可；若某条本身是实现缺陷而非行为，按 §7.1 判断原则修。

> 来源：`booth-keeper/booth_core.py` 注释 + `SCORE_TABLE_R7.md` + `booth-free-collector` 各 SKILL.md

### 8.1 Windows Shell / 图标
1. **装饰 Unicode 目录名 → Explorer 永久拒读 desktop.ini**：逐字符过滤 emoji（0x1F300-0x1F9FF、0x2000-0x27BF、0x2B0-0x2FF、0x2070-0x209F）及 Mn/Me/Cn 类，保留 ASCII/中日韩/全角，截断 80 字符。
2. **宽幅封面非正方形 ICO**：先贴 `max(w,h)` 透明正方形画布居中，再保存，6 尺寸 256/128/64/48/32/16。
3. **desktop.ini/ico 必须 H+S 同设**（HIDDEN 0x02 + SYSTEM 0x04），只设 H 会被 Explorer 拒读。
4. **写前清 0x80=NORMAL 属性**，否则只读/系统文件覆写失败；失败时清理残缺 ini 并回滚。
5. **完整性契约 `IconContractError`**：ico 存在且 >1KB、ini 含 `IconResource=.folder_icon.ico`、文件夹 READONLY 位，三件套自检不过即清理重写。
6. **PIDL 生命周期**：`SHParseDisplayName` → `SHChangeNotify(SHCNE_UPDATEITEM, SHCNF_IDLIST)` → `CoTaskMemFree`；注意 Python 版第 4 参传 byref 属隐藏类型错误，Rust 要正确声明 `PPIDLIST_ABSOLUTE`。
7. **desktop.ini 编码契约统一**：当前代码为 UTF-8 无 BOM（`[ViewState]` 在前），文档与 legacy 声称 UTF-16——移植时以"能写能读 + audit 三编码读回兼容（utf-16→utf-8→gbk）"为准，先对齐文档。
8. **SHChangeNotify 事件码（旧实现有错，移植必须修正）**：Python 基线用 `0x8`（SHCNE_MKDIR）、`0x8000`（SHCNE_UPDATEIMAGE）刷新文件夹图标是**错误事件**。正确组合：`SHCNE_UPDATEITEM(0x2000)` + `SHCNE_UPDATEDIR(0x1000)` 兜底，可加 `SHCNE_ASSOCCHANGED(0x08000000)` 全局兜底；PIDL 用 `ILCreateFromPathW` + `SHCNF_IDLIST|SHCNF_FLUSH`。**Win11 实测 SHChangeNotify 刷新文件夹图标并不可靠**（ie4uinit/nircmd 亦无效），真正可靠的是 `SHGetSetFolderCustomSettings`（FCS_FORCEWRITE|FCSM_ICONFILE，废弃但仍可用，内部即 SetFileAttributes(H+S)+PathMakeSystemFolder）——icon-sys 走此路径，优先采用，SHChangeNotify 仅作兜底。
9. **desktop.ini 设置图标不需要 `[ViewState]`**（微软规范只要求 PathMakeSystemFolder + H+S + Unicode），但本地基线含 `[ViewState] FolderType=Generic`，移植铁律要求逐条复刻，应保留。

### 8.2 数据/命名
10. **3 处环视正则**：`BARE_ID_RE`/`ID_RE`（`(?<!\d)`/`(?!\d)`）与驼峰拆词 `(?<=[a-z])(?=[A-Z])` → 统一用 fancy-regex。
11. **sanitize_query 7 层候选策略**：全名→驼峰拆词→纯日文主体→去版本→去尾部中日文→最长 ASCII 段→VRC 停用词剥离，去重保序——评分权重（+100/+20/-10、分差<30 判歧义）逐一复制。
12. **版本号保留**：`メカ弾エフェクトVer_2.00` → 整理名不得丢版本，`extract_version_tag` 输出 `Ver_x.y`。
13. **单结果也须名称命中**，否则解 .unitypackage 资源名验真（zip→gzip→tar→pathname）。
14. **HTML 解析脆弱**：搜索/店铺翻页正则绑定 BOOTH DOM（`data-product-id`），booth 改版即崩；保留原样正则防行为漂移，另做 JSON 接口降级兜底。

### 8.3 幂等/归档
15. **无 manifest，纯文件系统推导状态**：`valid_file`（存在+非空+非 HTML 伪装）扫描幂等；`.part` 原子 rename + Range 续传复用。
16. **假文件魔数校验**：头 256 字节 lstrip 后 `<!doctype`/`<html` 即判伪（未登录返回伪装 zip/png 的登录页 HTML）。
17. **移动后属性丢失 → 图标失效**：copy 后重补属性；跨盘移动保留 mtime（Rust `fs::copy` 不保留，需 `set_file_times`）。
18. **空目录链清理**：跳过隐藏文件（desktop.ini/Thumbs.db/.DS_Store），walk-up 清理 max 6 级，root 不删。
19. **macOS NFD 归一化**：去重/比对前 `unicode-normalization` 归一化。

---

## 9. 落地路线图

### M0 立项与骨架（预估 1-2 天）
- 新建工作区：`booth-toolkit` 统一仓库，`engine`/`gui`/`shell_win` 三 crate 骨架。
- 复制 `SOLUTION.md` + `AGENTS.md` 入库。
- CI：Windows 主目标 + Linux/macOS 门控（engine 层三平台跑测试）。

### M1 引擎核心：纯函数层（ROI 最高，先行）
- CATEGORY_MAP（去重固化）/ classify 回退链 / sanitize_filename / sanitize_query / extract_version_tag / 评分选优 / ID 与链接解析。
- 全部为无 IO 纯函数，**单测先行**，逐条对照 Python 行为（血泪坑 §8.2）。
- 交付物：`engine` crate + 全量单测，与 Python 输出 diff 对拍。

### M2 引擎网络层
- reqwest 会话（代理三态：配置文件 `proxy` > 环境变量 `HTTPS_PROXY` > 系统默认，**禁硬编码个人地址**）、cookie 三态加载、fetch_item JSON、搜索/店铺翻页 HTML 解析、封面下载（Referer 头）、流式下载 + `.part` + Range 续传（**用 legacy 健壮版**）、假文件校验、限速。
- 交付物：download 全流程 CLI 可用。

### M3 Windows Shell 模块（最高风险，单独里程碑）
- `shell_win`：属性位、desktop.ini 写入、ICO 生成、SHChangeNotify（全局 + PIDL 单目录）、完整性契约。
- **实机验证**：写后 Explorer 大图标预览 + 图标缓存刷新，对照 §8.1 全部 7 条。
- 交付物：文件夹图标三件套 CLI 子命令。

### M4 CLI 统一与 agent 补强
- clap 四子命令（download/organize/search/audit）+ 自动分流。
- **新增**：`--json` 结构化输出 + 语义化退出码；路径参数化。
- 同步更新各 `SKILL.md` 入口命令（`python scripts/booth.py ...` → `booth ...`），协议与触发词不变。
- 交付物：CLI 完全对齐 Python 行为，agent 技能可切换。

### M5 MCP server
- rmcp 3.0 stdio server，`#[tool_router]` 暴露 download/organize/search/audit 四工具；JSON 输入输出复用 engine。
- 并发安全：缓存加锁；限速不绕过。
- 交付物：`booth-mcp` 二进制，opencode 等 agent 可通过 MCP 调用。

### M6 GUI（Tauri + React）
- 复刻三主题六配色（CSS variables + styled-components）+ SVG 母题生成器（JSX 组件，`_seigaiha_layer` 算法保留为 TS 循环）。
- 四页面对应：批量链接 / 拖拽分类 / 实验检索 / 巡检设置；QThread → async command + 进度事件，**补取消机制**。
- 拖拽：默认 `tauri://drag-*`；需要拖放区定位高亮时用原生 HTML5 DnD（`dragDropEnabled:false`）。
- 交付物：桌面应用功能对齐 booth-keeper，含无边框窗口、主题化弹窗（React modal）。

### M7 收敛与发布
- 删除 Python 双仓库实现（冻结归档），Git 历史保留。
- Windows NSIS 安装包（Tauri bundle 自带）+ 三平台产物。
- 回归：对拍 Python 旧版行为，UI 视觉离屏验收。

---

## 10. 验收标准

- [ ] engine 层在 Windows/macOS/Linux 三平台测试全绿。
- [ ] 同一输入，CLI/MCP/GUI 三端输出与行为一致（单一事实源）。
- [ ] 三主题六配色与 PySide6 版视觉对齐（截图对比）。
- [ ] 文件夹图标三件套 + Explorer 刷新实机验证通过。
- [ ] agent 技能切换入口后全流程可用（下载/整理/搜索/巡检）。
- [ ] 退出码 + JSON 输出可供 MCP 可靠判断成败。

---

## 11. 会话交接清单（2026-08-14 环境切换用）

> 本会话即将结束（文件夹将重命名，会话一并丢失）。此节为后续会话的唯一交接入口，环境切换后先读本节 + AGENTS.md + §1~§10。

### 11.1 当前物理状态

| 项 | 值 |
|---|---|
| 工作区根目录 | `C:\Users\30885\Documents\新建文件夹 (7)`（将重命名为 `booth-vault-toolhub`） |
| 统一项目代号 | booth-vault-toolhub（尚未初始化 git 仓库） |
| 已落盘文档 | `AGENTS.md`（规则）、`SOLUTION.md`（方案）、`.gitignore`（已排除两个旧仓库） |
| 方案状态 | 方案 A 已定稿，风险点实证核验完毕，等待 M0 立项 |

### 11.2 两个旧仓库的 git 拓扑（重要，勿混淆）

**`booth-keeper/`**（只读参考）
- 来源：`git@github.com:linnnnnnnnnnnnnnnnnnnnn/booth-keeper.git`
- 单提交扁平发布（1.0.0），分支 `main`，无历史演进。

**`booth-free-collector/`**（只读参考，本会话已做过操作）
- `origin` = `git@github.com:XChen446/booth-free-collector.git`
- `upstream` = `git@github.com:linnnnnnnnnnnnnnnnnnnnn/booth-free-collector.git`（本会话新增）
- 本地分支 `patch-1` 已**快进合并**至 `upstream/main`（`9b0930a` → `a673e12`，21 提交，booth-toolkit 三合一新架构），工作树干净。
- 本地 `patch-1` 仍落后 `origin/patch-1` 远程（未 push）。如需同步远程，执行 `git push origin patch-1`。

### 11.3 本会话已完成（不必重做）

1. 分析了两个仓库的功能与技术栈（结论见 §1）。
2. 三路 subagent 并行审计：booth-keeper 可移植性、collector 可移植性、Rust 生态/多平台（结论并入 §2~§7）。
3. 六路 doko Bing 搜索 + 详情页细读，实证核验全部风险点（结论见 §5）。
4. 起草 `SOLUTION.md` + `AGENTS.md` + `.gitignore`。

### 11.4 下一步待办（M0）

1. ~~将文件夹重命名为 `booth-vault-toolhub`~~（已完成）。
2. ~~根目录 `git init`~~（已完成，分支 master，尚未提交）。**待首提交**：`git add AGENTS.md SOLUTION.md .gitignore`。
3. 按 §9 M0：建 `engine`/`gui`/`shell_win` 三 crate 骨架 + CI（工具链已就绪，见 §11.5）。
4. 依序实施 M1（纯函数层，单测先行）→ M2（网络层）→ M3（Win32 shell）→ M4（CLI+JSON+退出码）→ M5（MCP）→ M6（GUI）→ M7（收敛发布）。

### 11.5 环境与工具备忘

- **国内网络**：网页检索默认用 Bing（`dokobot read --local 'https://www.bing.com/search?q=...'`），**避免百度**。
- **代理**：**配置项，禁止硬编码个人地址**。优先级：配置文件 `proxy`（用户目录 > 应用目录）> 环境变量 `HTTPS_PROXY`（无缺省回退）> reqwest 系统默认（Windows 读系统代理注册表）。必要时 `.no_proxy()`。
- **依赖锁定**：`rmcp = "3"`（MSRV 1.88）、`reqwest = "0.13"`、`tauri = "2"`、`image = "0.25"`、`windows = "0.62"`。
- **当前本机状态（2026-08-15 实测）**：
  - Rust **已装**：cargo 1.97.1 / rustup（`RUSTUP_HOME=D:\rustup`、`CARGO_HOME=D:\cargo`，PATH 含 `D:\cargo\bin`）。
  - **MSVC 编译器**在 `D:\VS2022\VC\Tools\MSVC\14.44.35207`；**Windows SDK 10.0.26100** 在 C 盘 `Program Files (x86)\Windows Kits\10\`。
  - **VS 安装器整体失败**（DiagnosticsHub.Collection.Service 服务配置失败，错误 1939/1603，疑似火绒拦截/权限），但编译所需文件全部落盘。**已通过手动补注册表接入**：`HKLM\SOFTWARE\Microsoft\VisualStudio\SxS\VC7` 和 WOW6432Node 同路径设 `14.0 = "D:\VS2022\VC\"`，rustc 即可定位 MSVC，`cargo build` 实测可用。**勿重跑 VS 安装器修复**。
  - crates 镜像已由用户配置（rsproxy 等，`D:\cargo\config.toml`）。
- **参考旧实现基线**：collector 以 `scripts/legacy-20260803/*`（更完整实现）+ 统一版 `booth.py` 入口结构为基准（见 §7），两份都留在只读仓库中。
