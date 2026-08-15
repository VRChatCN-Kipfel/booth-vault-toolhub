/**
 * 动画速度刻度尺：固定档位（0.25x - 4x，非线性），拖动即吸附生效。
 * 刻度/标签按档位集合映射到线性索引，天然对齐。
 */

import styled from 'styled-components';
import {
  useThemeStore, ANIM_STOPS, stopIndex,
} from '../store/themeStore';

const Wrap = styled.div`
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 320px;
  max-width: 100%;
`;

const Head = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: baseline;
`;

const Label = styled.span`
  color: var(--bvt-text2);
  font-size: 13px;
`;

const Value = styled.span`
  color: var(--bvt-accent);
  font-size: 14px;
  font-weight: 600;
`;

const Track = styled.div`
  position: relative;
  height: 24px;
  display: flex;
  align-items: center;
`;

/** 原生 range 底轨（视觉：细线 + 刻度点）。 */
const RangeInput = styled.input`
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
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--bvt-accent);
    margin-top: -7px;
    border: none;
    box-shadow: 0 0 0 3px var(--bvt-surface);
  }
  &:focus { outline: none; }
`;

/** thumb 半径（对齐 range 可达范围内缩）。 */
const THUMB_R = 8;

const Ticks = styled.div`
  position: absolute;
  inset: 0;
  z-index: 1;
  pointer-events: none;
  left: ${THUMB_R}px;
  right: ${THUMB_R}px;
`;

const TickWrap = styled.div<{ $idx: number }>`
  position: absolute;
  top: 0;
  bottom: 0;
  left: ${({ $idx }) => `${($idx / (ANIM_STOPS.length - 1)) * 100}%`};
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
`;

const TickLine = styled.div`
  width: 1px;
  height: 8px;
  background: var(--bvt-text3);
  opacity: 0.5;
`;

const TicksLabel = styled.div`
  position: relative;
  height: 14px;
  margin-top: 2px;
  color: var(--bvt-text3);
  font-size: 11px;
  left: ${THUMB_R}px;
  right: ${THUMB_R}px;
`;

const TickLabel = styled.span<{ $idx: number }>`
  position: absolute;
  transform: translateX(-50%);
  left: ${({ $idx }) => `${($idx / (ANIM_STOPS.length - 1)) * 100}%`};
`;

/** 只显示主要标签（避免过密）。 */
const LABEL_IDX = [0, 3, 6, 8];

export function SpeedSlider() {
  const { animSpeed, setAnimSpeed } = useThemeStore();
  const shownIdx = stopIndex(animSpeed);
  const shown = ANIM_STOPS[shownIdx];

  return (
    <Wrap>
      <Head>
        <Label>动画速度</Label>
        <Value>{shown}x</Value>
      </Head>
      <Track>
        <Ticks>
          {ANIM_STOPS.map((v, i) => (
            <TickWrap key={v} $idx={i}>
              <TickLine />
            </TickWrap>
          ))}
        </Ticks>
        <RangeInput
          type="range"
          min={0}
          max={ANIM_STOPS.length - 1}
          step={1}
          value={shownIdx}
          // step=1 索引 → 值天然落在刻度上，拖动即生效并持久化（无需松手吸附）。
          onChange={(e) => {
            const idx = parseInt(e.target.value, 10);
            const v = ANIM_STOPS[idx] ?? ANIM_STOPS[0];
            setAnimSpeed(v);
          }}
        />
      </Track>
      <TicksLabel>
        {LABEL_IDX.map((i) => (
          <TickLabel key={i} $idx={i}>
            {ANIM_STOPS[i]}x
          </TickLabel>
        ))}
      </TicksLabel>
    </Wrap>
  );
}
