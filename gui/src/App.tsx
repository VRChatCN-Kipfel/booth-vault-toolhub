/**
 * 应用根组件：主题注入 + 布局 + 页面导航。
 */

import { useEffect, useState, type CSSProperties } from 'react';
import styled, { ThemeProvider } from 'styled-components';
import { useThemeStore, resolveMode } from './store/themeStore';
import { useAppConfigStore } from './store/appConfigStore';
import { useSystemTheme } from './hooks/useSystemTheme';
import { THEMES } from './theme/themes';
import { GlobalStyle, paletteToVars } from './theme/global';
import { motifBg } from './theme/motifs';
import { Sidebar, type NavItemDef } from './components/Sidebar';
import { Titlebar } from './components/Titlebar';
import { StatusBar } from './components/StatusBar';
import { DialogHost } from './components/Dialog';
import { LinksPage } from './pages/LinksPage';
import { DragDropPage } from './pages/DragDropPage';
import { SearchPage } from './pages/SearchPage';
import { AuditPage } from './pages/AuditPage';
import { SettingsPage } from './pages/SettingsPage';

const NAV_ITEMS: NavItemDef[] = [
  { key: 'links', label: '批量链接' },
  { key: 'drag', label: '拖拽分类' },
  { key: 'search', label: '实验检索' },
  { key: 'audit', label: '目录巡检' },
  { key: 'settings', label: '设置' },
];

const MOTIF_KIND: Record<string, string> = {
  zhuyin: 'zhuyin',
  liujin: 'gold',
  guwen: 'guwen',
};

const ROOT_STYLE: CSSProperties = {
  display: 'flex',
  height: '100vh',
  flexDirection: 'column',
};

const BODY_STYLE: CSSProperties = {
  display: 'flex',
  flex: 1,
  minHeight: 0,
};

/** 内容区：背景母题垫底 + 页面浮于其上。 */
const ContentWrap = styled.div`
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
`;

const ContentBg = styled.div`
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  svg { width: 100%; height: 100%; }
`;

const Content = styled.div`
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 10px 12px 8px;
  background: color-mix(in srgb, var(--bvt-surface) 78%, transparent);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius, 2px);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--bvt-accent) 12%, transparent);
`;

/** 页面容器（key 变化重挂载 → 触发淡入动画）。 */
const PageWrap = styled.div`
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  animation: bvtPageIn calc(0.28s / var(--bvt-anim)) ease both;
  @keyframes bvtPageIn {
    from { opacity: 0; transform: translateY(6px); }
    to { opacity: 1; transform: translateY(0); }
  }
`;

function App() {
  const { theme, mode, systemTheme, animSpeed, setSystemTheme, hydrate: hydrateTheme } = useThemeStore();
  const { hydrate: hydrateConfig } = useAppConfigStore();
  const [page, setPage] = useState('links');
  const [vars, setVars] = useState<Record<string, string>>({});
  const sysTheme = useSystemTheme();

  useEffect(() => {
    void hydrateTheme();
    void hydrateConfig();
  }, [hydrateTheme, hydrateConfig]);

  // 系统明暗变化 → 写入 store（仅 mode=system 时消费）。
  useEffect(() => {
    setSystemTheme(sysTheme);
  }, [sysTheme, setSystemTheme]);

  // 解析实际渲染模式（system → 跟随系统）。
  const resolved = resolveMode(mode, systemTheme);

  // 主题色板 → CSS variables。
  useEffect(() => {
    setVars(paletteToVars(THEMES[theme][resolved]));
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.mode = resolved;
  }, [theme, resolved]);

  const pal = THEMES[theme][resolved];
  const bgSvg = motifBg(MOTIF_KIND[theme] as never, pal.accentDeep, resolved === 'light');

  return (
    <ThemeProvider theme={{ theme, mode: resolved }}>
      <GlobalStyle />
      <div style={ROOT_STYLE}>
        {/* CSS variables 注入到 :root（颜色过渡由 GlobalStyle 通用规则承担，动画倍速全局生效） */}
        <style>{`:root { ${Object.entries(vars).map(([k, v]) => `${k}: ${v};`).join(' ')} --bvt-anim: ${animSpeed}; }`}</style>
        <Titlebar />
        <div style={BODY_STYLE}>
          <Sidebar items={NAV_ITEMS} active={page} onNavigate={setPage} />
          <ContentWrap>
            <ContentBg dangerouslySetInnerHTML={{ __html: bgSvg }} />
            <Content>
              <PageWrap key={page}>
                {page === 'links' && <LinksPage />}
                {page === 'drag' && <DragDropPage />}
                {page === 'search' && <SearchPage />}
                {page === 'audit' && <AuditPage />}
                {page === 'settings' && <SettingsPage />}
              </PageWrap>
            </Content>
            <StatusBar />
          </ContentWrap>
        </div>
      </div>
      <DialogHost />
    </ThemeProvider>
  );
}

export default App;
