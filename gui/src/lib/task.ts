/**
 * 长任务：Channel 写入模块级 zustand，页面只订阅。
 */

import { Channel, invoke } from '@tauri-apps/api/core';
import {
  useTaskStore,
  type ProgressEvt,
  type TaskKind,
  failedItems,
} from '../store/taskStore';

export type { ProgressEvt, TaskKind };

const KIND_LABEL: Partial<Record<TaskKind, string>> = {
  download: '批量下载',
  organize: '拖拽归档',
  search: '实验检索',
  search_archive: '检索归档',
  audit: '目录巡检',
  audit_fix: '修复三件套',
  version_audit: '版本巡检',
  mismatch_audit: '错位检测',
  fix_mismatch: '错位纠正',
};

export async function runTask(
  cmd: string,
  args: Record<string, unknown>,
  opts?: { kind?: TaskKind; label?: string },
): Promise<string> {
  const kind = opts?.kind ?? (cmd as TaskKind);
  const label = opts?.label ?? KIND_LABEL[kind] ?? cmd;
  let taskId = '';
  const pending: ProgressEvt[] = [];
  const onEventCh = new Channel<ProgressEvt>();
  onEventCh.onmessage = (evt) => {
    if (!taskId) pending.push(evt);
    else useTaskStore.getState().applyEvent(taskId, evt);
  };
  const got = await invoke<string>(cmd, { ...args, onEvent: onEventCh });
  useTaskStore.getState().begin(got, { kind, label, cmd, args });
  taskId = got;
  for (const evt of pending) {
    useTaskStore.getState().applyEvent(taskId, evt);
  }
  return taskId;
}

export async function cancelTask(taskId: string): Promise<void> {
  await invoke('cancel_task', { taskId });
}

export async function retryFailed(taskId: string): Promise<string | null> {
  const task = useTaskStore.getState().tasks[taskId];
  if (!task) return null;
  const failed = failedItems(task);
  if (failed.length === 0) return null;
  const ids = failed.map((i) => i.path || i.source || i.id).filter(Boolean);
  if (ids.length === 0) return null;
  const args = { ...task.args };
  if (task.kind === 'download' || task.cmd === 'download') {
    args.items = failed.map((i) => i.id);
    args.shop = null;
  } else if (task.kind === 'organize' || task.cmd === 'organize') {
    args.archives = ids;
  } else if (task.cmd === 'search') {
    args.files = ids;
    args.dryRun = false;
  } else {
    return null;
  }
  return runTask(task.cmd, args, { kind: task.kind, label: `${task.label} · 重试` });
}
