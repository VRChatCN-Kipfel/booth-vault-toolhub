/**
 * 三主题形制：朱印方折 / 鎏金金缮 / 古纹云雷。
 * 色板仍走 CSS variables，这里只决定「长什么样」。
 */

import type { ThemeName } from './themes';

export function themeRadius(t: ThemeName): string {
  if (t === 'liujin') return '4px';
  if (t === 'guwen') return '3px';
  return '1px';
}

export function themeTitleFont(t: ThemeName): string {
  if (t === 'zhuyin') {
    return `'Noto Serif CJK SC','Source Han Serif SC','Songti SC','STSong',serif`;
  }
  if (t === 'liujin') {
    return `'Noto Serif CJK SC','Source Han Serif SC','Songti SC',serif`;
  }
  return `'Noto Serif CJK SC','Source Han Serif SC','Songti SC',serif`;
}

/** 侧栏印章 / 金结 / 叶符。 */
export function brandMark(kind: ThemeName, color: string): string {
  if (kind === 'zhuyin') {
    return `<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
      <rect x="2.5" y="2.5" width="35" height="35" fill="none" stroke="${color}" stroke-width="2.2"/>
      <rect x="7.5" y="7.5" width="25" height="25" fill="none" stroke="${color}" stroke-width="1.3"/>
      <path d="M14 13 H26 V27 H14 Z" fill="none" stroke="${color}" stroke-width="1.1"/>
      <rect x="17" y="16" width="6" height="8" fill="${color}"/>
    </svg>`;
  }
  if (kind === 'liujin') {
    return `<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
      <circle cx="20" cy="20" r="16" fill="none" stroke="${color}" stroke-width="1.4"/>
      <path d="M20 5 C12 14,12 26,20 35 C28 26,28 14,20 5 Z" fill="none" stroke="${color}" stroke-width="1.1"/>
      <circle cx="20" cy="20" r="3.2" fill="${color}"/>
      <path d="M8 20 H32 M20 8 V32" stroke="${color}" stroke-width="0.6" opacity="0.55"/>
    </svg>`;
  }
  return `<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
    <path d="M20 4 C12 12,10 22,20 36 C30 22,28 12,20 4 Z" fill="none" stroke="${color}" stroke-width="1.5"/>
    <path d="M20 10 C16 16,16 24,20 30 C24 24,24 16,20 10 Z" fill="${color}" fill-opacity="0.35" stroke="${color}" stroke-width="0.8"/>
    <path d="M20 8 V32" stroke="${color}" stroke-width="0.9"/>
  </svg>`;
}

/** 页标题旁的母题花饰。 */
export function titleOrnament(kind: ThemeName, color: string): string {
  if (kind === 'zhuyin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 168 36" width="168" height="36">
      <path d="M4 28 H52 V8 H12 V24 H44 V12 H20" fill="none" stroke="${color}" stroke-width="1.3" opacity="0.75"/>
      <rect x="62" y="10" width="16" height="16" fill="none" stroke="${color}" stroke-width="1.4"/>
      <rect x="66" y="14" width="8" height="8" fill="${color}"/>
      <path d="M88 28 C118 26,140 16,164 6" fill="none" stroke="${color}" stroke-width="1.1" opacity="0.5"/>
    </svg>`;
  }
  if (kind === 'liujin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 168 36" width="168" height="36">
      <path d="M2 30 C40 28,70 8,110 10 C140 12,154 22,166 8" fill="none" stroke="${color}" stroke-width="1.25" opacity="0.7"/>
      <path d="M36 28 A10 10 0 0 1 56 28" fill="none" stroke="${color}" stroke-width="0.8" opacity="0.45"/>
      <path d="M46 28 A6 6 0 0 1 58 28" fill="none" stroke="${color}" stroke-width="0.7" opacity="0.35"/>
      <circle cx="110" cy="10" r="2" fill="${color}"/>
      <circle cx="70" cy="12" r="1.4" fill="${color}" opacity="0.7"/>
    </svg>`;
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 168 36" width="168" height="36">
    <path d="M6 30 C28 8,52 8,74 28 C96 8,120 10,160 26" fill="none" stroke="${color}" stroke-width="1.2" opacity="0.65"/>
    <path d="M74 28 q14 -10 20 4 q-6 10 -20 -4 Z" fill="${color}" fill-opacity="0.28"/>
    <path d="M28 16 a6 6 0 1 1 -6 -6 a3 3 0 1 0 3 3" fill="none" stroke="${color}" stroke-width="1" opacity="0.5"/>
  </svg>`;
}
