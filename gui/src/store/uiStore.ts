/**
 * 全局 UI 状态：状态栏消息、任务进度通知。
 * 对齐原版 main.set_status（底部状态栏消息）。
 */

import { create } from 'zustand';

interface UiState {
  status: string;
  busy: boolean;
  pendingPage: string | null;
  setStatus: (msg: string) => void;
  setBusy: (b: boolean) => void;
  goTo: (page: string) => void;
  consumePage: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  status: '',
  busy: false,
  pendingPage: null,
  setStatus: (msg) => set({ status: msg }),
  setBusy: (b) => set({ busy: b }),
  goTo: (page) => set({ pendingPage: page }),
  consumePage: () => set({ pendingPage: null }),
}));
