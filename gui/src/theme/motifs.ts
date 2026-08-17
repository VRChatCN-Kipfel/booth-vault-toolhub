/**
 * SVG 母题：三主题各自成套，不再共用同一组藤蔓路径换色。
 */

import type { MotifKind } from './themes';

export function seigaihaLayer(color: string): string {
  const a = 0.16;
  const r = 22;
  const paths: string[] = [];
  let row = 0;
  let y = 0;
  while (y < 720 + r) {
    const off = (row % 2) * r;
    let x = off - r;
    while (x < 72 + r) {
      const layers: Array<[number, number, number]> = [
        [r, a, 0.9],
        [(r * 2) / 3, a * 0.7, 0.6],
        [r / 3, a * 0.45, 0.6],
      ];
      for (const [rr, op, w] of layers) {
        const f1 = (x - rr).toFixed(1);
        const f2 = (x + rr).toFixed(1);
        const rrs = rr.toFixed(1);
        paths.push(
          `<path d="M${f1} ${y} A${rrs} ${rrs} 0 0 1 ${f2} ${y}" fill="none" stroke="${color}" stroke-width="${w}" stroke-opacity="${op.toFixed(3)}"/>`,
        );
      }
      x += 2 * r;
    }
    y += r;
    row += 1;
  }
  return paths.join('');
}

function goldSidebar(color: string): string {
  const parts: string[] = [];
  parts.push(
    `<path d="M28 0 C10 90,50 170,26 260 C8 350,52 440,28 530 C12 610,48 670,30 720" fill="none" stroke="${color}" stroke-width="1.7" stroke-opacity="0.74"/>`,
    `<path d="M48 0 C62 100,28 180,50 280 C66 370,32 460,52 570 C64 650,42 700,54 720" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="0.48"/>`,
  );
  const thin = [
    'M28 110 C40 150,50 170,64 192',
    'M28 248 C16 282,10 304,2 328',
    'M28 404 C42 436,54 460,66 486',
    'M28 528 C16 558,8 578,0 598',
    'M50 276 C58 310,64 342,70 368',
  ];
  for (const d of thin) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="0.5" stroke-opacity="0.26"/>`);
  }
  for (const [cx, cy, rad] of [
    [28, 88, 2.5],
    [28, 248, 2.5],
    [28, 404, 2.5],
    [28, 568, 2.5],
    [50, 276, 1.8],
    [52, 570, 1.8],
  ] as Array<[number, number, number]>) {
    parts.push(`<circle cx="${cx}" cy="${cy}" r="${rad}" fill="${color}" fill-opacity="0.86"/>`);
  }
  parts.push(seigaihaLayer(color));
  return parts.join('');
}

function zhuyinSidebar(color: string): string {
  const parts: string[] = [];
  for (let y = 16; y < 700; y += 60) {
    parts.push(
      `<path d="M12 ${y} H60 V${y + 42} H20 V${y + 8} H52 V${y + 34} H28 V${y + 16} H44" fill="none" stroke="${color}" stroke-width="1.35" stroke-opacity="0.78"/>`,
    );
  }
  parts.push(
    `<rect x="20" y="318" width="32" height="32" fill="none" stroke="${color}" stroke-width="1.8" stroke-opacity="0.82"/>`,
    `<rect x="25" y="323" width="22" height="22" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.45"/>`,
    `<rect x="29" y="331" width="14" height="14" fill="${color}" fill-opacity="0.88"/>`,
  );
  return parts.join('');
}

function guwenSidebar(color: string): string {
  const parts: string[] = [];
  for (const [sx, sy] of [
    [18, 96],
    [50, 188],
    [16, 292],
    [52, 396],
    [20, 504],
    [48, 608],
    [22, 696],
  ]) {
    parts.push(
      `<path d="M${sx} ${sy} a9 9 0 1 1 -9 -9 a4.5 4.5 0 1 0 4.5 4.5" fill="none" stroke="${color}" stroke-width="1.15" stroke-opacity="0.62"/>`,
    );
  }
  parts.push(
    `<path d="M10 720 C16 540,34 400,34 260 C34 140,50 70,64 0" fill="none" stroke="${color}" stroke-width="1.7" stroke-opacity="0.58"/>`,
  );
  const branch = [
    'M34 300 C18 322,8 346,4 372',
    'M34 210 C50 232,60 254,66 280',
    'M34 130 C18 152,8 176,4 202',
    'M34 50 C50 72,60 98,66 128',
    'M42 390 C28 414,20 438,18 468',
    'M20 520 C36 544,48 568,56 596',
  ];
  for (const d of branch) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.42"/>`);
  }
  parts.push(
    `<path d="M64 168 C72 158,76 144,74 130 C64 134,56 144,54 156 Z" fill="${color}" fill-opacity="0.24"/>`,
    `<path d="M34 300 C22 300,12 308,8 320 C18 328,28 328,34 320 Z" fill="${color}" fill-opacity="0.24"/>`,
    `<path d="M42 390 C52 386,60 394,62 406 C52 414,42 410,38 400 Z" fill="${color}" fill-opacity="0.24"/>`,
  );
  return parts.join('');
}

