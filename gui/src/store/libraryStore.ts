/**
 * 库存列表缓存。
 *
 * 「每次切到库存页都重新扫一遍」是纯粹的感知浪费：引擎侧已把扫描降到毫秒级，
 * 但组件挂载即发请求仍会让界面闪一次「正在扫描」。这里做 stale-while-revalidate：
 * 有缓存就先渲染缓存（零等待），同时后台静默刷新，回来后替换。
 *
 * 换归档根目录时按 `loadedFor` 识别并清空——否则旧库的条目会短暂串到新库上。
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface LibraryRow {
  id: string;
  name: string;
  category: string;
  path: string;
}

interface LibraryState {
  items: LibraryRow[];
  /** 当前 items 属于哪个归档根目录。 */
  loadedFor: string;
  /** 仅在「无缓存可显示」或「用户显式刷新」时为真，避免后台刷新把界面打回加载态。 */
  loading: boolean;
  error: string;
  load: (root: string, force?: boolean) => Promise<void>;
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  items: [],
  loadedFor: '',
  loading: false,
  error: '',

  load: async (root, force = false) => {
    if (!root) {
      set({ items: [], loadedFor: '', error: '', loading: false });
      return;
    }
    const switched = get().loadedFor !== root;
    if (switched) {
      set({ items: [], loadedFor: root, error: '' });
    }
    const hasCache = !switched && get().items.length > 0;
    if (!hasCache || force) {
      set({ loading: true });
    }
    try {
      const rows = await invoke<LibraryRow[]>('list_library', { base: root, force });
      set({ items: rows, loadedFor: root, error: '' });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loading: false });
    }
  },
}));
