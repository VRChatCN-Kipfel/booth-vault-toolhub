/**
 * 基础 UI 组件库（styled-components）。
 * 对齐 booth-keeper 的形状体系：直角 / 2px 微圆角 / flat 无阴影 / 1px 分隔线。
 */

import styled from 'styled-components';

export const AccentButton = styled.button`
  background: var(--bvt-btn-fill);
  color: #fafafa;
  border: 1px solid var(--bvt-accent-deep);
  border-top: 1px solid var(--bvt-accent-light);
  border-radius: 2px;
  padding: 8px 16px;
  font-size: 14px;
  cursor: pointer;
  font-family: inherit;
  &:hover { background: var(--bvt-btn-fill-hover); }
  &:active { background: var(--bvt-btn-fill-press); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
`;

export const SecondaryButton = styled.button`
  background: transparent;
  color: var(--bvt-accent);
  border: 1.5px solid var(--bvt-accent);
  border-radius: 2px;
  padding: 7px 15px;
  font-size: 14px;
  cursor: pointer;
  font-family: inherit;
  &:hover { background: var(--bvt-accent-light); }
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
  border-radius: 2px;
  padding: 14px;
`;

export const Input = styled.input`
  background: var(--bvt-input-bg-70);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  color: var(--bvt-text);
  border: 1.5px solid var(--bvt-accent-deep);
  border-left: 4px solid var(--bvt-accent);
  border-radius: 2px;
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  &:focus { border-color: var(--bvt-accent); }
  &::placeholder { color: var(--bvt-text3); }
`;

export const TextArea = styled.textarea`
  background: var(--bvt-input-bg-70);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  color: var(--bvt-text);
  border: 1.5px solid var(--bvt-accent-deep);
  border-left: 4px solid var(--bvt-accent);
  border-radius: 2px;
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  resize: none;
  &:focus { border-color: var(--bvt-accent); }
  &::placeholder { color: var(--bvt-text3); }
`;

/** 观测/队列面板（对齐 QListWidget#obs：surface2 底 + 顶部 2px accent 规）。 */
export const ObsPanel = styled.div`
  background: var(--bvt-surface2-70);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  border-top: 2px solid var(--bvt-accent);
  overflow-y: auto;
  border-radius: 0;
  padding: 6px;
`;

/** 面板标题（对齐原版列表上方的标题文字）。 */
export const PanelLabel = styled.div`
  font-size: 13px;
  font-weight: 600;
  color: var(--bvt-text2);
  padding: 2px 0 4px;
`;

export const ProgressBar = styled.div`
  height: 10px;
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-border);
  border-radius: 0;
  overflow: hidden;
  & > div {
    height: 100%;
    background: linear-gradient(to right, var(--bvt-accent-deep), var(--bvt-accent));
    transition: width 0.2s;
  }
`;

/** 徽章 5 态（ok/run/wait/warn/err）。 */
export const Badge = styled.span<{ kind: 'ok' | 'run' | 'wait' | 'warn' | 'err' }>`
  display: inline-block;
  padding: 2px 8px;
  border-radius: 2px;
  font-size: 11px;
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

/** 选项段宽。 */
const SEG_W = 64;
/** 段高（胶囊高度）。 */
const SEG_H = 32;

/** 左右滑动胶囊选择器：滑块在长椭圆轨道内滑动到选中段（对齐侧栏明暗开关的视觉）。 */
const SegWrap = styled.div`
  position: relative;
  display: inline-flex;
  flex: none;
  width: fit-content;
  height: ${SEG_H}px;
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-border);
  border-radius: ${SEG_H / 2}px;
  padding: 2px;
`;

const SegSliderKnob = styled.div<{ $index: number; $accent: string }>`
  position: absolute;
  top: 2px;
  left: 2px;
  width: ${SEG_W}px;
  height: ${SEG_H - 6}px;
  background: ${({ $accent }) => $accent};
  border-radius: ${(SEG_H - 6) / 2}px;
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
  border-radius: ${(SEG_H - 6) / 2}px;
  background: transparent;
  color: ${({ $selected }) => ($selected ? '#FAFAFA' : 'var(--bvt-text2)')};
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
  /** 选项标签数组，index 即位置。 */
  options: string[];
  /** 当前选中 index。 */
  value: number;
  /** 滑块颜色（accent）。 */
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
