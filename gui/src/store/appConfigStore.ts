/**
 * 应用配置（归档根目录 / 代理 / Cookie）。
 * 单一事实源：用户目录 config.toml（与 CLI / MCP 共用）。
 * 旧版 Tauri plugin-store 仅作一次性迁移。
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

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

interface TomlSettings {
  boothRoot: string;
  proxy: boolean;
  proxyUrl: string;
  cookie: string;
}

async function loadLegacyStore(): Promise<Partial<AppConfigState> | null> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('appconfig.json', { autoSave: false });
    const boothRoot = (await store.get<string>('booth_root')) ?? '';
    const proxy = (await store.get<boolean>('proxy')) ?? true;
    const proxyUrl = (await store.get<string>('proxy_url')) ?? '';
    const cookie = (await store.get<string>('cookie')) ?? '';
    if (!boothRoot && !proxyUrl && !cookie) return null;
    return { boothRoot, proxy, proxyUrl, cookie };
  } catch {
    return null;
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
    const s = get();
    await invoke('save_app_config', {
      boothRoot: s.boothRoot,
      proxy: s.proxy,
      proxyUrl: s.proxyUrl,
      cookie: s.cookie,
    });
  },
  hydrate: async () => {
    try {
      const cfg = await invoke<TomlSettings>('load_app_config');
      const empty = !cfg.boothRoot && !cfg.proxyUrl && !cfg.cookie;
      if (!empty) {
        set({
          boothRoot: cfg.boothRoot ?? '',
          proxy: cfg.proxy ?? true,
          proxyUrl: cfg.proxyUrl ?? '',
          cookie: cfg.cookie ?? '',
        });
        return;
      }
    } catch {
      // 非 Tauri 或命令未就绪
    }
    const legacy = await loadLegacyStore();
    if (!legacy) return;
    set({
      boothRoot: legacy.boothRoot ?? '',
      proxy: legacy.proxy ?? true,
      proxyUrl: legacy.proxyUrl ?? '',
      cookie: legacy.cookie ?? '',
    });
    try {
      await get().save();
    } catch {
      // 迁移失败下次再试
    }
  },
}));
