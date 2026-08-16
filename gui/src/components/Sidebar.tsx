/**
 * 侧栏：固定 196px，母题底纹 + 品牌 + 导航 + 主题/明暗切换。
 */

import styled from 'styled-components';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEME_NAMES, THEMES } from '../theme/themes';
import { sidebarMotif } from '../theme/motifs';
import { ModeToggle } from './ModeToggle';

const MOTIF_KIND: Record<string, string> = {
  zhuyin: 'zhuyin',
  liujin: 'gold',
  guwen: 'guwen',
};

const SidebarWrap = styled.div`
  width: 196px;
  min-width: 196px;
  background: var(--bvt-surface2);
  border-right: 1px solid var(--bvt-border);
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
`;

/** 母题底纹层（垫底，事件穿透）。 */
const MotifLayer = styled.div`
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  svg { width: 100%; height: 100%; }
`;

const Content = styled.div`
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
`;

const Brand = styled.div`
  padding: 18px 14px 12px;
  display: flex;
  align-items: center;
  gap: 10px;
`;

const Seal = styled.div`
  width: 28px;
  height: 28px;
  svg { width: 100%; height: 100%; }
`;

const BrandText = styled.div`
  display: flex;
  flex-direction: column;
  line-height: 1.25;
  .en { font-size: 14px; font-weight: 700; color: var(--bvt-text); letter-spacing: 0.5px; }
  .zh { font-size: 11px; color: var(--bvt-text2); }
`;

/** 导航项尺寸常量（滑块与项对齐）。 */
const NAV_ITEM_H = 38;
const NAV_ITEM_GAP = 4;
const NAV_PAD_Y = 8;
const NAV_PAD_X = 8;

const Nav = styled.nav`
  flex: 1;
  padding: ${NAV_PAD_Y}px ${NAV_PAD_X}px;
  position: relative;
`;

/** 选中滑块：按 active 索引 translateY 平滑滑动。 */
const NavSlider = styled.div<{ $index: number }>`
  position: absolute;
  left: ${NAV_PAD_X}px;
  right: ${NAV_PAD_X}px;
  top: ${NAV_PAD_Y}px;
  height: ${NAV_ITEM_H}px;
  background: var(--bvt-sel-bg);
  border-left: 2px solid var(--bvt-accent);
  transform: translateY(${({ $index }) => $index * (NAV_ITEM_H + NAV_ITEM_GAP)}px);
  transition: transform calc(0.32s / var(--bvt-anim)) cubic-bezier(0.4, 0, 0.2, 1);
  z-index: 0;
`;

const NavItem = styled.button<{ active: boolean }>`
  position: relative;
  z-index: 1;
  display: block;
  width: 100%;
  height: ${NAV_ITEM_H}px;
  margin-bottom: ${NAV_ITEM_GAP}px;
  padding: 0 14px;
  text-align: left;
  background: transparent;
  color: ${({ active }) => (active ? 'var(--bvt-sel-text)' : 'var(--bvt-text2)')};
  border: none;
  font-family: inherit;
  font-size: 14px;
  cursor: pointer;
  ${({ active }) => (active ? 'font-weight: 600;' : '')}
  /* hover：active 项文字不变；未选中项文字提亮 accent */
  &:hover {
    color: ${({ active }) => (active ? 'var(--bvt-sel-text)' : 'var(--bvt-accent)')};
  }
`;

const SidebarFooter = styled.div`
  padding: 10px 12px 14px;
  border-top: 1px solid var(--bvt-border2);
  display: flex;
  flex-direction: column;
  gap: 6px;
`;

const GhostAction = styled.button`
  display: flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  border: none;
  color: var(--bvt-text2);
  font-family: inherit;
  font-size: 13px;
  cursor: pointer;
  padding: 6px 8px;
  /* hover 不压背景（母题干扰），改为文字提亮 accent */
  &:hover { color: var(--bvt-accent); background: transparent; }
  svg { width: 16px; height: 16px; }
`;

export interface NavItemDef {
  key: string;
  label: string;
}

export function Sidebar({
  items,
  active,
  onNavigate,
}: {
  items: NavItemDef[];
  active: string;
  onNavigate: (key: string) => void;
}) {
  const { theme, mode, systemTheme, cycleTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  const motifKind = MOTIF_KIND[theme];

  // 印章（简化为方印）。
  const sealSvg = (
    <svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
      <rect x="2" y="2" width="36" height="36" rx="3" fill="none" stroke={pal.accent} strokeWidth="2.4" />
      <rect x="9" y="9" width="22" height="22" fill="none" stroke={pal.accent} strokeWidth="1.4" />
      <rect x="16" y="16" width="8" height="8" fill={pal.accent} />
    </svg>
  );

  return (
    <SidebarWrap>
      <MotifLayer dangerouslySetInnerHTML={{ __html: sidebarMotif(motifKind as never, pal.accent) }} />
      <Content>
        <Brand>
          <Seal>{sealSvg}</Seal>
          <BrandText>
            <span className="en">Booth Vault Toolhub</span>
            <span className="zh">展位守护者</span>
          </BrandText>
        </Brand>
        <Nav>
          <NavSlider $index={Math.max(0, items.findIndex((it) => it.key === active))} />
          {items.map((it) => (
            <NavItem
              key={it.key}
              active={active === it.key}
              onClick={() => onNavigate(it.key)}
            >
              {it.label}
            </NavItem>
          ))}
        </Nav>
        <SidebarFooter>
          <GhostAction onClick={cycleTheme}>
            <span>主題 · {THEME_NAMES[theme]}</span>
          </GhostAction>
          <ModeToggle />
        </SidebarFooter>
      </Content>
    </SidebarWrap>
  );
}
