/**
 * 页题：宋体落款 + 随主题变的花饰。
 */

import styled from 'styled-components';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEMES } from '../theme/themes';
import { titleOrnament } from '../theme/chrome';

const Wrapper = styled.div`
  display: flex;
  align-items: flex-end;
  gap: 12px;
  margin-bottom: 2px;
`;

const Title = styled.h2`
  font-size: 22px;
  font-weight: 600;
  color: var(--bvt-text);
  font-family: var(--bvt-serif);
  margin: 0;
  letter-spacing: var(--bvt-title-track, 0.12em);
  line-height: 1.15;
`;

const Motif = styled.div`
  flex: 1;
  min-width: 80px;
  height: 36px;
  opacity: 0.9;
  svg { width: 168px; height: 36px; display: block; }
`;

export function PageTitle({ title }: { title: string }) {
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  return (
    <Wrapper>
      <Title>{title}</Title>
      <Motif dangerouslySetInnerHTML={{ __html: titleOrnament(theme, pal.accent) }} />
    </Wrapper>
  );
}
