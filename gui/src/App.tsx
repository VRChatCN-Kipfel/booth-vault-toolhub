/**
 * 应用根组件：主题注入 + 布局 + 页面导航。
 */

import { useEffect, useState, type CSSProperties } from 'react';
import styled, { ThemeProvider } from 'styled-components';
import { useThemeStore, resolveMode } from './store/themeStore';
import { useAppConfigStore } from './store/appConfigStore';
import { useSystemTheme } from './hooks/useSystemTheme';
import { motifBgSrc, THEMES } from './theme/themes';
import { GlobalStyle, paletteToVars } from './theme/global';
import { contentFrame } from './theme/chrome';
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

const ContentBg = styled.div<{ $src: string }>`
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  background-image: url(${({ $src }) => $src});
  background-size: cover;
  background-position: center;
  opacity: ${({ theme }) => (theme.mode === 'dark' ? 0.34 : 0.32)};
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
  background: color-mix(in srgb, var(--bvt-surface) 96%, transparent);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius, 0px);
  box-shadow: ${({ theme }) => contentFrame(theme.theme)};
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

  useEffect(() => {
    const root = document.documentElement;
    const next = paletteToVars(THEMES[theme][resolved]);
    for (const [k, v] of Object.entries(next)) {
      root.style.setProperty(k, v);
    }
    root.style.setProperty('--bvt-anim', String(animSpeed));
    root.dataset.theme = theme;
    root.dataset.mode = resolved;
  }, [theme, resolved, animSpeed]);

  return (
    <ThemeProvider theme={{ theme, mode: resolved }}>
      <GlobalStyle />
      <div style={ROOT_STYLE}>
        <Titlebar />
        <div style={BODY_STYLE}>
          <Sidebar items={NAV_ITEMS} active={page} onNavigate={setPage} />
          <ContentWrap>
            <ContentBg $src={motifBgSrc(theme, resolved)} />
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
