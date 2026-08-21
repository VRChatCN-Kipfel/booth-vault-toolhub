/**
 * 三主题印记：朱印方折 / 鎏金圆转 / 古纹叶脉。
 * 只出 SVG 与圆角，颜色一律由调用方从色板取。
 */

import type { ThemeName } from './themes';

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

