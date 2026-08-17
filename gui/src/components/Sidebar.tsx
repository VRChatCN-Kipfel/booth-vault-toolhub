/**
 * 侧栏：固定 196px，母题底纹 + 品牌 + 导航 + 主题/明暗切换。
 */

import styled from 'styled-components';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEME_NAMES, THEMES } from '../theme/themes';
import { sidebarMotif } from '../theme/motifs';
import { brandMark } from '../theme/chrome';
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
  padding: 20px 14px 14px;
  display: flex;
  align-items: center;
  gap: 10px;
`;

const Seal = styled.div`
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  svg { width: 100%; height: 100%; display: block; }
`;

const BrandText = styled.div`
  display: flex;
  flex-direction: column;
  line-height: 1.2;
  min-width: 0;
  .zh {
    font-size: 15px;
    font-weight: 600;
    color: var(--bvt-text);
    font-family: 'Noto Serif CJK SC','Songti SC','STSong',serif;
    letter-spacing: 0.16em;
  }
  .en { font-size: 10px; color: var(--bvt-text3); letter-spacing: 0.04em; margin-top: 3px; }
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

  return (
    <SidebarWrap>
      <MotifLayer dangerouslySetInnerHTML={{ __html: sidebarMotif(motifKind as never, pal.accent) }} />
      <Content>
        <Brand>
          <Seal dangerouslySetInnerHTML={{ __html: brandMark(theme, pal.accent) }} />
          <BrandText>
            <span className="zh">展位库</span>
            <span className="en">Booth Vault</span>
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
