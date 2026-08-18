/**
 * 全局样式：CSS variables 注入 + 基础 reset。
 * styled-components 的 createGlobalStyle，消费主题 store 提供的色板。
 */

import { createGlobalStyle } from 'styled-components';
import type { ThemeMode, ThemePalette } from './themes';
import { FONTS } from './themes';

function hexToRgba(hex: string, alpha: number): string {
  const h = hex.replace('#', '');
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function paletteToVars(pal: ThemePalette, mode: ThemeMode = 'light'): Record<string, string> {
  const glassA = mode === 'dark' ? 0.42 : 0.58;
  const glass2A = mode === 'dark' ? 0.34 : 0.46;
  const inputA = mode === 'dark' ? 0.4 : 0.52;
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
    '--bvt-surface2-70': hexToRgba(pal.surface2, 0.72),
    '--bvt-input-bg-70': hexToRgba(pal.inputBg, 0.78),
    '--bvt-accent': pal.accent,
    '--bvt-accent-deep': pal.accentDeep,
    '--bvt-accent-light': pal.accentLight,
    '--bvt-btn-fill': pal.btnFill,
    '--bvt-btn-fill-hover': pal.btnFillHover,
    '--bvt-btn-fill-press': pal.btnFillPress,
    '--bvt-on-accent': pal.onAccent,
    '--bvt-on-btn': pal.onBtn,
    '--bvt-success': pal.success,
    '--bvt-success-l': pal.successL,
    '--bvt-warn': pal.warn,
    '--bvt-warn-l': pal.warnL,
    '--bvt-danger': pal.danger,
    '--bvt-danger-l': pal.dangerL,
    '--bvt-sel-bg': pal.selBg,
    '--bvt-sel-text': pal.selText,
    '--bvt-glass': hexToRgba(pal.surface, glassA),
    '--bvt-glass-2': hexToRgba(pal.surface2, glass2A),
    '--bvt-glass-input': hexToRgba(pal.inputBg, inputA),
    '--bvt-glass-border': hexToRgba(pal.border, 0.55),
    '--bvt-glass-highlight': `inset 0 1px 0 ${hexToRgba(pal.accentLight, 0.42)}`,
  };
}

