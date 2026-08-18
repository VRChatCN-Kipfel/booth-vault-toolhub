/**
 * 明暗开关滑块：三态循环（亮色 → 系统 → 深色）。
 * 滑块平滑滑动 + 全局颜色过渡同时进行。
 * 系统态显示 Monitor 图标。
 */

import styled from 'styled-components';
import { Sun, Moon, Monitor } from 'lucide-react';
import { useThemeStore, type ModePref } from '../store/themeStore';

const Track = styled.button<{ $state: number }>`
  display: flex;
  align-items: center;
  width: 72px;
  height: 28px;
  border-radius: var(--bvt-pill-radius, 0px);
  border: 1px solid var(--bvt-glass-border);
  background: var(--bvt-input-bg);
  box-shadow: var(--bvt-glass-highlight);
  padding: 0 3px;
  cursor: pointer;
  position: relative;
  transition: background-color calc(0.35s / var(--bvt-anim)) ease, border-color calc(0.35s / var(--bvt-anim)) ease;
`;

/** 三态位置：亮(0) / 系统(1) / 暗(2)。 */
const Knob = styled.span<{ $state: number }>`
  position: absolute;
  top: 2px;
  width: 22px;
  height: 22px;
  border-radius: var(--bvt-knob-radius, 0px);
  background: var(--bvt-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform calc(0.3s / var(--bvt-anim)) cubic-bezier(0.4, 0, 0.2, 1), background-color calc(0.35s / var(--bvt-anim));
  transform: translateX(${({ $state }) => $state * 24}px);
  svg { width: 12px; height: 12px; color: var(--bvt-on-accent); }
`;

const Icons = styled.div`
  display: flex;
  justify-content: space-between;
  width: 100%;
  padding: 0 4px;
  color: var(--bvt-text2);
  svg { width: 12px; height: 12px; }
`;

const MODE_ORDER: ModePref[] = ['light', 'system', 'dark'];

export function ModeToggle() {
  const { mode, cycleMode } = useThemeStore();
  const state = MODE_ORDER.indexOf(mode);
  const knobIcon = mode === 'dark' ? <Moon /> : mode === 'system' ? <Monitor /> : <Sun />;
  return (
    <Track
      type="button"
      $state={Math.max(0, state)}
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
