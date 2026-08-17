# booth-vault-toolhub 知识库记忆

> 本文档为项目长期运行的知识沉淀：来源、架构、协作约定、关键决策与踩坑实录。
> 维护位置：仓库 `docs/`（随代码版本管理）+ WorkBuddy 资料库（个人检索）。
> 更新日期：2026-08-17

---

## 一、项目是什么

`booth-vault-toolhub` 是统一 BOOTH 素材工具链：用 **Rust（Tauri v2 + React 19）** 重写了两个 Python 项目——`booth-keeper`（桌面端管理）与 `booth-free-collector`（免费商品批量下载/归档）——合并为一个仓库，**CLI / MCP / GUI 三端共享 `engine` crate**（单一事实源）。

- **仓库**：`VRChatCN-Kipfel/booth-vault-toolhub`（public，Apache-2.0）
- **版权**：`NOTICE` 保留主上 `linnnnnnnnnnnnnnnnnnnnn`（小凛酱丷）的 MIT 版权，原始 Python 仓库保留只读归档
- **里程碑**：M1–M6 完成，M7 收敛发布进行中
- **模块**：`engine/`（核心逻辑）、`gui/`（Tauri GUI）、`booth-mcp/`（MCP server）、`shell_win/`（Windows 图标三件套）、`skills/booth/`（agent 技能包）

## 二、功能模块 → 源码落点

| 功能 | 落点 |
|------|------|
| 免费商品下载（断点续传/假文件校验/限速防封） | `engine/src/download.rs` + `http.rs` |
| 按 7 位 ID 归档整理 | `engine/src/organize.rs` + `id.rs` |
| 按文件名搜索 + 评分选优 | `engine/src/search.rs` / `clean.rs` / `norm.rs` / `classify.rs` / `score.rs` |
| 图标三件套巡检 + 自动修复 | `engine/src/audit.rs` + `shell_win/` |
| 工具自更新检查（v1.3.2 对齐新增） | `engine/src/update.rs` |
| 代理配置 | `engine/src/config.rs` / `session.rs` |
| GUI 编排命令 | `gui/src-tauri/src/commands.rs` |

## 三、协作约定（AGENTS.md 摘要）

- **CLI 是 agent 能力的官道**：优先走 `booth` CLI / booth-mcp，不绕开重新实现
- **提交纪律**：conventional-commits 风格，分步提交，禁止巨型 sync commit；**用真实 GitHub 身份提交**（禁止 `agent@...` 类非认证 author）
- **网络红线**：代理裁决复用 `config::resolve_proxy`，禁硬编码；不写 BOM 文件
- **推送约定**：不私自推 master（走分支 + PR），但 admin 协作者有完整 push 权限
- **退出码契约**：`0` 成功 / `1` 有失败 / `2` 致命
- **完整性契约**：整理目录后必须 cover.jpg + .folder_icon.ico + desktop.ini 三件套齐全，跑 `booth audit` 巡检

## 四、关键决策记录（ADR 摘要）

1. **三端共享 engine**：业务逻辑全在 engine，CLI/MCP/GUI 仅薄封装 —— 单一事实源
2. **自更新检查用 HTML 重定向法**（GET `/releases/latest` 解析 302 Location）：不消耗 GitHub API 配额，规避 60 次/小时限流；API 兜底 + 403/429 指数退避 + 代理失败直连重试 + UA 伪装（对齐 Python v1.3.2 updater.py）
3. **403 退避仅限 GitHub API 域**：BOOTH 域 403 是 Cloudflare 风控/登录页伪装，重试无益反而叠加批量延迟
4. **退避时长封顶 `MAX_BACKOFF_SECS=32`**：杜绝 Retry-After 极端值无限 sleep

## 五、踩坑实录（重要）

- **filter-branch 必须限定 range**：`git filter-branch --env-filter ... origin/master..HEAD`，否则会把全历史（含他人提交）重写，导致与远端无共同祖先、PR 无法合并。重写后必须验 `git merge-base origin/master HEAD` 连续
- **git clone 被死代理阻断**：本机 git 全局代理 `127.0.0.1:20122` 不可达 → 用 `gh api .../tarball/master` 下载认证 tarball 绕过；所有 git 命令加 `-c http.proxy= -c https.proxy=`；github.com:443 间歇性阻断，api.github.com 稳定，push 靠抢窗口
- **C 盘 Temp 满致链接失败**：cargo 构建时把 `TEMP/TMP/TMPDIR` 重定向到 D 盘；`NO_PROXY=*` 强制直连
- **tarball 初始化无共同祖先**：先 `git init` 再 fetch origin master，cherry-pick 到真实 master 之上重建 sync

## 六、状态与待办

- [x] PR #18 已合并（工具自更新检查 + 403/429 限流退避，XChen446 approve + merge）
- [ ] 归档原两库（`booth-keeper` / `booth-free-collector` 设只读，可逆）
- [ ] 主上 GitHub 2FA：2026-09-16 前开启（否则影响贡献代码）
- [ ] M7 收敛发布（打包/发布流程收尾）

## 七、访问入口

- 仓库：https://github.com/VRChatCN-Kipfel/booth-vault-toolhub
- 本技能包：`skills/booth/`（含 SKILL.md，可被 agent 加载）
- 本地工作区：`D:\Lin_Agent\WB-WorkSpace\Booth\booth-vault-toolhub`
