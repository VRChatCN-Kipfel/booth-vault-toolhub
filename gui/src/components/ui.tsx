/**
 * 控件套件：东方装帧语汇的最小实现。
 *
 * 结构靠 1px 界线和留白，不靠阴影堆叠；强调色只出现在当前态和主按钮上。
 * 所有尺寸、圆角、颜色一律取 global.ts 的令牌，组件内不写死。
 */

import styled, { css } from 'styled-components';
import type { KeyboardEvent, ReactNode } from 'react';

/* ── 按钮 ──────────────────────────────────────────────── */

const btnBase = css`
  height: var(--bvt-h-ctl);
  padding: 0 var(--bvt-s4);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--bvt-s2);
  white-space: nowrap;
  font-size: var(--bvt-fz-md);
  border-radius: var(--bvt-radius);
  cursor: pointer;
  transition: background-color 0.16s var(--bvt-ease), border-color 0.16s var(--bvt-ease),
    color 0.16s var(--bvt-ease);
  &:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  svg {
    width: 14px;
    height: 14px;
  }
`;

export const AccentButton = styled.button`
  ${btnBase}
  background: var(--bvt-btn-fill);
  color: var(--bvt-on-btn);
  border: 1px solid transparent;
  font-weight: 500;
  &:hover:not(:disabled) { background: var(--bvt-btn-fill-hover); }
  &:active:not(:disabled) { background: var(--bvt-btn-fill-press); }
`;

export const SecondaryButton = styled.button`
  ${btnBase}
  background: transparent;
  color: var(--bvt-text);
  border: 1px solid var(--bvt-border);
  &:hover:not(:disabled) {
    background: var(--bvt-hover);
    border-color: color-mix(in srgb, var(--bvt-text3) 60%, var(--bvt-border));
  }
  &:active:not(:disabled) { background: var(--bvt-surface2); }
`;

export const GhostButton = styled.button`
  ${btnBase}
  background: transparent;
  color: var(--bvt-text2);
  border: 1px solid transparent;
  padding: 0 var(--bvt-s3);
  &:hover:not(:disabled) { background: var(--bvt-hover); color: var(--bvt-text); }
`;

/** 行内文字按钮：列表右侧的「复制/打开/重试」这类动作。 */
export const TextButton = styled.button`
  background: none;
  border: none;
  padding: 0 2px;
  font: inherit;
  font-size: var(--bvt-fz-sm);
  color: var(--bvt-accent);
  cursor: pointer;
  &:hover:not(:disabled) { text-decoration: underline; }
  &:disabled { opacity: 0.4; cursor: not-allowed; }
`;

/* ── 输入 ──────────────────────────────────────────────── */

const fieldBase = css`
  width: 100%;
  background: var(--bvt-input-bg);
  color: var(--bvt-text);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius);
  padding: 0 var(--bvt-s3);
  font-size: var(--bvt-fz-md);
  outline: none;
  transition: border-color 0.16s var(--bvt-ease), box-shadow 0.16s var(--bvt-ease);
  &::placeholder { color: var(--bvt-text3); }
  &:hover:not(:disabled) { border-color: color-mix(in srgb, var(--bvt-text3) 55%, var(--bvt-border)); }
  &:focus {
    border-color: var(--bvt-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--bvt-accent) 14%, transparent);
  }
  &:focus-visible { outline: none; }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
`;

export const Input = styled.input`
  ${fieldBase}
  height: var(--bvt-h-ctl);
`;

export const TextArea = styled.textarea`
  ${fieldBase}
  padding: var(--bvt-s2) var(--bvt-s3);
  resize: none;
  line-height: 1.7;
  font-family: var(--bvt-mono);
  font-size: var(--bvt-fz-sm);
`;

export const Checkbox = styled.input.attrs({ type: 'checkbox' })`
  width: 15px;
  height: 15px;
  flex: none;
  accent-color: var(--bvt-accent);
  cursor: pointer;
  &:disabled { cursor: not-allowed; }
`;

