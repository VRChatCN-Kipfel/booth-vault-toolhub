---
name: booth-toolkit
description: |-
  Unified BOOTH toolkit for VRChat / XR creators — Rust rewrite (CLI + MCP).
  Bundles download / organize / search / audit for the full BOOTH asset lifecycle:
  (1) download fetches a shop's free items or scattered share-links ("免费鸡蛋" =
  free BOOTH goods in VRChat slang); (2) organize tidies a local archive whose
  filename embeds a 7-digit BOOTH item id; (3) search finds a BOOTH product by
  filename when no id is present; (4) audit inspects folder-icon three-piece
  completeness. Prefer the `booth` CLI (or booth-mcp) over raw scripts — same
  engine, single entry. Trigger words: 下载免费鸡蛋、booth下载、免费商品下载、
  VRChat免费素材、booth归档、下载booth店铺、免费鸡蛋、领鸡蛋、白嫖鸡蛋、下载散链、
  朋友发的booth、booth整理、整理booth压缩包、归档booth文件、整理这个booth包、
  booth文件归类、给这个压缩包重命名、按名字搜booth、搜booth商品名、整理vrc插件道具、
  找booth商品、按文件名搜索、booth按名搜索、整理vrc素材、整理着色器、booth巡检.
agent_created: true
---

# BOOTH Toolkit — BOOTH 素材全家桶（Rust 版）

BOOTH（日本数字创作集市，VRChat 素材主产地）素材的**下载 / 归档 / 按名搜索整理 / 巡检**四件套。
统一输出 `类目中文\ID_标题\` 目录结构（含 `cover.jpg` 封面 + Windows 文件夹图标）。

## 统一入口（三选一，同一引擎）

| 入口 | 说明 | 适用 |
|------|------|------|
| **`booth` CLI** | 编译后的二进制，五子命令 | 终端/agent 首选 |
| **booth-mcp** | MCP stdio server，暴露同五工具 | opencode / Claude Code 等 MCP 客户端 |
| 原 `python scripts/booth.py` | 旧版 Python（仅参考，已废弃） | 不推荐 |

## 找到 CLI / MCP 二进制（先定位，再调用）

`booth`、`booth-mcp`、`booth-shell` 三个二进制**永远与主程序 `booth-keeper` 同目录**：

| 安装形态 | 二进制位置 |
|------|------|
| 便携版 zip | 解压目录（与 `booth-keeper.exe` 同目录） |
| 安装版（MSI/NSIS） | `C:\Program Files\booth-keeper\`（与主程序同目录） |
| 源码构建 | 仓库 `target/release/`（`--target` 时在 `target/<triple>/release/`） |

定位命令（Windows）：

```powershell
where.exe booth booth-mcp        # 已在 PATH 时直接命中
Get-ChildItem "$env:ProgramFiles\booth-keeper" -Filter 'booth*.exe'
```

若 `booth-mcp` 不在 PATH，MCP 客户端配置改填绝对路径（示例见 `skills/booth/README.md`）。
**优先走 `booth` CLI（官方通道），不要绕开或重新实现。**

```bash
booth download <店铺URL|散链> [--cookie ...] [--out DIR]   # 下载免费商品
booth organize <本地包...> [--id ID] [--out DIR]           # 按 ID 整理归档
booth search   <本地文件...> [--id ID] [--base-dir DIR]    # 按名搜索整理
booth audit    [--base DIR] [--dry-run]                    # 图标三件套巡检
booth update-check [--proxy]                               # 工具自更新检查
```

全局 `--json` 输出结构化结果，退出码语义：`0` 成功 / `1` 有失败项 / `2` 致命错误。

## MCP 接入（可选）

以 stdio server 运行 `booth-mcp`，同五工具（download/organize/search/audit/update_check），
JSON 输入输出与 CLI 完全一致。客户端配置示例（`.mcp.json` 片段）：

```json
{ "mcpServers": {
  "booth": { "command": "booth-mcp", "args": [] }
} }
```

## 子命令路由（决策树）

```
用户丢来一个文件 / 链接
├─ 是 BOOTH 店铺 URL 或商品散链（booth.pm/...）
│   └─► booth download              （从网上下载免费商品，需 Cookie）
├─ 是本地压缩包，文件名含 7 位数字（如 跟随悬浮机-6504842等3个文件.rar）
│   └─► booth organize              （按 ID 取元数据整理）
├─ 是本地压缩包，无 ID，是 BOOTH 商品名（如 SimpleJoinAlert_v100.zip）
│   └─► booth search                （按名字搜索 + 水印/UnityPackage 辅助识别）
└─ 主上想让整个库图标整齐 / 图标异常
    └─► booth audit                 （三件套巡检 + 自动修复）
