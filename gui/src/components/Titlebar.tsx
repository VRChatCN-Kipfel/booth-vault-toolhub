/**
 * 无边框窗口标题栏：data-tauri-drag-region 拖拽 + 最小化/最大化/关闭。
 */

import styled from 'styled-components';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { FONTS } from '../theme/themes';
import { useUpdateStore } from '../store/updateStore';
import { useUiStore } from '../store/uiStore';

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

const Meta = styled.button`
  -webkit-app-region: no-drag;
  margin-right: 8px;
  border: none;
  background: transparent;
  color: var(--bvt-text3);
  font-family: ${FONTS.serif};
  font-size: 11px;
  letter-spacing: 0.08em;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  &:hover { color: var(--bvt-accent); }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--bvt-accent);
  }
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
  const info = useUpdateStore((s) => s.info);
  const goTo = useUiStore((s) => s.goTo);
  const ver = info?.local_version;
  return (
    <Bar data-tauri-drag-region style={isMac ? { paddingLeft: 78 } : undefined}>
      <Title data-tauri-drag-region>Booth Vault Toolhub</Title>
      <div style={{ display: 'flex', alignItems: 'center' }}>
        {ver && (
          <Meta type="button" onClick={() => goTo('settings')} title="软件更新">
            {info?.has_update && <span className="dot" />}
            {info?.has_update ? `有新版本 ${info.remote_version}` : ver}
          </Meta>
        )}
        {!isMac && <WinControls />}
      </div>
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
