/**
 * 底部状态栏：显示全局任务状态/提示消息（对齐原版 set_status）。
 */

import styled from 'styled-components';
import { useUiStore } from '../store/uiStore';

const Bar = styled.div`
  height: 26px;
  display: flex;
  align-items: center;
  padding: 0 12px;
  background: var(--bvt-surface2);
  border-top: 1px solid var(--bvt-border2);
  color: var(--bvt-text2);
  font-size: 12px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
`;

export function StatusBar() {
  const status = useUiStore((s) => s.status);
  return <Bar>{status || '\u00A0'}</Bar>;
}
