/**
 * 主题状态管理（zustand）。
 * 持久化到 Tauri store（plugin-store）。
 */

import { create } from 'zustand';
import type { ThemeName } from '../theme/themes';
import { DEFAULT_MODE_PER_THEME, DEFAULT_THEME, THEMES } from '../theme/themes';

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
  setTheme: (t: ThemeName) => void;
  setMode: (m: ModePref) => void;
  cycleTheme: () => void;
  /** 三态循环明暗：light → system → dark → light。 */
  cycleMode: () => void;
  setSystemTheme: (t: ResolvedMode) => void;
  setAnimSpeed: (v: number) => void;
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

/** 从 plugin-store 读取配置。 */
async function loadPrefs(): Promise<Partial<Record<'theme' | 'mode' | 'anim_speed', string | number>> | null> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('settings.json', { autoSave: false });
    const theme = (await store.get<string>('theme')) ?? null;
    const mode = (await store.get<string>('mode')) ?? null;
    const animSpeed = (await store.get<number>('anim_speed')) ?? null;
    return {
      theme: theme ?? undefined,
      mode: mode ?? undefined,
      anim_speed: animSpeed ?? undefined,
    };
  } catch {
    return null;
  }
}

/** 写回 plugin-store。 */
async function savePrefs(theme: ThemeName, mode: ModePref, animSpeed: number) {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load('settings.json', { autoSave: false });
    await store.set('theme', theme);
    await store.set('mode', mode);
    await store.set('anim_speed', animSpeed);
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

  setTheme: (t) => {
    const cur = get();
    set({ theme: t });
    void savePrefs(t, cur.mode, cur.animSpeed);
  },

  setMode: (m) => {
    const cur = get();
    set({ mode: m });
    void savePrefs(cur.theme, m, cur.animSpeed);
  },

  cycleTheme: () => {
    const cur = get();
    const idx = THEME_ORDER.indexOf(cur.theme);
    const next = THEME_ORDER[(idx + 1) % THEME_ORDER.length];
    set({ theme: next });
    void savePrefs(next, cur.mode, cur.animSpeed);
  },

  cycleMode: () => {
    const cur = get();
    const order: ModePref[] = ['light', 'system', 'dark'];
    const idx = order.indexOf(cur.mode);
    const next = order[(idx + 1) % order.length];
    set({ mode: next });
    void savePrefs(cur.theme, next, cur.animSpeed);
  },

  setSystemTheme: (t) => set({ systemTheme: t }),

  setAnimSpeed: (v) => {
    const cur = get();
    set({ animSpeed: v });
    void savePrefs(cur.theme, cur.mode, v);
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
  },
}));
