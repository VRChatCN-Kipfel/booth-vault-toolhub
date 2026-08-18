/**
 * 全局样式：设计令牌 + reset。
 *
 * 令牌只有一套刻度，主题之间只换「纸墨色」和少量形制（圆角、朱记宽度）。
 * 组件一律消费 var(--bvt-*)，不得再写死颜色或间距。
 */

import { createGlobalStyle } from 'styled-components';
import type { ThemeMode, ThemePalette } from './themes';
import { FONTS } from './themes';

export function paletteToVars(pal: ThemePalette, mode: ThemeMode = 'light'): Record<string, string> {
  const dark = mode === 'dark';
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
    '--bvt-shadow-1': dark ? '0 1px 2px rgba(0,0,0,0.45)' : '0 1px 2px rgba(28,26,23,0.06)',
    '--bvt-shadow-2': dark
      ? '0 12px 32px rgba(0,0,0,0.6), 0 2px 8px rgba(0,0,0,0.4)'
      : '0 12px 32px rgba(28,26,23,0.12), 0 2px 8px rgba(28,26,23,0.06)',
  };
}

export const GlobalStyle = createGlobalStyle`
  :root {
    /* 间距刻度：8 的倍数为主，4 用于行内 */
    --bvt-s1: 4px;
    --bvt-s2: 8px;
    --bvt-s3: 12px;
    --bvt-s4: 16px;
    --bvt-s5: 24px;
    --bvt-s6: 32px;
    --bvt-s7: 48px;

    /* 字号刻度 */
    --bvt-fz-xs: 11px;
    --bvt-fz-sm: 12px;
    --bvt-fz-md: 13px;
    --bvt-fz-lg: 15px;
    --bvt-fz-title: 20px;

    /* 控件高度 */
    --bvt-h-ctl: 32px;
    --bvt-h-row: 34px;

    --bvt-serif: ${FONTS.serif};
    --bvt-mono: ${FONTS.mono};

    /* 形制：圆角三主题统一，只有字距与朱记宽度随主题变 */
    --bvt-radius: 6px;
    --bvt-radius-sm: 4px;
    --bvt-pill: 999px;
    --bvt-title-track: 0.1em;
    /* 朱记：页题与当前项左侧的那道竖线 */
    --bvt-mark-w: 3px;

    --bvt-anim: 1;
    --bvt-ease: cubic-bezier(0.32, 0.72, 0, 1);
    /* 背景花纹浓淡，设置页滑条覆写 */
    --bvt-motif-opacity: 0.08;

    /* 壳体底色：Windows/Linux 用实色，macOS 换成半透明让原生材质透上来 */
    --bvt-shell-bg: var(--bvt-surface);
    --bvt-rail-bg: var(--bvt-surface2);

    /* paletteToVars 未注入前的占位（首帧防闪白） */
    --bvt-bg: #F5F2EC;
    --bvt-surface: #FFFFFF;
    --bvt-surface2: #EAE5DC;
    --bvt-text: #1C1A17;
    --bvt-text2: #5A554E;
    --bvt-text3: #918B81;
    --bvt-border: #DDD7CC;
    --bvt-border2: #ECE7DE;
    --bvt-hover: #F0ECE3;
    --bvt-input-bg: #FFFFFF;
    --bvt-accent: #A8322A;
    --bvt-shadow-1: 0 1px 2px rgba(28, 26, 23, 0.06);
    --bvt-shadow-2: 0 12px 32px rgba(28, 26, 23, 0.12);
  }

  html[data-theme='liujin'] {
    --bvt-title-track: 0.06em;
    --bvt-mark-w: 2px;
  }
  html[data-theme='guwen'] {
    --bvt-title-track: 0.04em;
    --bvt-mark-w: 2px;
  }

  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body, #root { height: 100%; }

  body {
    font-family: ${FONTS.sans};
    font-size: var(--bvt-fz-md);
    line-height: 1.65;
    background: var(--bvt-bg);
    color: var(--bvt-text);
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
    user-select: none;
    overflow: hidden;
  }

  button, input, textarea, select { font-family: inherit; }

  /* 纸纹：极淡的横向帘纹，只垫在最底层 */
  body::before {
    content: '';
    position: fixed;
    inset: 0;
    z-index: -1;
    pointer-events: none;
    background-image: repeating-linear-gradient(
      0deg,
      transparent 0 3px,
      color-mix(in srgb, var(--bvt-text) 2%, transparent) 3px 4px
    );
  }
  html[data-platform='mac'] body::before { content: none; }

  ::selection {
    background: var(--bvt-sel-bg);
    color: var(--bvt-sel-text);
  }

  ::-webkit-scrollbar { width: 8px; height: 8px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--bvt-text3) 35%, transparent);
    border-radius: 999px;
  }
  ::-webkit-scrollbar-thumb:hover { background: color-mix(in srgb, var(--bvt-text3) 70%, transparent); }

  :focus-visible {
    outline: 2px solid var(--bvt-accent);
    outline-offset: 2px;
    border-radius: var(--bvt-radius-sm);
  }

  @media (prefers-reduced-motion: reduce) {
    :root { --bvt-anim: 80; }
    *, *::before, *::after {
      animation-duration: 0.01ms !important;
      transition-duration: 0.01ms !important;
    }
  }

  /* ── 平台适配 ───────────────────────────────────────────────
     mac：窗体背后是原生材质，网页只负责染色，禁止自绘 blur（blur(0) 也会
     建 WebKit backdrop 根，导致图层错位）。win/linux：自绘毛玻璃。 */
  html[data-platform='mac'] {
    --bvt-glass-blur: none;
    --bvt-shell-bg: color-mix(in srgb, var(--bvt-surface) 82%, transparent);
    --bvt-rail-bg: color-mix(in srgb, var(--bvt-surface2) 62%, transparent);
  }
  html[data-platform='mac'],
  html[data-platform='mac'] body,
  html[data-platform='mac'] #root {
    background: transparent;
  }
`;
