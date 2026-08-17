/**
 * 长任务：先挂 Channel 再 invoke，立刻拿到 task_id。
 */

import { Channel, invoke } from '@tauri-apps/api/core';

export type ProgressEvt = {
  type: string;
  total?: number;
  done?: number;
  failed?: number;
  id?: string;
  message?: string;
  status?: string;
  line?: string;
};

export async function runTask(
  cmd: string,
  args: Record<string, unknown>,
  onEvent: (evt: ProgressEvt) => void,
): Promise<string> {
  const onEventCh = new Channel<ProgressEvt>();
  onEventCh.onmessage = onEvent;
  return invoke<string>(cmd, { ...args, onEvent: onEventCh });
}
