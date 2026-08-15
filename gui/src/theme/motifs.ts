/**
 * SVG 母题生成器：三主题侧栏纹样 + 根背景脉络。
 *
 * 算法参数 1:1 复刻自 booth-keeper theme.py 的 _seigaiha_layer /
 * _motif_sidebar / _motif_bg_* 系列。
 */

import type { MotifKind } from './themes';

/** 青海波层（鎏金侧栏专有，theme.py:310-334）。 */
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
      // 3 层同心半圆（k=3,2,1）
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

/** 鎏金侧栏纹样（金线脉络 + 青海波，theme.py:68-88）。 */
function goldSidebar(color: string): string {
  const parts: string[] = [];
  // 主脉络
  parts.push(
    `<path d="M30 0 C12 80,48 160,30 250 C12 340,48 430,30 520 C16 600,46 660,30 720" fill="none" stroke="${color}" stroke-width="1.6" stroke-opacity="0.72"/>`,
  );
  // 副脉络
  parts.push(
    `<path d="M44 0 C60 90,30 170,46 270 C62 360,34 450,48 560 C60 640,40 690,52 720" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="0.5"/>`,
  );
  // 细金网络 5 条
  const thin = [
    'M30 120 C40 150,48 170,60 190',
    'M30 250 C20 280,14 300,4 320',
    'M30 400 C40 430,50 455,60 480',
    'M30 520 C20 550,12 570,0 590',
    'M44 270 C52 300,58 330,64 360',
  ];
  for (const d of thin) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="0.5" stroke-opacity="0.24"/>`);
  }
  // 分支 7 条
  const branch = [
    'M30 90 C50 104,62 116,72 134',
    'M30 200 C10 216,2 232,0 252',
    'M30 330 C50 344,64 356,72 372',
    'M30 450 C10 466,4 482,0 500',
    'M30 620 C50 634,64 646,72 662',
    'M44 180 C58 200,66 220,72 240',
    'M44 360 C30 380,20 400,14 420',
  ];
  for (const d of branch) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.5"/>`);
  }
  // 节点圆
  for (const [cx, cy, rad, op] of [
    [30, 90, 2.4, 0.85],
    [30, 250, 2.4, 0.85],
    [30, 400, 2.4, 0.85],
    [30, 560, 2.4, 0.85],
    [46, 270, 1.8, 0.85],
    [48, 560, 1.8, 0.85],
  ] as Array<[number, number, number, number]>) {
    parts.push(
      `<circle cx="${cx}" cy="${cy}" r="${rad}" fill="${color}" fill-opacity="${op}"/>`,
    );
  }
  // 青海波叠加
  parts.push(seigaihaLayer(color));
  return parts.join('');
}

/** 朱印侧栏纹样（回字纹 + 印章，theme.py:89-105）。 */
function zhuyinSidebar(color: string): string {
  const parts: string[] = [];
  // 回字纹竖列 11 个
  for (let y = 20; y < 700; y += 64) {
    parts.push(
      `<path d="M14 ${y} H58 V${y + 44} H22 V${y + 10} H50 V${y + 34} H30" fill="none" stroke="${color}" stroke-width="1.4" stroke-opacity="0.8"/>`,
    );
  }
  // 主脉
  parts.push(
    `<path d="M36 0 C22 90,50 180,36 280 C24 380,52 470,36 560 C26 640,48 690,36 720" fill="none" stroke="${color}" stroke-width="1.4" stroke-opacity="0.55"/>`,
  );
  // 印章方结
  parts.push(
    `<rect x="24" y="330" width="24" height="24" fill="none" stroke="${color}" stroke-width="1.6" stroke-opacity="0.7"/>`,
    `<rect x="28" y="334" width="16" height="16" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.4"/>`,
    `<rect x="31" y="341" width="10" height="10" fill="${color}" fill-opacity="0.85"/>`,
  );
  return parts.join('');
}

/** 古纹侧栏纹样（云雷纹 + 叶脉，theme.py:106-125）。 */
function guwenSidebar(color: string): string {
  const parts: string[] = [];
  // 云雷纹回旋 5 个
  for (const [sx, sy] of [
    [20, 120],
    [52, 240],
    [18, 410],
    [50, 540],
    [24, 660],
  ]) {
    parts.push(
      `<path d="M${sx} ${sy} a8 8 0 1 1 -8 -8 a4 4 0 1 0 4 4" fill="none" stroke="${color}" stroke-width="1.1" stroke-opacity="0.6"/>`,
    );
  }
  // 主茎
  parts.push(
    `<path d="M8 720 C14 560,30 420,30 280 C30 160,44 80,62 0" fill="none" stroke="${color}" stroke-width="1.6" stroke-opacity="0.6"/>`,
  );
  // 分支
  const branch = [
    'M30 300 C16 320,8 340,4 360',
    'M30 220 C44 240,54 256,60 280',
    'M30 140 C16 160,8 180,4 200',
    'M30 60 C44 80,54 100,62 130',
    'M40 380 C28 400,22 420,20 445',
    'M20 500 C34 520,44 540,52 560',
  ];
  for (const d of branch) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1" stroke-opacity="0.45"/>`);
  }
  // 小叶填充
  parts.push(
    `<path d="M62 180 C70 172,74 160,72 148 C64 150,58 158,56 168 Z" fill="${color}" fill-opacity="0.22"/>`,
    `<path d="M30 300 C20 300,12 306,8 316 C16 322,24 322,30 316 Z" fill="${color}" fill-opacity="0.22"/>`,
    `<path d="M40 380 C48 378,54 384,56 394 C48 400,40 398,36 390 Z" fill="${color}" fill-opacity="0.22"/>`,
  );
  return parts.join('');
}