export function sidebarMotif(kind: MotifKind, color: string): string {
  let body = '';
  switch (kind) {
    case 'gold':
      body = goldSidebar(color);
      break;
    case 'zhuyin':
      body = zhuyinSidebar(color);
      break;
    case 'guwen':
      body = guwenSidebar(color);
      break;
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="72" height="720" viewBox="0 0 72 720">` +
    body +
    `</svg>`
  );
}

export function motifBg(kind: MotifKind, color: string, light: boolean): string {
  switch (kind) {
    case 'gold':
      return goldBg(color, light);
    case 'zhuyin':
      return cinnabarBg(color, light);
    case 'guwen':
      return leafBg(color, light);
  }
}

function goldBg(color: string, light: boolean): string {
  const s = light ? 1 : 0.78;
  const parts: string[] = [];
  const r = 46;
  let row = 0;
  for (let y = 40; y < 800; y += r) {
    const off = (row % 2) * r;
    for (let x = off - r; x < 1240; x += r * 2) {
      parts.push(
        `<path d="M${x} ${y} A${r} ${r} 0 0 1 ${x + r * 2} ${y}" fill="none" stroke="${color}" stroke-width="0.8" stroke-opacity="${(0.11 * s).toFixed(3)}"/>`,
        `<path d="M${x + r * 0.35} ${y} A${r * 0.65} ${r * 0.65} 0 0 1 ${x + r * 1.65} ${y}" fill="none" stroke="${color}" stroke-width="0.55" stroke-opacity="${(0.07 * s).toFixed(3)}"/>`,
      );
    }
    row += 1;
  }
  const cracks = [
    'M40 80 C180 160,120 280,280 360 C460 460,340 580,560 680 C700 760,820 720,980 790',
    'M1180 20 C980 140,1080 280,860 380 C680 490,760 620,520 720',
    'M-10 420 C140 460,260 500,400 540 C560 590,700 680,840 760',
  ];
  for (const d of cracks) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1.25" stroke-opacity="${(0.2 * s).toFixed(3)}"/>`);
  }
  for (const [cx, cy] of [
    [280, 360],
    [560, 680],
    [860, 380],
    [400, 540],
    [520, 720],
  ]) {
    parts.push(`<circle cx="${cx}" cy="${cy}" r="2.2" fill="${color}" fill-opacity="${(0.28 * s).toFixed(3)}"/>`);
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="780" viewBox="0 0 1200 780">` +
    parts.join('') +
    `</svg>`
  );
}

function cinnabarBg(color: string, light: boolean): string {
  const a = light ? 0.2 : 0.15;
  const parts: string[] = [];
  const corners: Array<[number, number, number, number]> = [
    [16, 16, 1, 1],
    [1184, 16, -1, 1],
    [16, 764, 1, -1],
    [1184, 764, -1, -1],
  ];
  for (const [x, y, sx, sy] of corners) {
    parts.push(
      `<path d="M${x} ${y + 70 * sy} V${y} H${x + 70 * sx}" fill="none" stroke="${color}" stroke-width="1.6" stroke-opacity="${a}"/>`,
      `<path d="M${x + 10 * sx} ${y + 54 * sy} V${y + 10 * sy} H${x + 54 * sx}" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="${a * 0.7}"/>`,
    );
  }
  const seals: Array<[number, number, number]> = [
    [180, 120, 28],
    [920, 90, 22],
    [1080, 420, 34],
    [240, 560, 24],
    [640, 200, 18],
    [780, 640, 26],
    [420, 380, 20],
  ];
  for (const [x, y, s] of seals) {
    parts.push(
      `<rect x="${x}" y="${y}" width="${s}" height="${s}" fill="none" stroke="${color}" stroke-width="1.4" stroke-opacity="${a}"/>`,
      `<rect x="${x + 4}" y="${y + 4}" width="${s - 8}" height="${s - 8}" fill="${color}" fill-opacity="${a * 0.35}"/>`,
    );
  }
  parts.push(
    `<rect x="1040" y="560" width="88" height="88" fill="none" stroke="${color}" stroke-width="2" stroke-opacity="${a * 0.85}"/>`,
    `<rect x="1054" y="574" width="60" height="60" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="${a * 0.5}"/>`,
    `<rect x="1068" y="596" width="32" height="28" fill="${color}" fill-opacity="${a * 0.55}"/>`,
  );
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">` +
    parts.join('') +
    `</svg>`
  );
}

function leafBg(color: string, light: boolean): string {
  const a = light ? 0.22 : 0.16;
  const parts: string[] = [];
  for (let y = 40; y < 780; y += 120) {
    for (let x = 40; x < 1180; x += 160) {
      const ox = ((y / 120) % 2) * 80;
      parts.push(
        `<path d="M${x + ox} ${y} a10 10 0 1 1 -10 -10 a5 5 0 1 0 5 5" fill="none" stroke="${color}" stroke-width="1.05" stroke-opacity="${a}"/>`,
      );
    }
  }
  parts.push(
    `<path d="M80 820 C160 520,260 360,360 200 C430 90,520 40,620 -10" fill="none" stroke="${color}" stroke-width="1.6" stroke-opacity="${a * 1.15}"/>`,
    `<path d="M1180 40 C980 180,1040 340,820 460 C660 560,720 680,540 790" fill="none" stroke="${color}" stroke-width="1.4" stroke-opacity="${a}"/>`,
  );
  const leaves: Array<[number, number]> = [
    [340, 220],
    [520, 80],
    [800, 470],
    [560, 760],
    [200, 540],
    [980, 300],
  ];
  for (const [lx, ly] of leaves) {
    parts.push(
      `<path d="M${lx} ${ly} q24 -14 32 8 q-8 16 -32 -8 Z" fill="${color}" fill-opacity="${a * 0.95}"/>`,
    );
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">` +
    parts.join('') +
    `</svg>`
  );
}

export function dropTile(kind: MotifKind, color: string): string {
  if (kind === 'zhuyin') {
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 28" width="28" height="28">
      <path d="M4 4 H24 V24 H4 Z" fill="none" stroke="${color}" stroke-width="1.2" stroke-opacity="0.7"/>
      <path d="M8 8 H20 V20 H8 Z" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.55"/>
      <rect x="12" y="12" width="4" height="4" fill="${color}" fill-opacity="0.55"/>
    </svg>`;
  }
  if (kind === 'gold') {
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
