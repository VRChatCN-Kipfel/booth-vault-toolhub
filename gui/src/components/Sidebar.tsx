/**
 * 侧栏：固定 208px，母题底纹 + 品牌 + 导航 + 主题/明暗切换。
 */

import styled from 'styled-components';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { FONTS, motifSidebarSrc, THEME_NAMES, THEMES } from '../theme/themes';
import { brandMark } from '../theme/chrome';
import { ModeToggle } from './ModeToggle';

const SidebarWrap = styled.div`
  width: 208px;
  min-width: 208px;
  background: var(--bvt-surface2);
  border-right: 1px solid var(--bvt-glass-border);
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
`;

const MotifLayer = styled.div<{ $src: string }>`
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  background-image: url(${({ $src }) => $src});
  background-size: cover;
  background-repeat: no-repeat;
  background-position: center;
  opacity: ${({ theme }) => (theme.mode === 'dark' ? 0.46 : 0.4)};
`;

const Content = styled.div`
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
`;

const Brand = styled.div`
  padding: 22px 16px 16px;
  display: flex;
  align-items: center;
  gap: 10px;
`;

const Seal = styled.div`
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  svg { width: 100%; height: 100%; display: block; }
`;

const BrandText = styled.div`
  display: flex;
  flex-direction: column;
  line-height: 1.15;
  min-width: 0;
  .zh {
    font-size: 16px;
    font-weight: 600;
    color: var(--bvt-text);
    font-family: ${FONTS.serif};
    letter-spacing: var(--bvt-title-track, 0.16em);
  }
  .en { font-size: 10px; color: var(--bvt-text3); letter-spacing: 0.06em; margin-top: 4px; }
`;

const NAV_ITEM_H = 40;
const NAV_ITEM_GAP = 4;
const NAV_PAD_Y = 8;
const NAV_PAD_X = 10;

const Nav = styled.nav`
  flex: 1;
  padding: ${NAV_PAD_Y}px ${NAV_PAD_X}px;
  position: relative;
`;

const NavSlider = styled.div<{ $index: number }>`
  position: absolute;
  left: ${NAV_PAD_X}px;
  right: ${NAV_PAD_X}px;
  top: ${NAV_PAD_Y}px;
  height: ${NAV_ITEM_H}px;
  background: var(--bvt-sel-bg);
  border-left: var(--bvt-nav-mark);
  border-radius: var(--bvt-radius, 0px);
  box-shadow: var(--bvt-nav-ring);
  transform: translateY(${({ $index }) => $index * (NAV_ITEM_H + NAV_ITEM_GAP)}px);
  transition: transform calc(0.32s / var(--bvt-anim)) cubic-bezier(0.4, 0, 0.2, 1);
  z-index: 0;
`;

const NavKbd = styled.span`
  font-size: 10px;
  color: var(--bvt-text3);
  letter-spacing: 0.02em;
  font-weight: 400;
`;

const NavItem = styled.button<{ active: boolean }>`
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
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
  letter-spacing: 0.04em;
  cursor: pointer;
  ${({ active }) => (active ? 'font-weight: 600;' : '')}
  &:hover {
    color: ${({ active }) => (active ? 'var(--bvt-sel-text)' : 'var(--bvt-accent)')};
  }
`;

const SidebarFooter = styled.div`
  padding: 12px 12px 16px;
  border-top: 1px solid var(--bvt-border2);
  display: flex;
  flex-direction: column;
  gap: 8px;
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
  &:hover { color: var(--bvt-accent); background: transparent; }
  .mark { width: 16px; height: 16px; flex: none; }
  .mark svg { width: 100%; height: 100%; display: block; }
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
  const mod = /Mac|Macintosh/.test(navigator.userAgent) ? '⌘' : 'Ctrl+';

  return (
    <SidebarWrap>
      <MotifLayer $src={motifSidebarSrc(theme, resolved)} />
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
          {items.map((it, i) => (
            <NavItem
              key={it.key}
              type="button"
              active={active === it.key}
              aria-current={active === it.key ? 'page' : undefined}
              onClick={() => onNavigate(it.key)}
            >
              <span>{it.label}</span>
              <NavKbd>{mod}{i + 1}</NavKbd>
            </NavItem>
          ))}
        </Nav>
        <SidebarFooter>
          <GhostAction type="button" onClick={cycleTheme}>
            <span
              className="mark"
              dangerouslySetInnerHTML={{ __html: brandMark(theme, pal.accent) }}
            />
            <span>主题 · {THEME_NAMES[theme]}</span>
          </GhostAction>
          <ModeToggle />
        </SidebarFooter>
      </Content>
    </SidebarWrap>
  );
}
