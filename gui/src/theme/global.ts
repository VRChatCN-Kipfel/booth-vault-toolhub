/**
 * 全局样式：CSS variables 注入 + 基础 reset。
 * styled-components 的 createGlobalStyle，消费主题 store 提供的色板。
 */

import { createGlobalStyle } from 'styled-components';
import type { ThemePalette } from './themes';
import { FONTS } from './themes';

/** hex #RRGGBB → rgba(r,g,b,a)。 */
function hexToRgba(hex: string, alpha: number): string {
  const h = hex.replace('#', '');
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** 把调色板展开为 CSS variables（--bvt-* 前缀）。 */
export function paletteToVars(pal: ThemePalette): Record<string, string> {
  return {
    '--bvt-bg': pal.bg,
    '--bvt-surface': pal.surface,
    '--bvt-surface2': pal.surface2,
    '--bvt-text': pal.text,
    '--bvt-text2': pal.text2,
    '--bvt-text3': pal.text3,
    '--bvt-border': pal.border,
    '--bvt-border2': pal.border2,
    '--bvt-hover': pal.hover,
    '--bvt-input-bg': pal.inputBg,
    // 半透明变体：面板/输入框毛玻璃背景，透出柔化后的母题脉络。
    '--bvt-surface2-70': hexToRgba(pal.surface2, 0.7),
    '--bvt-input-bg-70': hexToRgba(pal.inputBg, 0.7),
    '--bvt-accent': pal.accent,
    '--bvt-accent-deep': pal.accentDeep,
    '--bvt-accent-light': pal.accentLight,
    '--bvt-btn-fill': pal.btnFill,
    '--bvt-btn-fill-hover': pal.btnFillHover,
    '--bvt-btn-fill-press': pal.btnFillPress,
    '--bvt-success': pal.success,
    '--bvt-success-l': pal.successL,
    '--bvt-warn': pal.warn,
    '--bvt-warn-l': pal.warnL,
    '--bvt-danger': pal.danger,
    '--bvt-danger-l': pal.dangerL,
    '--bvt-sel-bg': pal.selBg,
    '--bvt-sel-text': pal.selText,
  };
}

export const GlobalStyle = createGlobalStyle`
  /* 兜底默认色板（zhuyin light）：App.tsx 运行时注入 :root 会覆盖同名变量；此处字面声明供静态分析解析 var() */
  :root {
    --bvt-bg: #FAF6EE;
    --bvt-surface: #FFFDF8;
    --bvt-surface2: #F3ECDD;
    --bvt-text: #2A2622;
    --bvt-text2: #6B6256;
    --bvt-text3: #9A9183;
    --bvt-border: #D9CFBE;
    --bvt-border2: #E8E0D2;
    --bvt-hover: #EFE7D8;
    --bvt-input-bg: #FCFAF4;
    --bvt-surface2-70: rgba(243, 236, 221, 0.7);
    --bvt-input-bg-70: rgba(252, 250, 244, 0.7);
    --bvt-accent: #B83A2E;
    --bvt-accent-deep: #8F2C22;
    --bvt-accent-light: #F5DCD6;
    --bvt-btn-fill: #B83A2E;
    --bvt-btn-fill-hover: #8F2C22;
    --bvt-btn-fill-press: #8F2C22;
    --bvt-success: #2A5F4F;
    --bvt-success-l: #D6E3D9;
    --bvt-warn: #8C6A2A;
    --bvt-warn-l: #ECDFB6;
    --bvt-danger: #9E2B20;
    --bvt-danger-l: #F5DCD6;
    --bvt-sel-bg: #F5DCD6;
    --bvt-sel-text: #8F2C22;
    --bvt-anim: 1;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { height: 100%; }
  body {
    font-family: ${FONTS.sans};
    font-size: 13px;
    background: var(--bvt-bg);
    color: var(--bvt-text);
    -webkit-font-smoothing: antialiased;
    user-select: none;
    overflow: hidden;
  }
  /* 全局颜色过渡：主题/明暗切换时背景/文字/边框/按钮平滑渐变（时长随动画倍速缩放） */
  body, button, input, textarea, div, span, label, li, h1, h2, h3, select {
    transition: background-color calc(0.35s / var(--bvt-anim)) ease, color calc(0.35s / var(--bvt-anim)) ease, border-color calc(0.35s / var(--bvt-anim)) ease;
  }
  /* 滚动条 */
  ::-webkit-scrollbar { width: 10px; height: 10px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: var(--bvt-border); border-radius: 5px; }
  ::-webkit-scrollbar-thumb:hover { background: var(--bvt-text3); }
  input, textarea, select { font-family: ${FONTS.sans}; }
`;
