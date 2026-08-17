# booth-vault-toolhub

[![CI](https://img.shields.io/github/actions/workflow/status/VRChatCN-Kipfel/booth-vault-toolhub/ci.yml?branch=master&label=CI)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub/actions)
[![Release](https://img.shields.io/github/actions/workflow/status/VRChatCN-Kipfel/booth-vault-toolhub/release.yml?branch=master&label=Release%20Workflow)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub/actions)
[![Rust](https://img.shields.io/badge/Rust-1.88+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

[![Version](https://img.shields.io/github/v/release/VRChatCN-Kipfel/booth-vault-toolhub?label=Release)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub/releases)
[![Release Date](https://img.shields.io/github/release-date/VRChatCN-Kipfel/booth-vault-toolhub?label=更新日期)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub/releases)
[![Downloads](https://img.shields.io/github/downloads/VRChatCN-Kipfel/booth-vault-toolhub/total?label=Downloads)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub/releases)

[![Stars](https://img.shields.io/github/stars/VRChatCN-Kipfel/booth-vault-toolhub?label=Stars)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub)
[![Forks](https://img.shields.io/github/forks/VRChatCN-Kipfel/booth-vault-toolhub?label=Forks)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub)
[![Last Commit](https://img.shields.io/github/last-commit/VRChatCN-Kipfel/booth-vault-toolhub?label=最近提交)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub)
[![Code Size](https://img.shields.io/github/languages/code-size/VRChatCN-Kipfel/booth-vault-toolhub?label=代码体积)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub)
[![Top Lang](https://img.shields.io/github/languages/top/VRChatCN-Kipfel/booth-vault-toolhub?label=主要语言)](https://github.com/VRChatCN-Kipfel/booth-vault-toolhub)

BOOTH 素材统一管理工具 —— VRChat / XR 创作者的 BOOTH 资产全流程工具链。
用 Rust 统一重写原 Python 双仓库（[booth-keeper](https://github.com/linnnnnnnnnnnnnnnnnnnnn/booth-keeper) + [booth-free-collector](https://github.com/linnnnnnnnnnnnnnnnnnnnn/booth-free-collector)），
一份引擎同时支撑 **CLI / MCP / GUI** 三种接口。

### 鸣谢

- GUI 设计提供：[linnnnnnnnnnnnnnnnnnnnn/booth-keeper](https://github.com/linnnnnnnnnnnnnnnnnnnnn/booth-keeper)
- CLI 灵感提供：[linnnnnnnnnnnnnnnnnnnnn/booth-free-collector](https://github.com/linnnnnnnnnnnnnnnnnnnnn/booth-free-collector)

## 功能

- **下载**：整店爬取 / 散链下载 BOOTH 免费商品（variation 级免费筛选、断点续传、假文件校验、限速防封）
- **整理**：本地压缩包文件名含 7 位 ID → 按 ID 取元数据归档（无登录）
- **按名搜索**：文件名无 ID 时，清洗生成搜索候选 → 评分选优 → 归档（含水印/UnityPackage 二次验真）
- **巡检**：文件夹图标三件套（cover.jpg + .folder_icon.ico + desktop.ini）完整性扫描 + 自动修复 + 版本巡检 + 错位纠正
- **自更新检查**：GitHub 查最新 release（HTML 重定向法规避 API 限流 + 403/429 退避 + 代理直连重试），CLI/MCP 可用；GUI 预留 backend `update_check` 命令，前端入口待接入
- **三主题六配色 GUI**：朱印（印泥/宣纸）/ 鎏金（金缮/漆）/ 古纹（青铜/青瓷）× 亮 / 暗，形制与色板分开，SVG 母题纹样，动效齐全

## 架构

```
┌────────────────────────────────────────────┐
│  GUI (Tauri+React)    CLI (clap)   MCP (rmcp) │
│  └─────────────┬───────────────────────────┘ │
│                ▼                             │
│   ┌──────────────────────────────┐           │
│   │         engine crate          │ 分类/清洗/评分/解析/HTTP/归档/巡检 │
│   └──────────────┬───────────────┘           │
│                  ▼                           │
│   ┌──────────────────────────────┐           │
│   │      shell_win crate          │ Windows 文件夹图标三件套 │
│   └──────────────────────────────┘           │
└────────────────────────────────────────────┘
```

| crate | 职责 | 平台 |
|---|---|---|
| `engine` | 纯函数层 + 网络 + 归档 + 巡检，三端共享单一事实源 | 三平台 |
| `shell_win` | 属性位 / desktop.ini / ICO / SHChangeNotify（图标三件套） | Windows only |
| `shell_mac` | 封面正方形化 + Finder 自定义文件夹图标 | macOS only |
| `booth-mcp` | MCP stdio server，暴露四工具 | 三平台 |
| `gui` | Tauri v2 + React 19 桌面应用 | 三平台 |

## 快速开始

### 构建

```bash
cargo build --release --workspace
# 产物：target/release/booth、booth-mcp、booth-shell
```

> 发布包（安装版/便携版）中 `booth` / `booth-mcp` / `booth-shell` 与主程序
> `booth-vault-toolhub` 同目录（安装版在 `C:\Program Files\booth-vault-toolhub\`），agent/MCP
> 直接按此定位，无需另行安装。
>
> MSI/NSIS 安装器均带可选组件页（booth CLI / booth MCP / booth Shell / Add to user PATH，
> 默认全勾选）：默认把安装目录加入当前用户 PATH（幂等，卸载自动移除）。
> **主程序每次启动会把自身目录写入用户环境变量 `BOOTHVAULT_TOOLHUB`**（便携版同样生效），
> agent 定位工具目录优先读该变量（验证路径存在），再回退 PATH / 常规位置。
> macOS：写入 `launchctl setenv` + `~/Library/LaunchAgents/com.boothvault.toolhub.env.plist`，
> 工具在 `.app/Contents/MacOS/`。

### CLI

```bash
booth download <店铺URL|散链> [--cookie ...] [--out DIR]   # 下载免费商品
booth organize <本地包...> [--id ID] [--out DIR]           # 按 ID 整理归档
booth search   <本地文件...> [--id ID] [--base-dir DIR]    # 按名搜索整理
booth audit    [--base DIR] [--dry-run]                    # 图标三件套巡检
booth update-check [--proxy]                               # 工具自更新检查
```

- 全局 `--json`：结构化输出（MCP 依赖）
- 退出码：`0` 成功 / `1` 有失败项 / `2` 致命错误
- 每个子命令 `--help` 有详细说明

### GUI

```bash
cd gui
npm install
npm run tauri dev        # 开发
npm run tauri build      # 打包
```

### MCP

```bash
booth-mcp                # stdio server，暴露 download/organize/search/audit
```

客户端配置示例见 `skills/booth/mcp.example.json`。

### Agent 技能（SKILL.md）

技能包在 `skills/booth/`，可用成熟安装器 [skills.sh](https://github.com/vercel-labs/skills) 安装：

```bash
# 从 GitHub 安装
npx skills add VRChatCN-Kipfel/booth-vault-toolhub --skill booth -a opencode -a claude-code

# 或本地仓库直接装
npx skills add ./skills/booth -a opencode -a claude-code
```

详见 `skills/booth/README.md`。

### 克隆仓库

```bash
git clone https://github.com/VRChatCN-Kipfel/booth-vault-toolhub.git
```

## 配置

配置文件（TOML，用户目录与应用目录两处支持）：

```toml
proxy = "http://127.0.0.1:7890"   # 可选；优先级 配置文件 > HTTPS_PROXY > 系统默认
proxy_enabled = true               # false = 强制直连（忽略环境变量/系统代理）
download_root = "D:/BOOTH"         # 归档根目录（不硬编码路径）
rate_limit_secs = 0.8              # 限速秒数（三端统一，防封）
cookie = ""                        # 可选；仅用户目录。CLI --cookie / MCP 参数优先
```

- 用户目录：`{config_dir}/booth-vault-toolhub/config.toml`（GUI 设置页写入这里，三端共用）
- 应用目录：exe 同目录或 CWD 的 `config.toml`

## 目录结构

```
<归档根目录>/
└── 类目中文tag/
    └── ID_标题/
        ├── ID_标题.ext   原文件（保留原文件名含版本号）
        ├── cover.jpg
        ├── .folder_icon.ico    (隐藏)
        └── desktop.ini         (隐藏+系统)
```

## 开发

```bash
cargo build                 # 构建
cargo test                  # 单测
cargo clippy --all-targets --all-features
cargo fmt --check
```

## 许可

Apache License 2.0 — 见 [LICENSE](LICENSE)。

本项目部分逻辑参考并衍生自 [linnnnnnnnnnnnnnnnnnnnn](https://github.com/linnnnnnnnnnnnnnnnnnnnn)
的 MIT 开源项目（见 [NOTICE](NOTICE)），版权与许可声明在 NOTICE 中完整保留。