/**
 * 无边框窗口标题栏：data-tauri-drag-region 拖拽 + 最小化/最大化/关闭。
 */

import styled from 'styled-components';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { FONTS } from '../theme/themes';

const Bar = styled.div`
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bvt-surface2);
  border-bottom: 1px solid var(--bvt-border2);
  box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--bvt-accent) 22%, transparent);
  -webkit-app-region: drag;
  user-select: none;
`;

const Title = styled.div`
  padding-left: 14px;
  font-size: 11px;
  letter-spacing: 0.18em;
  color: var(--bvt-text3);
  font-family: ${FONTS.serif};
`;

const Controls = styled.div`
  display: flex;
  -webkit-app-region: no-drag;
`;

const CtrlBtn = styled.button`
  width: 42px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--bvt-text2);
  cursor: pointer;
  &:hover { background: var(--bvt-hover); color: var(--bvt-text); }
  &.close:hover { background: var(--bvt-danger); color: #fff; }
  svg { width: 14px; height: 14px; }
`;

const isMac =
  typeof navigator !== 'undefined' && /Mac|Macintosh/.test(navigator.userAgent);

export function Titlebar() {
  return (
    <Bar data-tauri-drag-region style={isMac ? { paddingLeft: 78 } : undefined}>
      <Title data-tauri-drag-region>Booth Vault Toolhub</Title>
      {!isMac && <WinControls />}
    </Bar>
  );
}

function WinControls() {
  const win = getCurrentWindow();
  return (
    <Controls>
      <CtrlBtn onClick={() => void win.minimize()} title="最小化">
        <Minus />
      </CtrlBtn>
      <CtrlBtn onClick={() => void win.toggleMaximize()} title="最大化">
        <Square />
      </CtrlBtn>
      <CtrlBtn className="close" onClick={() => void win.close()} title="关闭">
        <X />
      </CtrlBtn>
    </Controls>
  );
}
