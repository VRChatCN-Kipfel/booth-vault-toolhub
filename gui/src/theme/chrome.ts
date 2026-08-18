/**
 * 三主题形制：朱印方折 / 鎏金金缮 / 古纹云雷。
 * 色板走 CSS variables，这里只决定「长什么样」。
 */

import type { ThemeName } from './themes';
import { FONTS } from './themes';

export function themeRadius(t: ThemeName): string {
  if (t === 'liujin') return '8px';
  if (t === 'guwen') return '4px';
  return '0px';
}

export function themeTitleFont(t: ThemeName): string {
  if (t === 'guwen') {
    return FONTS.serif;
  }
  return FONTS.serif;
}

export function themeTitleTrack(t: ThemeName): string {
  if (t === 'zhuyin') return '0.22em';
  if (t === 'liujin') return '0.14em';
  return '0.08em';
}

export function brandMark(kind: ThemeName, color: string): string {
  if (kind === 'zhuyin') {
    return `<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
      <rect x="1.5" y="1.5" width="37" height="37" fill="none" stroke="${color}" stroke-width="2.4"/>
      <rect x="6" y="6" width="28" height="28" fill="none" stroke="${color}" stroke-width="1.2"/>
      <path d="M12 11 H28 V29 H12 Z" fill="none" stroke="${color}" stroke-width="1.1"/>
      <rect x="16" y="15" width="8" height="10" fill="${color}"/>
      <path d="M12 11 H16 M24 11 H28 M12 29 H16 M24 29 H28" stroke="${color}" stroke-width="1.1"/>
    </svg>`;
  }
  if (kind === 'liujin') {
    return `<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
      <circle cx="20" cy="20" r="16.5" fill="none" stroke="${color}" stroke-width="1.3"/>
      <path d="M20 4.5 C11 14,11 26,20 35.5 C29 26,29 14,20 4.5 Z" fill="none" stroke="${color}" stroke-width="1.15"/>
      <circle cx="20" cy="20" r="3.4" fill="${color}"/>
      <path d="M7 20 H33 M20 7 V33" stroke="${color}" stroke-width="0.55" opacity="0.5"/>
      <path d="M20 4.5 C24 12,26 16,33 20" fill="none" stroke="${color}" stroke-width="0.7" opacity="0.55"/>
    </svg>`;
  }
  return `<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
    <path d="M20 3 C11 12,9 22,20 37 C31 22,29 12,20 3 Z" fill="none" stroke="${color}" stroke-width="1.5"/>
    <path d="M20 9 C15.5 16,15.5 24,20 31 C24.5 24,24.5 16,20 9 Z" fill="${color}" fill-opacity="0.32" stroke="${color}" stroke-width="0.8"/>
    <path d="M20 7 V33" stroke="${color}" stroke-width="0.9"/>
    <path d="M20 18 a5 5 0 1 1 -5 -5 a2.5 2.5 0 1 0 2.5 2.5" fill="none" stroke="${color}" stroke-width="0.9" opacity="0.7"/>
  </svg>`;
}

export function titleOrnament(kind: ThemeName, color: string): string {
  if (kind === 'zhuyin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 176 36" width="176" height="36">
      <path d="M3 30 H56 V6 H10 V26 H48 V10 H18 V22 H40" fill="none" stroke="${color}" stroke-width="1.25" opacity="0.78"/>
      <rect x="66" y="9" width="18" height="18" fill="none" stroke="${color}" stroke-width="1.5"/>
      <rect x="70" y="13" width="10" height="10" fill="${color}"/>
      <path d="M94 28 H172 M94 22 H148" stroke="${color}" stroke-width="1" opacity="0.45"/>
    </svg>`;
  }
  if (kind === 'liujin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 176 36" width="176" height="36">
      <path d="M2 30 C36 28,62 8,104 11 C136 13,154 22,174 8" fill="none" stroke="${color}" stroke-width="1.25" opacity="0.75"/>
      <path d="M34 28 A11 11 0 0 1 56 28" fill="none" stroke="${color}" stroke-width="0.85" opacity="0.5"/>
      <path d="M44 28 A6 6 0 0 1 56 28" fill="none" stroke="${color}" stroke-width="0.7" opacity="0.38"/>
      <circle cx="104" cy="11" r="2.1" fill="${color}"/>
      <circle cx="68" cy="13" r="1.3" fill="${color}" opacity="0.7"/>
      <path d="M104 11 C118 8,132 16,146 12" fill="none" stroke="${color}" stroke-width="0.7" opacity="0.45"/>
    </svg>`;
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 176 36" width="176" height="36">
    <path d="M4 30 C28 8,54 8,78 28 C102 8,128 10,172 26" fill="none" stroke="${color}" stroke-width="1.2" opacity="0.68"/>
    <path d="M78 28 q15 -11 22 4 q-7 11 -22 -4 Z" fill="${color}" fill-opacity="0.3"/>
    <path d="M26 16 a7 7 0 1 1 -7 -7 a3.5 3.5 0 1 0 3.5 3.5" fill="none" stroke="${color}" stroke-width="1" opacity="0.55"/>
    <path d="M132 14 a5 5 0 1 1 -5 -5 a2.4 2.4 0 1 0 2.4 2.4" fill="none" stroke="${color}" stroke-width="0.85" opacity="0.4"/>
  </svg>`;
}

export function contentFrame(t: ThemeName): string {
  if (t === 'zhuyin') {
    return `var(--bvt-glass-highlight), inset 0 0 0 1px var(--bvt-glass-border), inset 0 0 0 2px var(--bvt-accent)`;
  }
  if (t === 'liujin') {
    return `var(--bvt-glass-highlight), 0 0 0 1px color-mix(in srgb, var(--bvt-accent) 20%, transparent)`;
  }
  return `var(--bvt-glass-highlight), inset 0 0 0 1px color-mix(in srgb, var(--bvt-accent) 18%, transparent)`;
}
