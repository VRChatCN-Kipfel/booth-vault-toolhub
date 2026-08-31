/**
 * 主题状态管理（zustand）。
 * 持久化到 Tauri store（plugin-store）。
 */

import { create } from 'zustand';
import type { AppIconId, ThemeName } from '../theme/themes';
import { DEFAULT_APP_ICON, DEFAULT_MODE_PER_THEME, DEFAULT_THEME, THEMES, isAppIconId } from '../theme/themes';

/** 明暗偏好：light / dark / system（跟随系统）。 */
export type ModePref = 'light' | 'dark' | 'system';
/** 实际渲染的明暗（system 已解析）。 */
export type ResolvedMode = 'light' | 'dark';

interface ThemeState {
  theme: ThemeName;
  mode: ModePref;
  /** 系统当前明暗（由 useSystemTheme 写入；仅 mode=system 时消费）。 */
  systemTheme: ResolvedMode;
  /** 动画速度倍速（0.25 ~ 4，默认 1）。 */
  animSpeed: number;
  /** 背景花纹不透明度（0 ~ 1）。 */
  motifOpacity: number;
  /** 自定义花纹图（data URL）；null 用主题自带图。 */
  motifImage: string | null;
  /** 程序图标，与主题解耦。 */
  appIcon: AppIconId;
  setTheme: (t: ThemeName) => void;
  setAppIcon: (id: AppIconId) => void;
  setMode: (m: ModePref) => void;
  cycleTheme: () => void;
  /** 三态循环明暗：light → system → dark → light。 */
  cycleMode: () => void;
  setSystemTheme: (t: ResolvedMode) => void;
  setAnimSpeed: (v: number) => void;
  setMotifOpacity: (v: number) => void;
  setMotifImage: (src: string | null) => void;
  /** 从持久化加载（返回是否成功）。 */
  hydrate: () => Promise<void>;
}

/** 解析实际渲染模式：system → 跟随系统。 */
export function resolveMode(mode: ModePref, systemTheme: ResolvedMode): ResolvedMode {
  return mode === 'system' ? systemTheme : mode;
}

const THEME_ORDER: ThemeName[] = ['zhuyin', 'liujin', 'guwen'];

/**
 * 动画速度固定档位（非线性：低速密、高速疏——人对低速差异敏感，高速感知钝化）。
 * 值即动画倍速（0.25 = 慢 4 倍，4 = 快 4 倍）。
 */
export const ANIM_STOPS: number[] = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
export const ANIM_DEFAULT = 1;
/** 最小值/最大值（供 range 约束）。 */
export const ANIM_MIN = ANIM_STOPS[0];
export const ANIM_MAX = ANIM_STOPS[ANIM_STOPS.length - 1];

/** 吸附到最近档位。 */
export function snapAnim(v: number): number {
  let best = ANIM_STOPS[0];
  let bestDist = Infinity;
  for (const s of ANIM_STOPS) {
    const d = Math.abs(s - v);
    if (d < bestDist) {
      bestDist = d;
      best = s;
    }
  }
  return best;
}

/** 档位 → 索引（range 用）。找不到回退最近。 */
export function stopIndex(v: number): number {
  const idx = ANIM_STOPS.indexOf(snapAnim(v));
  return Math.max(0, idx);
}

/** 花纹不透明度：默认即上限 50%。 */
export const MOTIF_DEFAULT = 0.5;
export const MOTIF_MAX = 0.5;

interface Prefs {
  theme?: string;
  mode?: string;
  anim_speed?: number;
  motif_opacity?: number;
  motif_image?: string;
  app_icon?: string;
}

/** 从 plugin-store 读取配置。 */
async function loadPrefs(): Promise<Prefs | null> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('settings.json', { autoSave: false });
    return {
      theme: (await store.get<string>('theme')) ?? undefined,
      mode: (await store.get<string>('mode')) ?? undefined,
      anim_speed: (await store.get<number>('anim_speed')) ?? undefined,
      motif_opacity: (await store.get<number>('motif_opacity')) ?? undefined,
      motif_image: (await store.get<string>('motif_image')) ?? undefined,
      app_icon: (await store.get<string>('app_icon')) ?? undefined,
    };
  } catch {
    return null;
  }
}

/** 写回 plugin-store（一次落全量，setter 调用 set 后直接传 get()）。 */
async function savePrefs(s: ThemeState) {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('settings.json', { autoSave: false });
    await store.set('theme', s.theme);
    await store.set('mode', s.mode);
    await store.set('anim_speed', s.animSpeed);
    await store.set('motif_opacity', s.motifOpacity);
    await store.set('motif_image', s.motifImage ?? '');
    await store.set('app_icon', s.appIcon);
    await store.save();
  } catch {
    // 非 Tauri 环境（浏览器 dev）静默失败
  }
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  theme: DEFAULT_THEME,
  mode: DEFAULT_MODE_PER_THEME[DEFAULT_THEME],
  systemTheme: 'light',
  animSpeed: ANIM_DEFAULT,
  motifOpacity: MOTIF_DEFAULT,
  motifImage: null,
  appIcon: DEFAULT_APP_ICON,

  setTheme: (t) => {
    set({ theme: t });
    void savePrefs(get());
  },

  setAppIcon: (id) => {
    set({ appIcon: id });
    void savePrefs(get());
  },

  setMode: (m) => {
    set({ mode: m });
    void savePrefs(get());
  },

  cycleTheme: () => {
    const idx = THEME_ORDER.indexOf(get().theme);
    const theme = THEME_ORDER[(idx + 1) % THEME_ORDER.length];
    set({ theme });
    void savePrefs(get());
  },

  cycleMode: () => {
    const order: ModePref[] = ['light', 'system', 'dark'];
    const idx = order.indexOf(get().mode);
    set({ mode: order[(idx + 1) % order.length] });
    void savePrefs(get());
  },

  setSystemTheme: (t) => set({ systemTheme: t }),

  setAnimSpeed: (v) => {
    set({ animSpeed: v });
    void savePrefs(get());
  },

  setMotifOpacity: (v) => {
    set({ motifOpacity: Math.min(MOTIF_MAX, Math.max(0, v)) });
    void savePrefs(get());
  },

  setMotifImage: (src) => {
    set({ motifImage: src });
    void savePrefs(get());
  },

  hydrate: async () => {
    const prefs = await loadPrefs();
    if (!prefs) return;
    const t = (prefs.theme as ThemeName | undefined) ?? get().theme;
    const m = (prefs.mode as ModePref | undefined) ?? get().mode;
    // 校验合法性，非法回退默认（对齐旧版防御清理）。
    if (t in THEMES) set({ theme: t });
    if (m === 'light' || m === 'dark' || m === 'system') set({ mode: m });
    const a = prefs.anim_speed;
    // 持久化值须吸附到档位集合（旧版线性值如 1.75/2.25 不在新档位，统一吸附）。
    if (typeof a === 'number') set({ animSpeed: snapAnim(a) });
    const o = prefs.motif_opacity;
    if (typeof o === 'number' && o >= 0) set({ motifOpacity: Math.min(MOTIF_MAX, o) });
    // 只认 data URL：路径在 webview 里加载不到，脏值当没设过。
    if (prefs.motif_image?.startsWith('data:image/')) set({ motifImage: prefs.motif_image });
    if (prefs.app_icon && isAppIconId(prefs.app_icon)) set({ appIcon: prefs.app_icon });
  },
}));
