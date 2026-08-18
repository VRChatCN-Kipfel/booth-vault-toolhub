/**
 * 基础 UI：形制随主题变，色板走 CSS variables。
 */

import styled from 'styled-components';
import type { KeyboardEvent } from 'react';

export const AccentButton = styled.button`
  background: var(--bvt-btn-fill);
  color: var(--bvt-on-btn);
  border: 1px solid var(--bvt-accent-deep);
  border-top: 1px solid color-mix(in srgb, var(--bvt-accent-light) 70%, var(--bvt-btn-fill));
  border-radius: var(--bvt-radius, 0px);
  padding: 8px 18px;
  font-size: 13px;
  letter-spacing: var(--bvt-btn-track, 0.08em);
  cursor: pointer;
  font-family: var(--bvt-btn-font, inherit);
  box-shadow: var(--bvt-btn-sheen, none);
  &:hover { background: var(--bvt-btn-fill-hover); }
  &:active { background: var(--bvt-btn-fill-press); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
`;

export const SecondaryButton = styled.button`
  background: var(--bvt-input-bg);
  color: var(--bvt-accent);
  border: 1.5px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  padding: 7px 15px;
  font-size: 13px;
  letter-spacing: 0.04em;
  cursor: pointer;
  font-family: inherit;
  box-shadow: var(--bvt-glass-highlight);
  &:hover { background: var(--bvt-accent-light); color: var(--bvt-accent-deep); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
`;

export const GhostButton = styled.button`
  background: transparent;
  color: var(--bvt-text2);
  border: none;
  padding: 6px 12px;
  font-size: 13px;
  cursor: pointer;
  font-family: inherit;
  &:hover { color: var(--bvt-text); background: var(--bvt-hover); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
`;

export const Card = styled.div`
  background: var(--bvt-surface);
  border: 1px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  padding: 14px;
  box-shadow: var(--bvt-glass-highlight);
`;

export const Input = styled.input`
  background: var(--bvt-input-bg);
  color: var(--bvt-text);
  border: 1px solid var(--bvt-glass-border);
  border-left: 3px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  box-shadow: var(--bvt-glass-highlight);
  &:focus { border-color: var(--bvt-accent); box-shadow: inset 0 0 0 1px var(--bvt-accent-light); }
  &:focus-visible { outline: 2px solid var(--bvt-accent); outline-offset: 2px; }
  &::placeholder { color: var(--bvt-text3); }
`;

export const TextArea = styled.textarea`
  background: var(--bvt-input-bg);
  color: var(--bvt-text);
  border: 1px solid var(--bvt-glass-border);
  border-left: 3px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  resize: none;
  box-shadow: var(--bvt-glass-highlight);
  &:focus { border-color: var(--bvt-accent); box-shadow: inset 0 0 0 1px var(--bvt-accent-light); }
  &:focus-visible { outline: 2px solid var(--bvt-accent); outline-offset: 2px; }
  &::placeholder { color: var(--bvt-text3); }
`;

export const ObsPanel = styled.div`
  background: var(--bvt-input-bg);
  border: 1px solid var(--bvt-glass-border);
  border-top: 2px solid var(--bvt-accent);
  overflow-y: auto;
  border-radius: var(--bvt-radius, 0px);
  padding: 6px;
`;

export const PanelLabel = styled.div`
  font-size: 13px;
  font-weight: 600;
  color: var(--bvt-text2);
  padding: 2px 0 4px;
  letter-spacing: var(--bvt-label-track, 0.03em);
  font-family: var(--bvt-label-font, inherit);
`;

export const ProgressBar = styled.div`
  height: 7px;
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius, 0px);
  overflow: hidden;
  & > div {
    height: 100%;
    background: linear-gradient(to right, var(--bvt-accent-deep), var(--bvt-accent));
    transition: width 0.2s;
  }
`;

export const PageShell = styled.div`
  padding: 22px 24px 18px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow-y: auto;
`;

export const Lead = styled.p`
  margin: -8px 0 4px;
  color: var(--bvt-text2);
  font-size: 12.5px;
  letter-spacing: 0.05em;
`;

export const Badge = styled.span<{ kind: 'ok' | 'run' | 'wait' | 'warn' | 'err' }>`
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--bvt-radius, 2px);
  font-size: 11px;
  letter-spacing: 0.04em;
  ${({ kind }) => {
    switch (kind) {
      case 'ok':
        return 'background: var(--bvt-success-l); color: var(--bvt-success);';
      case 'run':
        return 'background: var(--bvt-sel-bg); color: var(--bvt-sel-text);';
      case 'warn':
        return 'background: var(--bvt-warn-l); color: var(--bvt-warn);';
      case 'err':
        return 'background: var(--bvt-danger-l); color: var(--bvt-danger);';
      default:
        return 'background: var(--bvt-border2); color: var(--bvt-text2);';
    }
  }}
`;

const SEG_W = 64;
const SEG_H = 32;

const SegWrap = styled.div`
  position: relative;
  display: inline-flex;
  flex: none;
  width: fit-content;
  height: ${SEG_H}px;
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-glass-border);
  border-radius: var(--bvt-pill-radius, 0px);
  padding: 2px;
  box-shadow: var(--bvt-glass-highlight);
`;

const SegSliderKnob = styled.div<{ $index: number; $accent: string }>`
  position: absolute;
  top: 2px;
  left: 2px;
  width: ${SEG_W}px;
  height: ${SEG_H - 6}px;
  background: ${({ $accent }) => $accent};
  border-radius: var(--bvt-pill-radius, 0px);
  transform: translateX(${({ $index }) => $index * SEG_W}px);
  transition: transform calc(0.3s / var(--bvt-anim)) cubic-bezier(0.4, 0, 0.2, 1), background-color calc(0.35s / var(--bvt-anim));
  z-index: 0;
`;

const SegItem = styled.button<{ $selected: boolean }>`
  position: relative;
  z-index: 1;
  width: ${SEG_W}px;
  height: ${SEG_H - 6}px;
  border: none;
  border-radius: var(--bvt-pill-radius, 0px);
  background: transparent;
  color: ${({ $selected }) => ($selected ? 'var(--bvt-on-accent)' : 'var(--bvt-text2)')};
  font-family: inherit;
  font-size: 13px;
  cursor: pointer;
  font-weight: ${({ $selected }) => ($selected ? 600 : 400)};
  transition: color 0.25s ease;
`;

export function SegSlider({
  options,
  value,
  accent,
  onChange,
}: {
  options: string[];
  value: number;
  accent: string;
  onChange: (index: number) => void;
}) {
  const move = (next: number, el: HTMLElement) => {
    const clamped = Math.max(0, Math.min(options.length - 1, next));
    if (clamped !== value) onChange(clamped);
    const radios = el.closest('[role="radiogroup"]')?.querySelectorAll<HTMLElement>('[role="radio"]');
    radios?.[clamped]?.focus();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') move(value + 1, e.currentTarget);
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') move(value - 1, e.currentTarget);
    else if (e.key === 'Home') move(0, e.currentTarget);
    else if (e.key === 'End') move(options.length - 1, e.currentTarget);
    else return;
    e.preventDefault();
  };
  return (
    <SegWrap role="radiogroup">
      <SegSliderKnob $index={value} $accent={accent} />
      {options.map((label, i) => (
        <SegItem
          key={label}
          type="button"
          role="radio"
          aria-checked={i === value}
          tabIndex={i === value ? 0 : -1}
          $selected={i === value}
          onClick={() => onChange(i)}
          onKeyDown={onKeyDown}
        >
          {label}
        </SegItem>
      ))}
    </SegWrap>
  );
}
