# booth-toolkit 技能安装说明

BOOTH 技能包：`SKILL.md`（父技能）+ `booth` CLI + `booth-mcp`。

## 快速安装技能（SKILL.md）

推荐用成熟安装器 [skills.sh](https://github.com/vercel-labs/skills)（`npx skills`）：

```bash
# 从本仓库安装（本地路径）
npx skills add /path/to/booth-vault-toolhub/skills/booth

# 或从 GitHub 仓库安装（发布后）
npx skills add <owner>/booth-vault-toolhub --skill booth

# 指定目标 agent（opencode / claude-code 等）
npx skills add <owner>/booth-vault-toolhub --skill booth -a opencode -a claude-code

# 非交互批量
npx skills add -y --all --skill '*' -a opencode -a claude-code /path/to/booth-vault-toolhub/skills
```

`npx skills` 自动把 SKILL.md 写入各 agent 的标准技能目录：
- opencode：项目 `.agents/skills/` 或全局 `~/.config/opencode/skills/`
- Claude Code：`.claude/skills/`

## 接入 MCP（可选）

`booth-mcp` 是 stdio MCP server，暴露 download/organize/search/audit 四工具。

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

### Claude Code

`.mcp.json`：

```json
{
  "mcpServers": {
    "booth": { "command": "booth-mcp", "args": [] }
  }
}
```

### 可视化管理（可选）

- **MCPane**（Microsoft Store）：统一 MCP Hub，图形化注册多个客户端
- **mcp-manager**（GitHub xjeway/mcp-manager）：跨平台桌面配置管理

## 构建 booth CLI / booth-mcp

```bash
cargo build --release --workspace
# 产物：target/release/booth、target/release/booth-mcp
```

建议将 `booth` 和 `booth-mcp` 加入 PATH，技能即可直接调用。

## 版本约定

- `booth` CLI 退出码：0 成功 / 1 有失败项 / 2 致命错误
- `--json` 全局输出结构化结果
