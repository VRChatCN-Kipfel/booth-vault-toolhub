/**
 * 全局 UI 状态：状态栏消息、任务进度通知。
 * 对齐原版 main.set_status（底部状态栏消息）。
 */

import { create } from 'zustand';

interface UiState {
  /** 状态栏消息。 */
  status: string;
  /** 任务中状态（如"巡检中…"）。 */
  busy: boolean;
  setStatus: (msg: string) => void;
  setBusy: (b: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  status: '',
  busy: false,
  setStatus: (msg) => set({ status: msg }),
  setBusy: (b) => set({ busy: b }),
}));
