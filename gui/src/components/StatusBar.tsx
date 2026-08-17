/**
 * 底部状态栏：显示全局任务状态/提示消息（对齐原版 set_status）。
 */

import styled from 'styled-components';
import { useUiStore } from '../store/uiStore';

const Bar = styled.div`
  height: 26px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  background: var(--bvt-surface2);
  border-top: 1px solid var(--bvt-border);
  box-shadow: inset 0 1px 0 color-mix(in srgb, var(--bvt-accent) 16%, transparent);
  color: var(--bvt-text3);
  font-size: 11px;
  letter-spacing: 0.1em;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
`;

export function StatusBar() {
  const status = useUiStore((s) => s.status);
  return <Bar>{status || '\u00A0'}</Bar>;
}
