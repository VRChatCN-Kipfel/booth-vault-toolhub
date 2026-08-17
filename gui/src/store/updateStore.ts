/**
 * 工具自更新：启动静默查一次，设置页可再查。
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export type UpdateInfo = {
  has_update: boolean;
  local_version: string;
  remote_version: string;
  url: string;
  error?: string | null;
  release_title?: string | null;
  release_body?: string | null;
};

interface UpdateState {
  checking: boolean;
  info: UpdateInfo | null;
  check: (useProxy: boolean) => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  checking: false,
  info: null,
  check: async (useProxy) => {
    if (get().checking) return;
    set({ checking: true });
    try {
      const info = await invoke<UpdateInfo>('update_check', { useProxy });
      set({ info, checking: false });
    } catch (e) {
      set({
        checking: false,
        info: {
          has_update: false,
          local_version: get().info?.local_version ?? '',
          remote_version: '',
          url: '',
          error: String(e),
        },
      });
    }
  },
}));
