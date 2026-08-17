/**
 * 基础 UI：形制随主题变，色板走 CSS variables。
 */

import styled from 'styled-components';
import { themeRadius } from '../theme/chrome';
import { FONTS } from '../theme/themes';

export const AccentButton = styled.button`
  background: var(--bvt-btn-fill);
  color: var(--bvt-on-btn);
  border: 1px solid var(--bvt-accent-deep);
  border-top: 1px solid color-mix(in srgb, var(--bvt-accent-light) 70%, var(--bvt-btn-fill));
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
  padding: 8px 18px;
  font-size: 13px;
  letter-spacing: ${({ theme }) => (theme.theme === 'zhuyin' ? '0.14em' : theme.theme === 'liujin' ? '0.08em' : '0.04em')};
  cursor: pointer;
  font-family: ${({ theme }) => (theme.theme === 'zhuyin' ? FONTS.serif : 'inherit')};
  box-shadow: ${({ theme }) =>
    theme.theme === 'liujin'
      ? 'inset 0 1px 0 color-mix(in srgb, var(--bvt-accent) 42%, transparent)'
      : 'none'};
  &:hover { background: var(--bvt-btn-fill-hover); }
  &:active { background: var(--bvt-btn-fill-press); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
`;

export const SecondaryButton = styled.button`
  background: transparent;
  color: var(--bvt-accent);
  border: 1.5px solid var(--bvt-accent);
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
  padding: 7px 15px;
  font-size: 13px;
  letter-spacing: 0.04em;
  cursor: pointer;
  font-family: inherit;
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
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
  padding: 14px;
`;

export const Input = styled.input`
  background: var(--bvt-input-bg-70);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  color: var(--bvt-text);
  border: 1px solid var(--bvt-border);
  border-left: 3px solid var(--bvt-accent);
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  &:focus { border-color: var(--bvt-accent); box-shadow: inset 0 0 0 1px var(--bvt-accent-light); }
  &::placeholder { color: var(--bvt-text3); }
`;

export const TextArea = styled.textarea`
  background: var(--bvt-input-bg-70);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  color: var(--bvt-text);
  border: 1px solid var(--bvt-border);
  border-left: 3px solid var(--bvt-accent);
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  resize: none;
  &:focus { border-color: var(--bvt-accent); box-shadow: inset 0 0 0 1px var(--bvt-accent-light); }
  &::placeholder { color: var(--bvt-text3); }
`;

export const ObsPanel = styled.div`
  background: var(--bvt-surface2-70);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  border: 1px solid var(--bvt-border2);
  border-top: 2px solid var(--bvt-accent);
  overflow-y: auto;
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
  padding: 6px;
`;

export const PanelLabel = styled.div`
  font-size: 13px;
  font-weight: 600;
  color: var(--bvt-text2);
  padding: 2px 0 4px;
  letter-spacing: ${({ theme }) => (theme.theme === 'zhuyin' ? '0.12em' : '0.03em')};
  font-family: ${({ theme }) => (theme.theme === 'zhuyin' ? FONTS.serif : 'inherit')};
`;

export const ProgressBar = styled.div`
  height: 7px;
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-border);
  border-radius: ${({ theme }) => themeRadius(theme.theme)};
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
  border: 1px solid var(--bvt-border);
  border-radius: ${({ theme }) =>
    theme.theme === 'zhuyin' ? '0px' : `${SEG_H / 2}px`};
  padding: 2px;
`;

const SegSliderKnob = styled.div<{ $index: number; $accent: string }>`
  position: absolute;
  top: 2px;
  left: 2px;
  width: ${SEG_W}px;
  height: ${SEG_H - 6}px;
  background: ${({ $accent }) => $accent};
  border-radius: ${({ theme }) =>
    theme.theme === 'zhuyin' ? '0px' : `${(SEG_H - 6) / 2}px`};
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
  border-radius: ${({ theme }) =>
    theme.theme === 'zhuyin' ? '0px' : `${(SEG_H - 6) / 2}px`};
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
  return (
    <SegWrap>
      <SegSliderKnob $index={value} $accent={accent} />
      {options.map((label, i) => (
        <SegItem
          key={label}
          $selected={i === value}
          onClick={() => onChange(i)}
        >
          {label}
        </SegItem>
      ))}
    </SegWrap>
  );
}