export const GlobalStyle = createGlobalStyle`
  :root {
    --bvt-bg: #F3E6CF;
    --bvt-surface: #FBF3E4;
    --bvt-surface2: #E6D3B4;
    --bvt-text: #1A1410;
    --bvt-text2: #5A4E42;
    --bvt-text3: #8A7B6A;
    --bvt-border: #C4A97A;
    --bvt-border2: #DCC9A4;
    --bvt-hover: #EBD9B8;
    --bvt-input-bg: #FDF6E8;
    --bvt-surface2-70: rgba(230, 211, 180, 0.72);
    --bvt-input-bg-70: rgba(253, 246, 232, 0.78);
    --bvt-accent: #C41A14;
    --bvt-accent-deep: #8A100C;
    --bvt-accent-light: #F4C4BC;
    --bvt-btn-fill: #C41A14;
    --bvt-btn-fill-hover: #8A100C;
    --bvt-btn-fill-press: #6E0C0A;
    --bvt-on-accent: #FFF8F2;
    --bvt-on-btn: #FFF8F2;
    --bvt-success: #2A5644;
    --bvt-success-l: #D4E4D8;
    --bvt-warn: #9A6A18;
    --bvt-warn-l: #F0E0B4;
    --bvt-danger: #A11410;
    --bvt-danger-l: #F4C4BC;
    --bvt-sel-bg: #F0C8C0;
    --bvt-sel-text: #8A100C;
    --bvt-glass: rgba(251, 243, 228, 0.58);
    --bvt-glass-2: rgba(230, 211, 180, 0.46);
    --bvt-glass-input: rgba(253, 246, 232, 0.52);
    --bvt-glass-border: rgba(196, 169, 122, 0.55);
    --bvt-glass-highlight: inset 0 1px 0 rgba(244, 196, 188, 0.42);
    --bvt-glass-blur: 14px;
    --bvt-glass-sat: 1.28;
    --bvt-anim: 1;
    --bvt-radius: 0px;
    --bvt-title-track: 0.22em;
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
    isolation: isolate;
  }
  /* macOS：窗体背后是 NSGlassEffectView / NSVisualEffectView，网页只染色 */
  html[data-platform='mac'] {
    --bvt-glass-blur: 0px;
    --bvt-glass-sat: 1.08;
    --bvt-glass: color-mix(in srgb, var(--bvt-surface) 28%, transparent);
    --bvt-glass-2: color-mix(in srgb, var(--bvt-surface2) 20%, transparent);
    --bvt-glass-input: color-mix(in srgb, var(--bvt-input-bg) 26%, transparent);
  }
  html[data-platform='mac'],
  html[data-platform='mac'] body,
  html[data-platform='mac'] #root {
    background: transparent;
  }
  html[data-platform='mac'] body {
    isolation: auto;
  }
  #root {
    position: relative;
    z-index: 1;
    height: 100%;
  }
  body, button, input, textarea, div, span, label, li, h1, h2, h3, select {
    transition: background-color calc(0.35s / var(--bvt-anim)) ease, color calc(0.35s / var(--bvt-anim)) ease, border-color calc(0.35s / var(--bvt-anim)) ease;
  }
  ::-webkit-scrollbar { width: 10px; height: 10px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: var(--bvt-border); border-radius: 5px; }
  ::-webkit-scrollbar-thumb:hover { background: var(--bvt-text3); }
  input, textarea, select { font-family: ${FONTS.sans}; }

  html[data-theme='zhuyin'] {
    --bvt-radius: 0px;
    --bvt-title-track: 0.22em;
  }
  html[data-theme='liujin'] {
    --bvt-radius: 8px;
    --bvt-title-track: 0.14em;
  }
  html[data-theme='guwen'] {
    --bvt-radius: 4px;
    --bvt-title-track: 0.08em;
  }
  /* 主题 blur 不得盖掉 mac 的 --bvt-glass-blur: 0（同特异性、后写会赢） */
  html[data-theme='zhuyin']:not([data-platform='mac']) {
    --bvt-glass-blur: 12px;
    --bvt-glass-sat: 1.18;
  }
  html[data-theme='liujin']:not([data-platform='mac']) {
    --bvt-glass-blur: 18px;
    --bvt-glass-sat: 1.42;
  }
  html[data-theme='guwen']:not([data-platform='mac']) {
    --bvt-glass-blur: 16px;
    --bvt-glass-sat: 1.3;
  }
  html[data-mode='dark']:not([data-platform='mac']) {
    --bvt-glass-blur: 18px;
  }

  /* 底纹只垫在内容后面，禁止 mix-blend-mode，避免洗白整窗 */
  html[data-theme] body::before {
    content: '';
    pointer-events: none;
    position: fixed;
    inset: 0;
    z-index: -1;
  }
  html[data-platform='mac'][data-theme] body::before {
    opacity: 0.12;
  }
  html[data-theme='zhuyin'] body::before {
    opacity: 0.28;
    background-image:
      repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(80, 42, 18, 0.04) 3px),
      repeating-linear-gradient(90deg, transparent, transparent 3px, rgba(80, 42, 18, 0.025) 4px);
  }
  html[data-theme='liujin'] body::before {
    opacity: 0.34;
    background-image:
      radial-gradient(circle at 18% 22%, rgba(201, 160, 24, 0.16) 0 1px, transparent 2px),
      radial-gradient(circle at 72% 64%, rgba(201, 160, 24, 0.12) 0 1px, transparent 2px),
      repeating-linear-gradient(118deg, transparent, transparent 11px, rgba(201, 160, 24, 0.045) 12px);
    background-size: 120px 120px, 160px 160px, 100% 100%;
  }
  html[data-theme='guwen'] body::before {
    opacity: 0.22;
    background-image:
      repeating-linear-gradient(45deg, transparent, transparent 13px, rgba(46, 92, 66, 0.06) 14px),
      repeating-linear-gradient(-45deg, transparent, transparent 21px, rgba(46, 92, 66, 0.04) 22px);
  }
`;