/** 侧栏竖幅纹样（72×720）。 */
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

/** 根背景脉络：按主题分派（theme.py:282-294）。 */
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

/** 鎏金金缮脉络根背景（theme.py:167-199）。 */
function goldBg(color: string, light: boolean): string {
  const s = light ? 4.2 : 3.4;
  const mainOp = (0.13 * s).toFixed(3);
  const thinOp = (0.07 * s).toFixed(3);
  const nodeOp = (0.16 * s).toFixed(3);
  const mainW = light ? 1.2 : 1.0;
  const thinW = light ? 0.6 : 0.5;
  const parts: string[] = [];
  const mains = [
    'M90 -5 C220 120,150 240,300 330 C470 440,360 560,540 660 C640 720,700 760,760 785',
    'M1210 -5 C1020 110,1140 250,980 360 C840 470,940 590,760 700 C680 760,620 800,560 830',
    'M-5 300 C80 340,140 360,220 380 C340 410,440 470,520 560',
    'M-5 560 C100 600,180 620,300 640 C420 660,520 720,620 800',
  ];
  for (const d of mains) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="${mainW}" stroke-opacity="${mainOp}"/>`);
  }
  const thins = [
    'M300 330 C360 300,420 280,500 250',
    'M540 660 C600 630,660 610,720 590',
    'M980 360 C900 330,840 310,780 300',
    'M360 420 C420 400,480 390,560 380',
    'M760 700 C680 720,600 730,520 740',
    'M460 660 C400 680,340 690,280 700',
  ];
  for (const d of thins) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="${thinW}" stroke-opacity="${thinOp}"/>`);
  }
  for (const [cx, cy] of [
    [300, 330],
    [540, 660],
    [980, 360],
    [360, 420],
    [760, 700],
    [460, 660],
  ]) {
    parts.push(`<circle cx="${cx}" cy="${cy}" r="2" fill="${color}" fill-opacity="${nodeOp}"/>`);
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="780" viewBox="0 0 1200 780">` +
    parts.join('') +
    `</svg>`
  );
}

/** 朱印缠枝卷云根背景（theme.py:208-246）。 */
function cinnabarBg(color: string, light: boolean): string {
  const a = light ? 0.22 : 0.17;
  const a2 = light ? 0.16 : 0.12;
  const parts: string[] = [];
  const vines = [
    'M90 -5 C220 120,150 240,300 330 C470 440,360 560,540 660 C640 720,700 760,760 785',
    'M1210 -5 C1020 110,1140 250,980 360 C840 470,940 590,760 700',
    'M-5 300 C80 340,140 360,220 380 C340 410,440 470,520 560',
    'M-5 560 C100 600,180 620,300 640',
  ];
  for (const d of vines) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1.8" stroke-opacity="${a}"/>`);
  }
  const clouds: Array<[number, number]> = [
    [300, 330], [540, 660], [980, 360], [360, 420],
    [760, 700], [460, 660], [820, 540], [240, 250],
  ];
  for (const [cx, cy] of clouds) {
    parts.push(
      `<path d="M${cx} ${cy} a11 11 0 1 1 -11 -11 a5 5 0 1 0 5 5" fill="none" stroke="${color}" stroke-width="1.4" stroke-opacity="${a2}"/>`,
    );
  }
  const leaves: Array<[number, number]> = [
    [240, 330], [500, 420], [740, 560], [420, 660], [880, 540], [300, 250],
  ];
  for (const [lx, ly] of leaves) {
    parts.push(
      `<path d="M${lx} ${ly} q22 -12 30 8 q-8 14 -30 -8 Z" fill="${color}" fill-opacity="${a2}"/>`,
    );
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">` +
    parts.join('') +
    `</svg>`
  );
}

/** 古纹青绿叶脉根背景（theme.py:249-279）。 */
function leafBg(color: string, light: boolean): string {
  const a = light ? 0.26 : 0.2;
  const a2 = 0.22;
  const parts: string[] = [];
  const stems = [
    'M90 -5 C220 120,150 240,300 330 C470 440,360 560,540 660',
    'M1210 -5 C1020 110,1140 250,980 360',
    'M-5 300 C80 340,140 360,220 380',
    'M-5 560 C100 600,180 620,300 640',
  ];
  for (const d of stems) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1.8" stroke-opacity="${a}"/>`);
  }
  const leaves: Array<[number, number]> = [
    [240, 330], [500, 420], [740, 560], [420, 660],
    [880, 540], [300, 250], [600, 720], [120, 380],
  ];
  for (const [lx, ly] of leaves) {
    parts.push(
      `<path d="M${lx} ${ly} q22 -12 30 8 q-8 14 -30 -8 Z" fill="${color}" fill-opacity="${a2}"/>`,
    );
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">` +
    parts.join('') +
    `</svg>`
  );
}
