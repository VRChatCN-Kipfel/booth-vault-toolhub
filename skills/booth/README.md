# booth-toolkit 技能安装说明

BOOTH 技能包：`SKILL.md`（父技能）+ `booth` CLI + `booth-mcp`。

## 快速安装技能（SKILL.md）

推荐用成熟安装器 [skills.sh](https://github.com/vercel-labs/skills)（`npx skills`）：

```bash
# 从本仓库安装（本地路径）
npx skills add /path/to/booth-vault-toolhub/skills/booth

# 或从 GitHub 仓库安装
npx skills add VRChatCN-Kipfel/booth-vault-toolhub --skill booth

# 指定目标 agent（opencode / claude-code 等）
npx skills add VRChatCN-Kipfel/booth-vault-toolhub --skill booth -a opencode -a claude-code

# 非交互批量
npx skills add -y --all --skill '*' -a opencode -a claude-code /path/to/booth-vault-toolhub/skills
```

`npx skills` 自动把 SKILL.md 写入各 agent 的标准技能目录：
- opencode：项目 `.agents/skills/` 或全局 `~/.config/opencode/skills/`
- Claude Code：`.claude/skills/`

## 接入 MCP（可选）

`booth-mcp` 是 stdio MCP server，暴露 download/organize/search/audit 四工具。

### 安装版 / 便携版的 CLI 位置

`booth` / `booth-mcp` / `booth-shell` 三个二进制**与主程序 `booth-keeper` 同目录**：

- 便携版 zip：解压目录
- 安装版（MSI/NSIS）：`C:\Program Files\booth-keeper\`

若不在 PATH，用绝对路径（例如 `C:\Program Files\booth-keeper\booth-mcp.exe`）。
建议把该目录加入 PATH 后技能即可直接调 `booth`：

```powershell
# 把 booth 二进制目录加入当前用户 PATH
$dir = "$env:ProgramFiles\booth-keeper"
[Environment]::SetEnvironmentVariable('Path', "$dir;$env:Path", 'User')
```

### opencode

编辑 `opencode.json`，合并 `skills/booth/mcp.example.json` 的内容：

```json
{
  "mcp": {
    "booth": {
      "type": "stdio",
      "command": "booth-mcp",
      "args": []
    }
  }
}
```

若 `booth-mcp` 不在 PATH，`command` 改用绝对路径：

```json
{
  "mcp": {
    "booth": {
      "type": "stdio",
      "command": "C:\\Program Files\\booth-keeper\\booth-mcp.exe",
      "args": []
    }
  }
}
```

### Claude Code

`.mcp.json`：

```json
{
  "mcpServers": {
    "booth": { "command": "booth-mcp", "args": [] }
  }
}
```

同样，不在 PATH 时改用绝对路径 `"command": "C:\\Program Files\\booth-keeper\\booth-mcp.exe"`。

### 可视化管理（可选）

- **MCPane**（Microsoft Store）：统一 MCP Hub，图形化注册多个客户端
- **mcp-manager**（GitHub xjeway/mcp-manager）：跨平台桌面配置管理

## 构建 booth CLI / booth-mcp

```bash
cargo build --release --workspace
# 产物：target/release/booth、target/release/booth-mcp
```

建议将 `booth` 和 `booth-mcp` 加入 PATH，技能即可直接调用。

## 打包 GUI（`tauri build` 自动带 CLI）

`tauri build` 的 `beforeBundleCommand` 钩子（`gui/scripts/stage-cli.mjs`）会自动从
`target/**/release/` 复制 CLI 三件套到 `gui/src-tauri/resources/` 并打进安装器/便携版，
无需手动复制。前提是先构建 CLI：

```bash
cargo build --release --workspace
cd gui && npm run tauri build
```

## 版本约定

- `booth` CLI 退出码：0 成功 / 1 有失败项 / 2 致命错误
- `--json` 全局输出结构化结果