/** 复选框 + 文字，整体可点。 */
export const CheckLabel = styled.label`
  display: inline-flex;
  align-items: center;
  gap: var(--bvt-s2);
  color: var(--bvt-text2);
  font-size: var(--bvt-fz-md);
  cursor: pointer;
  &:has(input:disabled) { opacity: 0.5; cursor: not-allowed; }
`;

/* ── 布局 ──────────────────────────────────────────────── */

export const PageShell = styled.div`
  height: 100%;
  overflow-y: auto;
  padding: var(--bvt-s6) var(--bvt-s6) var(--bvt-s5);
  display: flex;
  flex-direction: column;
  gap: var(--bvt-s5);
  @media (max-width: 900px) {
    padding: var(--bvt-s5) var(--bvt-s4) var(--bvt-s4);
    gap: var(--bvt-s4);
  }
`;

/**
 * 页内小节：细标题 + 内容，节与节之间靠留白分开，不加分割线。
 *
 * flex: none 是必须的：PageShell 是 flex 列容器，默认会压缩子项，
 * 一旦允许收缩，小节就会被压扁、内容互相重叠。
 */
export const Section = styled.section`
  flex: none;
  display: flex;
  flex-direction: column;
  gap: var(--bvt-s3);
`;

/** 撑满剩余高度的小节（放列表用）：只有它允许收缩，好让内部列表自己滚。 */
export const FlexSection = styled(Section)`
  flex: 1 1 auto;
  min-height: 0;
`;

export const Row = styled.div`
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  flex-wrap: wrap;
`;

/** 小节标题：小字、字距略开、旁边跟一道细线。 */
const LabelText = styled.span`
  flex: none;
  font-size: var(--bvt-fz-xs);
  font-weight: 600;
  letter-spacing: 0.14em;
  color: var(--bvt-text3);
`;

const LabelRule = styled.span`
  flex: 1;
  height: 1px;
  background: var(--bvt-border2);
`;

const LabelWrap = styled.div`
  display: flex;
  align-items: center;
  gap: var(--bvt-s3);
`;

export function PanelLabel({ children, extra }: { children: ReactNode; extra?: ReactNode }) {
  return (
    <LabelWrap>
      <LabelText>{children}</LabelText>
      <LabelRule />
      {extra}
    </LabelWrap>
  );
}

export const Lead = styled.p`
  color: var(--bvt-text2);
  font-size: var(--bvt-fz-sm);
  line-height: 1.75;
  max-width: 68ch;
`;

export const Muted = styled.span`
  color: var(--bvt-text3);
  font-size: var(--bvt-fz-sm);
`;

export const Card = styled.div`
  background: var(--bvt-surface);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius);
  padding: var(--bvt-s4);
`;

/* ── 列表 ──────────────────────────────────────────────── */

/** 列表容器：外框一道界线，行与行之间用更淡的线。 */
export const ObsPanel = styled.div`
  background: var(--bvt-surface);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius);
  overflow-y: auto;
  min-height: 0;
`;

export const ListRow = styled.div<{ $selected?: boolean }>`
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  min-height: var(--bvt-h-row);
  padding: var(--bvt-s1) var(--bvt-s3);
  font-size: var(--bvt-fz-md);
  color: var(--bvt-text);
  border-bottom: 1px solid var(--bvt-border2);
  background: ${({ $selected }) => ($selected ? 'var(--bvt-sel-bg)' : 'transparent')};
  ${({ $selected }) => $selected && 'color: var(--bvt-sel-text);'}
  &:last-child { border-bottom: none; }
  /* 悬停用 inset 染色而非改底色，才不会盖掉选中行的底 */
  &:hover { box-shadow: inset 0 0 0 100vmax color-mix(in srgb, var(--bvt-text) 4%, transparent); }
  .grow {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg {
    flex: 1;
    min-width: 0;
    color: var(--bvt-text2);
    font-size: var(--bvt-fz-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
`;

