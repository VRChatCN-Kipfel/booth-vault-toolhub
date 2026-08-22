/**
 * 无边框窗口标题栏。
 *
 * 三端形态不同：macOS 让位给原生交通灯、背景透明（窗体后面是原生材质）；
 * Windows/Linux 自绘最小化/最大化/关闭。拖拽区靠 data-tauri-drag-region。
 */

import styled from 'styled-components';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useUpdateStore } from '../store/updateStore';
import { useUiStore } from '../store/uiStore';

const H = 36;

const Bar = styled.div`
  height: ${H}px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bvt-rail-bg);
  border-bottom: 1px solid var(--bvt-border);
  -webkit-app-region: drag;
  user-select: none;
  html[data-platform='mac'] & {
    /* 交通灯占位由系统给出，拿不到时退回一组保守值 */
    padding-left: calc(env(titlebar-area-x, 16px) + env(titlebar-area-width, 62px) + 10px);
    height: max(${H}px, env(titlebar-area-height, ${H}px));
    background: transparent;
    border-bottom-color: transparent;
  }
`;

const Title = styled.div`
  padding-left: var(--bvt-s4);
  font-family: var(--bvt-serif);
  font-size: var(--bvt-fz-xs);
  letter-spacing: 0.18em;
  color: var(--bvt-text3);
  html[data-platform='mac'] & { padding-left: 0; }
`;

const Right = styled.div`
  display: flex;
  align-items: center;
`;

const Meta = styled.button`
  -webkit-app-region: no-drag;
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  height: 22px;
  margin-right: var(--bvt-s2);
  padding: 0 var(--bvt-s2);
  background: transparent;
  border: none;
  border-radius: var(--bvt-radius);
  color: var(--bvt-text3);
  font-size: var(--bvt-fz-xs);
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  &:hover { color: var(--bvt-text); background: var(--bvt-hover); }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--bvt-accent);
  }
`;

const Controls = styled.div`
  display: flex;
  -webkit-app-region: no-drag;
`;

const CtrlBtn = styled.button`
  width: 44px;
  height: ${H}px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--bvt-text2);
  cursor: pointer;
  &:hover { background: var(--bvt-hover); color: var(--bvt-text); }
  &.close:hover { background: var(--bvt-danger); color: #fff; }
  svg { width: 13px; height: 13px; }
`;

const isMac = typeof navigator !== 'undefined' && /Mac|Macintosh/.test(navigator.userAgent);

export function Titlebar() {
  const info = useUpdateStore((s) => s.info);
  const goTo = useUiStore((s) => s.goTo);
  const ver = info?.local_version;
  return (
    <Bar data-tauri-drag-region>
      <Title data-tauri-drag-region>展位库 · BOOTH VAULT</Title>
      <Right>
        {ver && (
          <Meta type="button" onClick={() => goTo('settings')} title="软件更新">
            {info?.has_update && <span className="dot" />}
            {info?.has_update ? `有新版本 ${info.remote_version}` : ver}
          </Meta>
        )}
        {!isMac && <WinControls />}
      </Right>
    </Bar>
  );
}

function WinControls() {
  const win = getCurrentWindow();
  return (
    <Controls>
      <CtrlBtn type="button" onClick={() => void win.minimize()} title="最小化" aria-label="最小化">
        <Minus />
      </CtrlBtn>
      <CtrlBtn type="button" onClick={() => void win.toggleMaximize()} title="最大化" aria-label="最大化">
        <Square />
      </CtrlBtn>
      <CtrlBtn className="close" type="button" onClick={() => void win.close()} title="关闭" aria-label="关闭">
        <X />
      </CtrlBtn>
    </Controls>
  );
}