└─ 询问工具是否有新版本 / 版本号
    └─► booth update-check [--proxy]（GitHub 查最新 release，HTML 重定向法规避限流）
```

## 关键共享知识

- **BOOTH 搜索端点**：`https://booth.pm/ja/items?q=词`（`?q=`，非 `?keyword=`）
- **公开元数据 API**：`https://booth.pm/ja/items/id.json`（免登录）
- **封面图 CDN**：`booth.pximg.net`（公开可达）
- **分类汉化**：engine `CATEGORY_MAP`（3Dテクスチャ→3D贴图 等）；未知类目保留日文原名不臆造
- **免费偏置禁忌**：整理已有文件时评分**不偏置免费**，避免付费商品错配到同名免费兄弟
- **文件名清洗**：去版本号/中文备注/括号、下划线→空格、驼峰拆词、纯日文主体
- **压缩包水印识图**：搜索无果读 `*.url`/`readme` 提取店铺 URL，走 `/items?page=N` 反查
- **UnityPackage 内部资源名是硬线索**：首段目录名=店铺名/作者名，内部 prefab/anim=商品主题
- **商品页下载文件名=终极锚点**：歧义时以实际免费文件名与本地版本号匹配确认
- **隐私铁律**：Cookie 仅存本机配置（应用级 config），**绝不上传 GitHub**

## 防错速查

| 症状 | 根因 | 修复 |
|------|------|------|
| 封面图标「居中小图」外留白 | cover 是宽幅矩形 | 正方形画布 paste 后保存（引擎内置） |
| 图标不显示 | 目录名含装饰 Unicode，Explorer 不读 desktop.ini | `sanitize_filename` 过滤 + 重启电脑 |
| 商品误配张冠李戴 | 单结果盲信 / 标题相似 | 名称归一化必须命中 + 解 UnityPackage 校验 |
| 整理后文件名版本号丢失 | 用标题生成文件名 | **内部文件名保持原文件名** |
| 商品页多免费版本本地只有 1 个 | 未检查其他免费版本 | `backfill_free_files` 自动补全（需 `--cookie`） |

## 完整性契约（强制规则）

**任何 agent 用本 Skill 整理商品目录后，必须满足三件套齐全**：
1. `cover.jpg`（商品首图）
2. `.folder_icon.ico`（≥1KB，含 256×256 正方形条目）
3. `desktop.ini`（必须含 `IconResource=.folder_icon.ico,0`）+ Hidden+System

引擎 `make_folder_icon` 内置完整性自检，写完立刻读回校验，缺失即清理不留半成品。
**整理后应主动跑 `booth audit` 巡检一遍。**

## 输出目录结构

```
<归档根目录>\            （配置 download_root，非硬编码路径）
└── 类目中文tag\
    └── ID_标题\
        ├── ID_标题.ext   原文件（下载/移动/复制，保留原文件名含版本号）
        ├── cover.jpg           商品首图
        ├── .folder_icon.ico    (隐藏)
        └── desktop.ini         (隐藏+系统)
```

## 非 BOOTH 商品处理

部分 VRC 素材不在 BOOTH 上（如 Poiyomi Toon 走 GitHub 分发）。
search 判定「所有搜索 + 水印探测均无果」时保留源文件不整理，交主上确认来源平台。

## 配置

`download_root` / 代理 / Cookie 读配置文件（用户目录与应用目录两处支持）。
代理优先级：配置文件 `proxy` > `HTTPS_PROXY` > 系统默认。
