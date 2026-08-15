# booth-vault-toolhub GUI

Tauri v2 + React 19 + TypeScript 桌面应用。

## 开发

```bash
npm install
npm run tauri dev
```

## 构建

```bash
npm run tauri build
```

## 前端结构

```
src/
├── theme/      主题色板（三主题六配色）+ SVG 母题生成器 + 全局样式
├── store/      zustand 状态（主题/配置/UI）
├── hooks/      系统明暗主题 hook
├── components/ 通用组件（侧栏/弹窗/滑块/状态栏等）
├── pages/      五页面（批量链接/拖拽分类/实验检索/目录巡检/设置）
└── App.tsx     根组件（主题注入 + 布局 + 导航）
```

Rust 侧：`src-tauri/src/commands.rs`（四工具 async command + Channel 进度 + 协作式取消）。
GUI 复用 `engine` crate，无独立业务逻辑（单一事实源）。
