/**
 * 系统明暗主题 hook（Tauri 原生优先，浏览器预览降级 matchMedia）。
 *
 * 返回系统当前明暗。Tauri 环境用 getCurrentWindow().theme() + onThemeChanged
 * 实时监听；非 Tauri 环境降级 matchMedia('(prefers-color-scheme: dark)')。
 */

import { useEffect, useState } from 'react';

export type SystemTheme = 'light' | 'dark';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function useSystemTheme(): SystemTheme {
  const [theme, setTheme] = useState<SystemTheme>('light');

  useEffect(() => {
    if (isTauri) {
      let unlisten: (() => void) | undefined;
      let cancelled = false;
      // 动态导入避免浏览器预览时加载 Tauri API 报错
      import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          const w = getCurrentWindow();
          w.theme().then((t) => {
            if (!cancelled && t) setTheme(t);
          });
          return w.onThemeChanged(({ payload }) => {
            setTheme(payload);
          });
        })
        .then((fn) => {
          unlisten = fn;
        });
      return () => {
        cancelled = true;
        unlisten?.();
      };
    }
    // 浏览器降级：matchMedia
    const mql = window.matchMedia('(prefers-color-scheme: dark)');
    setTheme(mql.matches ? 'dark' : 'light');
    const onChange = () => setTheme(mql.matches ? 'dark' : 'light');
    mql.addEventListener('change', onChange);
    return () => mql.removeEventListener('change', onChange);
  }, []);

  return theme;
}
