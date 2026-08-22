/**
 * 长任务：Channel 写入模块级 zustand，页面只订阅。
 */

import { Channel, invoke } from '@tauri-apps/api/core';
import {
  useTaskStore,
  type ProgressEvt,
  type TaskKind,
  type TaskRecord,
  failedItems,
} from '../store/taskStore';
import { useAppConfigStore } from '../store/appConfigStore';

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

function asStringList(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String) : [];
}

function forceIdForRetry(file: string, task: TaskRecord): string {
  const origFiles = asStringList(task.args.files);
  const origForce = asStringList(task.args.forceIds);
  const idx = origFiles.indexOf(file);
  if (idx >= 0 && origForce[idx]) return origForce[idx];
  const previewId = useTaskStore.getState().latestByKind.search;
  const preview = previewId ? useTaskStore.getState().tasks[previewId] : undefined;
  return preview?.items.find((i) => i.source === file && i.picked)?.picked ?? '';
}

export async function retryFailed(taskId: string): Promise<string | null> {
  const task = useTaskStore.getState().tasks[taskId];
  if (!task) return null;
  const failed = failedItems(task);
  if (failed.length === 0) return null;
  const ids = failed.map((i) => i.path || i.source || i.id).filter(Boolean);
  if (ids.length === 0) return null;
  const args = { ...task.args };
  args.cookie = useAppConfigStore.getState().cookie || null;
  if (task.kind === 'download' || task.cmd === 'download') {
    const shopErrs = failed.filter((i) => i.message.startsWith('店铺翻页失败'));
    const itemErrs = failed.filter((i) => !i.message.startsWith('店铺翻页失败'));
    const prevShop = typeof task.args.shop === 'string' ? task.args.shop : '';
    const retryItems = itemErrs.map((i) => i.id).filter(Boolean);
    const retryShop = shopErrs.length ? prevShop || shopErrs[0].id : null;
    if (retryItems.length === 0 && !retryShop) return null;
    args.items = retryItems;
    args.shop = retryShop;
  } else if (task.kind === 'organize' || task.cmd === 'organize') {
    args.archives = ids;
  } else if (task.cmd === 'search') {
    args.files = ids;
    args.dryRun = false;
    args.forceIds = ids.map((file) => forceIdForRetry(file, task));
  } else {
    return null;
  }
  return runTask(task.cmd, args, { kind: task.kind, label: `${task.label} · 重试` });
}
