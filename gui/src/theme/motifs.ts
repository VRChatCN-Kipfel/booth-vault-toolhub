import type { MotifKind } from './themes';

export function dropTile(kind: MotifKind, color: string): string {
  if (kind === 'zhuyin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 28" width="28" height="28">
      <path d="M4 4 H24 V24 H4 Z" fill="none" stroke="${color}" stroke-width="1.2" stroke-opacity="0.7"/>
      <path d="M8 8 H20 V20 H8 Z" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.55"/>
      <rect x="12" y="12" width="4" height="4" fill="${color}" fill-opacity="0.55"/>
    </svg>`;
  }
  if (kind === 'liujin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 28" width="28" height="28">
      <path d="M2 22 A12 12 0 0 1 26 22" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="0.55"/>
      <path d="M6 22 A8 8 0 0 1 22 22" fill="none" stroke="${color}" stroke-width="0.9" stroke-opacity="0.4"/>
      <path d="M10 22 A4 4 0 0 1 18 22" fill="none" stroke="${color}" stroke-width="0.8" stroke-opacity="0.35"/>
    </svg>`;
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 28" width="28" height="28">
    <path d="M16 8 a6 6 0 1 1 -6 -6 a3 3 0 1 0 3 3" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="0.55"/>
    <path d="M8 22 C12 14,16 12,24 10" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.4"/>
  </svg>`;
}
