/**
 * 侧栏：印记 + 导航 + 主题开关。
 *
 * 当前项用一道朱记竖线加淡底标记，不用整块反色。
 * 窄窗（<860px）收成图标栏，保证小屏和分屏下仍可用。
 */

import styled from 'styled-components';
import type { LucideIcon } from 'lucide-react';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { FONTS, motifSidebarSrc, THEME_NAMES, THEMES } from '../theme/themes';
import { brandMark } from '../theme/chrome';
import { ModeToggle } from './ModeToggle';

const RAIL = 860;
const NAV_H = 36;
const NAV_GAP = 2;
const NAV_PAD_Y = 8;
const NAV_PAD_X = 8;

const Wrap = styled.aside`
  width: 200px;
  flex: none;
  position: relative;
  display: flex;
  flex-direction: column;
  background: var(--bvt-rail-bg);
  border-right: 1px solid var(--bvt-border);
  overflow: hidden;
  @media (max-width: ${RAIL}px) {
    width: 56px;
  }
`;

/** 母题退成纸背底噪，并从上往下渐隐，不与文字争；浓淡由设置页滑条给。 */
const Motif = styled.div<{ $src: string }>`
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: url("${({ $src }) => $src}") center / cover no-repeat;
  opacity: var(--bvt-motif-opacity);
  mask-image: linear-gradient(to bottom, transparent, #000 45%);
`;

const Inner = styled.div`
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
`;

const Brand = styled.div`
  display: flex;
  align-items: center;
  gap: var(--bvt-s3);
  padding: var(--bvt-s5) var(--bvt-s4) var(--bvt-s5);
  @media (max-width: ${RAIL}px) {
    padding: var(--bvt-s4) 0;
    justify-content: center;
  }
`;

const Seal = styled.div`
  width: 26px;
  height: 26px;
  flex: none;
  svg { width: 100%; height: 100%; display: block; }
`;

const BrandText = styled.div`
  display: flex;
  flex-direction: column;
  min-width: 0;
  .zh {
    font-family: ${FONTS.serif};
    font-size: var(--bvt-fz-lg);
    font-weight: 600;
    line-height: 1.2;
    letter-spacing: var(--bvt-title-track);
    color: var(--bvt-text);
  }
  .en {
    margin-top: 2px;
    font-size: 9px;
    line-height: 1;
    letter-spacing: 0.18em;
    color: var(--bvt-text3);
  }
  @media (max-width: ${RAIL}px) {
    display: none;
  }
`;

const Nav = styled.nav`
  flex: 1;
  position: relative;
  padding: ${NAV_PAD_Y}px ${NAV_PAD_X}px;
`;

/** 当前项底衬：滑动切换，避免五个按钮各自闪烁。 */
const NavMark = styled.div<{ $index: number }>`
  position: absolute;
  left: ${NAV_PAD_X}px;
  right: ${NAV_PAD_X}px;
  top: ${NAV_PAD_Y}px;
  height: ${NAV_H}px;
  background: var(--bvt-sel-bg);
  border-left: var(--bvt-mark-w) solid var(--bvt-accent);
  border-radius: var(--bvt-radius);
  transform: translateY(${({ $index }) => $index * (NAV_H + NAV_GAP)}px);
  transition: transform calc(0.32s / var(--bvt-anim)) var(--bvt-ease);
`;

const NavKbd = styled.span`
  flex: none;
  font-size: 10px;
  color: var(--bvt-text3);
  opacity: 0;
  transition: opacity 0.15s var(--bvt-ease);
  @media (max-width: ${RAIL}px) {
    display: none;
  }
`;

const NavLabel = styled.span`
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  @media (max-width: ${RAIL}px) {
    display: none;
  }
`;

const NavItem = styled.button<{ $active: boolean }>`
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--bvt-s3);
  width: 100%;
  height: ${NAV_H}px;
  margin-bottom: ${NAV_GAP}px;
  padding: 0 var(--bvt-s3);
  background: transparent;
  border: none;
  border-radius: var(--bvt-radius);
  font-size: var(--bvt-fz-md);
  cursor: pointer;
  color: ${({ $active }) => ($active ? 'var(--bvt-sel-text)' : 'var(--bvt-text2)')};
  font-weight: ${({ $active }) => ($active ? 600 : 400)};
  transition: color 0.16s var(--bvt-ease);
  svg { width: 15px; height: 15px; flex: none; }
  &:hover { color: ${({ $active }) => ($active ? 'var(--bvt-sel-text)' : 'var(--bvt-text)')}; }
  &:hover ${NavKbd}, &:focus-visible ${NavKbd} { opacity: 1; }
  @media (max-width: ${RAIL}px) {
    justify-content: center;
    padding: 0;
  }
`;

const Footer = styled.div`
  display: flex;
  flex-direction: column;
  gap: var(--bvt-s2);
  padding: var(--bvt-s3);
  border-top: 1px solid var(--bvt-border2);
  @media (max-width: ${RAIL}px) {
    align-items: center;
    padding: var(--bvt-s2) 0;
  }
`;

const ThemeSwitch = styled.button`
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  height: 28px;
  padding: 0 var(--bvt-s2);
  background: transparent;
  border: none;
  border-radius: var(--bvt-radius);
  color: var(--bvt-text2);
  font-size: var(--bvt-fz-sm);
  cursor: pointer;
  &:hover { background: var(--bvt-hover); color: var(--bvt-text); }
  .mark { width: 14px; height: 14px; flex: none; }
  .mark svg { width: 100%; height: 100%; display: block; }
  .name {
    @media (max-width: ${RAIL}px) {
      display: none;
    }
  }
`;

export interface NavItemDef {
  key: string;
  label: string;
  icon: LucideIcon;
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
  const { theme, mode, systemTheme, motifImage, cycleTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  const mod = /Mac|Macintosh/.test(navigator.userAgent) ? '⌘' : 'Ctrl ';
  const index = Math.max(0, items.findIndex((it) => it.key === active));

  return (
    <Wrap>
      <Motif $src={motifImage ?? motifSidebarSrc(theme, resolved)} />
      <Inner>
        <Brand>
          <Seal dangerouslySetInnerHTML={{ __html: brandMark(theme, pal.accent) }} />
          <BrandText>
            <span className="zh">展位库</span>
            <span className="en">BOOTH VAULT</span>
          </BrandText>
        </Brand>

        <Nav>
          <NavMark $index={index} />
          {items.map((it, i) => (
            <NavItem
              key={it.key}
              type="button"
              $active={active === it.key}
              aria-current={active === it.key ? 'page' : undefined}
              title={it.label}
              onClick={() => onNavigate(it.key)}
            >
              <it.icon />
              <NavLabel>{it.label}</NavLabel>
              <NavKbd>{mod}{i + 1}</NavKbd>
            </NavItem>
          ))}
        </Nav>

        <Footer>
          <ThemeSwitch type="button" onClick={cycleTheme} title={`主题 · ${THEME_NAMES[theme]}`}>
            <span className="mark" dangerouslySetInnerHTML={{ __html: brandMark(theme, pal.accent) }} />
            <span className="name">主题 · {THEME_NAMES[theme]}</span>
          </ThemeSwitch>
          <ModeToggle />
        </Footer>
      </Inner>
    </Wrap>
  );
}