/** 空态：居中、留白、说人话。 */
const EmptyWrap = styled.div`
  height: 100%;
  min-height: 120px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--bvt-s2);
  padding: var(--bvt-s6) var(--bvt-s4);
  text-align: center;
  color: var(--bvt-text3);
  .mark {
    width: 22px;
    height: 22px;
    border: 1px solid currentColor;
    border-radius: var(--bvt-radius-sm);
    opacity: 0.5;
  }
  .sub { font-size: var(--bvt-fz-sm); }
`;

export function EmptyState({ title, hint }: { title: string; hint?: string }) {
  return (
    <EmptyWrap>
      <span className="mark" aria-hidden />
      <span>{title}</span>
      {hint && <span className="sub">{hint}</span>}
    </EmptyWrap>
  );
}

/* ── 状态 ──────────────────────────────────────────────── */

export const Badge = styled.span<{ kind: 'ok' | 'run' | 'wait' | 'warn' | 'err' }>`
  flex: none;
  display: inline-flex;
  align-items: center;
  height: 19px;
  padding: 0 var(--bvt-s2);
  border-radius: var(--bvt-pill);
  font-size: var(--bvt-fz-xs);
  font-variant-numeric: tabular-nums;
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

export const ProgressBar = styled.div`
  height: 3px;
  background: var(--bvt-border2);
  border-radius: 999px;
  overflow: hidden;
  & > div {
    height: 100%;
    background: var(--bvt-accent);
    border-radius: inherit;
    transition: width 0.3s var(--bvt-ease);
  }
`;

/** 计数：done/total，等宽数字防跳动。 */
export const Counter = styled.span`
  flex: none;
  font-variant-numeric: tabular-nums;
  font-size: var(--bvt-fz-sm);
  color: var(--bvt-text2);
`;

/** 滑条：细底轨 + 朱点滑块。thumb 半径 8px，刻度对齐时按此内缩。 */
export const RangeInput = styled.input.attrs({ type: 'range' })`
  width: 100%;
  appearance: none;
  -webkit-appearance: none;
  height: 24px;
  background: transparent;
  cursor: pointer;
  position: relative;
  z-index: 2;

  &::-webkit-slider-runnable-track {
    height: 2px;
    background: var(--bvt-border);
    border-radius: 1px;
  }
  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--bvt-accent);
    margin-top: -6px;
    border: none;
    box-shadow: 0 0 0 3px var(--bvt-surface);
  }
  &:focus { outline: none; }
`;

/* ── 分段控件 ──────────────────────────────────────────── */

const SEG_W = 64;
const SEG_H = 30;

const SegWrap = styled.div`
  position: relative;
  display: inline-flex;
  flex: none;
  height: ${SEG_H}px;
  padding: 2px;
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-pill);
`;

const SegKnob = styled.div<{ $index: number; $accent: string }>`
  position: absolute;
  top: 2px;
  left: 2px;
  width: ${SEG_W}px;
  height: ${SEG_H - 6}px;
  background: ${({ $accent }) => $accent};
  border-radius: var(--bvt-pill);
  transform: translateX(${({ $index }) => $index * SEG_W}px);
  transition: transform calc(0.3s / var(--bvt-anim)) var(--bvt-ease),
    background-color calc(0.35s / var(--bvt-anim));
`;

const SegItem = styled.button<{ $selected: boolean }>`
  position: relative;
  z-index: 1;
  width: ${SEG_W}px;
  height: ${SEG_H - 6}px;
  border: none;
  background: transparent;
  border-radius: var(--bvt-pill);
  font-size: var(--bvt-fz-md);
  cursor: pointer;
  color: ${({ $selected }) => ($selected ? 'var(--bvt-on-accent)' : 'var(--bvt-text2)')};
  font-weight: ${({ $selected }) => ($selected ? 600 : 400)};
  transition: color 0.25s var(--bvt-ease);
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
      <SegKnob $index={value} $accent={accent} />
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
