/**
 * 底部状态栏：全局任务状态 + 进行中任务可取消。
 */

import { useState } from 'react';
import styled from 'styled-components';
import { useUiStore } from '../store/uiStore';
import { useUpdateStore } from '../store/updateStore';
import { failedItems, runningCount, useTaskStore } from '../store/taskStore';
import { cancelTask, retryFailed } from '../lib/task';
import { TextButton } from './ui';

const Bar = styled.footer`
  position: relative;
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--bvt-s4);
  width: 100%;
  height: 28px;
  padding: 0 var(--bvt-s4);
  background: var(--bvt-rail-bg);
  border-top: 1px solid var(--bvt-border);
  color: var(--bvt-text3);
  font-size: var(--bvt-fz-xs);
`;

const StatusText = styled.span`
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
`;

const Pop = styled.div`
  position: absolute;
  right: var(--bvt-s3);
  bottom: 32px;
  z-index: 20;
  min-width: 300px;
  max-width: 440px;
  padding: var(--bvt-s2);
  background: var(--bvt-surface);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius);
  box-shadow: var(--bvt-shadow-2);
`;

const PopRow = styled.div`
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  padding: var(--bvt-s1) var(--bvt-s2);
  color: var(--bvt-text);
  font-size: var(--bvt-fz-sm);
  .lab { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { flex: none; color: var(--bvt-text2); font-variant-numeric: tabular-nums; }
`;

export function StatusBar() {
  const status = useUiStore((s) => s.status);
  const goTo = useUiStore((s) => s.goTo);
  const info = useUpdateStore((s) => s.info);
  const tasks = useTaskStore((s) => s.tasks);
  const [open, setOpen] = useState(false);
  const n = runningCount(tasks);
  const list = Object.entries(tasks)
    .filter(([, t]) => t.status === 'running' || (t.status === 'done' && failedItems(t).length > 0))
    .sort((a, b) => b[1].startedAt - a[1].startedAt)
    .slice(0, 8);

  return (
    <Bar>
      <StatusText>{status || '\u00A0'}</StatusText>
      {n > 0 && (
        <TextButton type="button" onClick={() => setOpen((v) => !v)}>
          {n} 个任务进行中
        </TextButton>
      )}
      {n === 0 && list.length > 0 && (
        <TextButton type="button" onClick={() => setOpen((v) => !v)}>
          {list.length} 个任务可重试
        </TextButton>
      )}
      {info?.has_update && (
        <TextButton type="button" onClick={() => goTo('settings')}>
          {info.remote_version} 可更新
        </TextButton>
      )}
      {open && (
        <Pop>
          {list.length === 0 && <PopRow>没有进行中的任务</PopRow>}
          {list.map(([id, t]) => (
            <PopRow key={id}>
              <span className="lab">{t.label}</span>
              <span className="meta">{t.done}/{t.total || '?'}</span>
              {t.status === 'running' && (
                <TextButton type="button" onClick={() => void cancelTask(id)}>取消</TextButton>
              )}
              {t.status === 'done' && failedItems(t).length > 0 && (
                <TextButton type="button" onClick={() => void retryFailed(id)}>重试</TextButton>
              )}
            </PopRow>
          ))}
        </Pop>
      )}
    </Bar>
  );
}
