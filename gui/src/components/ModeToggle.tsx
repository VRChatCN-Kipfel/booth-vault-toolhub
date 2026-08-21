/**
 * 明暗开关滑块：三态循环（亮色 → 系统 → 深色）。
 * 滑块平滑滑动 + 全局颜色过渡同时进行。
 * 系统态显示 Monitor 图标。
 */

import styled from 'styled-components';
import { Sun, Moon, Monitor } from 'lucide-react';
import { useThemeStore, type ModePref } from '../store/themeStore';

/** 窄窗收成图标栏时，滑轨塌成单个按钮（见 Sidebar 的 RAIL 断点）。 */
const RAIL = 860;

const Track = styled.button`
  position: relative;
  display: flex;
  align-items: center;
  width: 76px;
  height: 28px;
  padding: 0 3px;
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-pill);
  background: var(--bvt-input-bg);
  cursor: pointer;
  transition: border-color 0.16s var(--bvt-ease);
  &:hover { border-color: color-mix(in srgb, var(--bvt-text3) 55%, var(--bvt-border)); }
  @media (max-width: ${RAIL}px) {
    width: 28px;
    padding: 0;
  }
`;

/** 三态位置：亮(0) / 系统(1) / 暗(2)。 */
const Knob = styled.span<{ $state: number }>`
  position: absolute;
  top: 2px;
  left: 2px;
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--bvt-pill);
  background: var(--bvt-accent);
  transform: translateX(${({ $state }) => $state * 24}px);
  transition: transform calc(0.3s / var(--bvt-anim)) var(--bvt-ease),
    background-color calc(0.35s / var(--bvt-anim));
  svg { width: 12px; height: 12px; color: var(--bvt-on-accent); }
  @media (max-width: ${RAIL}px) {
    transform: none;
  }
`;

const Icons = styled.div`
  display: flex;
  justify-content: space-between;
  width: 100%;
  padding: 0 5px;
  color: var(--bvt-text3);
  svg { width: 12px; height: 12px; }
  @media (max-width: ${RAIL}px) {
    display: none;
  }
`;

const MODE_ORDER: ModePref[] = ['light', 'system', 'dark'];

export function ModeToggle() {
  const { mode, cycleMode } = useThemeStore();
  const state = MODE_ORDER.indexOf(mode);
  const knobIcon = mode === 'dark' ? <Moon /> : mode === 'system' ? <Monitor /> : <Sun />;
  return (
    <Track
      type="button"
      onClick={cycleMode}
      title="明暗切换（亮色/系统/深色）"
      aria-label="明暗切换（亮色/系统/深色）"
    >
      <Icons>
        <Sun />
        <Monitor />
        <Moon />
      </Icons>
      <Knob $state={Math.max(0, state)}>{knobIcon}</Knob>
    </Track>
  );
}
