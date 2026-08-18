/**
 * 底部状态栏：显示全局任务状态/提示消息（对齐原版 set_status）。
 */

import styled from 'styled-components';
import { useUiStore } from '../store/uiStore';
import { useUpdateStore } from '../store/updateStore';

const Bar = styled.div`
  height: 26px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  background: var(--bvt-surface2);
  border-top: 1px solid var(--bvt-glass-border);
  box-shadow: var(--bvt-glass-highlight), inset 0 1px 0 color-mix(in srgb, var(--bvt-accent) 16%, transparent);
  color: var(--bvt-text3);
  font-size: 11px;
  letter-spacing: 0.1em;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  width: 100%;
`;

const Hint = styled.button`
  margin-left: auto;
  border: none;
  background: transparent;
  color: var(--bvt-accent);
  font: inherit;
  letter-spacing: 0.08em;
  cursor: pointer;
  flex: none;
`;

export function StatusBar() {
  const status = useUiStore((s) => s.status);
  const goTo = useUiStore((s) => s.goTo);
  const info = useUpdateStore((s) => s.info);
  return (
    <Bar>
      <span>{status || '\u00A0'}</span>
      {info?.has_update && (
        <Hint type="button" onClick={() => goTo('settings')}>
          {info.remote_version} 可更新
        </Hint>
      )}
    </Bar>
  );
}
