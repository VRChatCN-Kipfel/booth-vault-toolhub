/**
 * 底部状态栏：全局任务状态 + 进行中任务可取消。
 */

import { useState } from 'react';
import styled from 'styled-components';
import { useUiStore } from '../store/uiStore';
import { useUpdateStore } from '../store/updateStore';
import { failedItems, runningCount, useTaskStore } from '../store/taskStore';
import { cancelTask, retryFailed } from '../lib/task';

const Bar = styled.div`
  position: relative;
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
  width: 100%;
`;

const StatusText = styled.span`
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  min-width: 0;
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

const TaskHint = styled.button`
  margin-left: 12px;
  border: none;
  background: transparent;
  color: var(--bvt-accent);
  font: inherit;
  letter-spacing: 0.08em;
  cursor: pointer;
  flex: none;
`;

const Pop = styled.div`
  position: absolute;
  right: 12px;
  bottom: 28px;
  min-width: 280px;
  max-width: 420px;
  background: var(--bvt-surface);
  border: 1px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  box-shadow: var(--bvt-glass-highlight);
  padding: 8px 10px;
  z-index: 20;
`;

const PopRow = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  color: var(--bvt-text);
  font-size: 12px;
  letter-spacing: 0;
  .lab { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--bvt-text2); flex: none; }
`;

const PopBtn = styled.button`
  border: none;
  background: transparent;
  color: var(--bvt-accent);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  flex: none;
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
        <TaskHint type="button" onClick={() => setOpen((v) => !v)}>
          {n} 个任务进行中
        </TaskHint>
      )}
      {n === 0 && list.length > 0 && (
        <TaskHint type="button" onClick={() => setOpen((v) => !v)}>
          {list.length} 个任务可重试
        </TaskHint>
      )}
      {info?.has_update && (
        <Hint type="button" onClick={() => goTo('settings')}>
          {info.remote_version} 可更新
        </Hint>
      )}
      {open && (
        <Pop>
          {list.length === 0 && <PopRow>没有进行中的任务</PopRow>}
          {list.map(([id, t]) => (
            <PopRow key={id}>
              <span className="lab">{t.label}</span>
              <span className="meta">{t.done}/{t.total || '?'}</span>
              {t.status === 'running' && (
                <PopBtn type="button" onClick={() => void cancelTask(id)}>取消</PopBtn>
              )}
              {t.status === 'done' && failedItems(t).length > 0 && (
                <PopBtn type="button" onClick={() => void retryFailed(id)}>重试</PopBtn>
              )}
            </PopRow>
          ))}
        </Pop>
      )}
    </Bar>
  );
}
