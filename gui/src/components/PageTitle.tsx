/**
 * 页面标题：标题文字 + 花饰（对齐原版 _motif_title，theme.py:129-147）。
 * 150×44 主弧 + 分支 + 节点圆，随 accent 变色。
 */

import styled from 'styled-components';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEMES } from '../theme/themes';

const Wrapper = styled.div`
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 2px;
`;

const Title = styled.h2`
  font-size: 18px;
  font-weight: 700;
  color: var(--bvt-text);
  font-family: inherit;
  margin: 0;
  letter-spacing: 1px;
`;

const Motif = styled.div`
  width: 150px;
  height: 44px;
  svg { width: 100%; height: 100%; }
`;

/** 花饰 SVG（对齐 _motif_title）。 */
function titleMotif(color: string): string {
  const parts: string[] = [];
  // 主弧
  parts.push(
    `<path d="M0 40 C50 38,95 28,148 6" fill="none" stroke="${color}" stroke-width="1.2" stroke-opacity="0.55"/>`,
  );
  // 4 分支
  const branches = [
    'M30 38 C40 34,48 28,52 20',
    'M60 36 C68 30,74 22,78 14',
    'M90 33 C98 26,104 18,108 10',
    'M120 30 C126 24,132 16,136 8',
  ];
  for (const d of branches) {
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="0.8" stroke-opacity="0.42"/>`);
  }
  // 3 节点圆
  for (const [cx, cy] of [[20, 40], [52, 20], [78, 14]]) {
    parts.push(`<circle cx="${cx}" cy="${cy}" r="1.6" fill="${color}" fill-opacity="0.6"/>`);
  }
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 150 44" width="150" height="44">` +
    parts.join('') +
    `</svg>`
  );
}

export function PageTitle({ title }: { title: string }) {
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  return (
    <Wrapper>
      <Title>{title}</Title>
      <Motif dangerouslySetInnerHTML={{ __html: titleMotif(pal.accent) }} />
    </Wrapper>
  );
}
