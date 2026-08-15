/**
 * 应用配置状态（归档根目录 / 代理 / Cookie）。
 * 对应旧版 ~/.boothkeeper.json，持久化到 Tauri plugin-store。
 */

import { create } from 'zustand';

interface AppConfigState {
  boothRoot: string;
  proxy: boolean;
  proxyUrl: string;
  cookie: string;
  setBoothRoot: (v: string) => void;
  setProxy: (v: boolean) => void;
  setProxyUrl: (v: string) => void;
  setCookie: (v: string) => void;
  save: () => Promise<void>;
  hydrate: () => Promise<void>;
}

async function loadConfig(): Promise<Partial<AppConfigState> | null> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('appconfig.json', { autoSave: false });
    const boothRoot = (await store.get<string>('booth_root')) ?? '';
    const proxy = (await store.get<boolean>('proxy')) ?? true;
    const proxyUrl = (await store.get<string>('proxy_url')) ?? '';
    const cookie = (await store.get<string>('cookie')) ?? '';
    return { boothRoot, proxy, proxyUrl, cookie };
  } catch {
    return null;
  }
}

async function saveConfig(state: AppConfigState) {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('appconfig.json', { autoSave: false });
    await store.set('booth_root', state.boothRoot);
    await store.set('proxy', state.proxy);
    await store.set('proxy_url', state.proxyUrl);
    await store.set('cookie', state.cookie);
    await store.save();
  } catch {
    // 非 Tauri 环境静默
  }
}

export const useAppConfigStore = create<AppConfigState>((set, get) => ({
  boothRoot: '',
  proxy: true,
  proxyUrl: '',
  cookie: '',

  setBoothRoot: (v) => set({ boothRoot: v }),
  setProxy: (v) => set({ proxy: v }),
  setProxyUrl: (v) => set({ proxyUrl: v }),
  setCookie: (v) => set({ cookie: v }),

  save: async () => {
    await saveConfig(get());
  },

  hydrate: async () => {
    const cfg = await loadConfig();
    if (!cfg) return;
    set({
      boothRoot: cfg.boothRoot ?? '',
      proxy: cfg.proxy ?? true,
      proxyUrl: cfg.proxyUrl ?? '',
      cookie: cfg.cookie ?? '',
    });
  },
}));
