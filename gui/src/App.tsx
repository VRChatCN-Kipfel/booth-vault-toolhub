/**
 * 应用根组件：主题注入 + 布局 + 页面导航。
 */

import { useEffect, useState, type CSSProperties } from 'react';
import styled, { ThemeProvider } from 'styled-components';
import { useThemeStore, resolveMode } from './store/themeStore';
import { useAppConfigStore } from './store/appConfigStore';
import { useUpdateStore } from './store/updateStore';
import { useUiStore } from './store/uiStore';
import { useSystemTheme } from './hooks/useSystemTheme';
import { Link2, MousePointerSquareDashed, Search, ClipboardCheck, Settings2 } from 'lucide-react';
import { THEMES } from './theme/themes';
import { GlobalStyle, paletteToVars } from './theme/global';
import { Sidebar, type NavItemDef } from './components/Sidebar';
import { Titlebar } from './components/Titlebar';
import { StatusBar } from './components/StatusBar';
import { confirmation, DialogHost, isDialogOpen } from './components/Dialog';
import { useTaskStore } from './store/taskStore';
import { cancelTask } from './lib/task';
import { LinksPage } from './pages/LinksPage';
import { DragDropPage } from './pages/DragDropPage';
import { SearchPage } from './pages/SearchPage';
import { AuditPage } from './pages/AuditPage';
import { SettingsPage } from './pages/SettingsPage';

const NAV_ITEMS: NavItemDef[] = [
  { key: 'links', label: '批量链接', icon: Link2 },
  { key: 'drag', label: '拖拽分类', icon: MousePointerSquareDashed },
  { key: 'search', label: '实验检索', icon: Search },
  { key: 'audit', label: '目录巡检', icon: ClipboardCheck },
  { key: 'settings', label: '设置', icon: Settings2 },
];

/** 三端窗口形态差异只认这一处；其余靠 html[data-platform] 选择器分流。 */
function detectPlatform(): 'mac' | 'win' | 'linux' {
  const ua = navigator.userAgent;
  if (/Mac|Macintosh/.test(ua)) return 'mac';
  if (/Windows/.test(ua)) return 'win';
  return 'linux';
}

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

const ContentWrap = styled.div`
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
`;

/** 册页：内容区整幅铺满，靠侧栏的界线分隔，不再套一层描边卡片。 */
const Content = styled.main`
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bvt-shell-bg);
`;

/** 页面容器（key 变化重挂载 → 触发淡入动画）。 */
const PageWrap = styled.div`
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  animation: bvtPageIn calc(0.24s / var(--bvt-anim)) var(--bvt-ease) both;
  @keyframes bvtPageIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }
`;

function isEditableTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.isContentEditable) return true;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

function App() {
  const {
    theme, mode, systemTheme, animSpeed, motifOpacity, setSystemTheme, hydrate: hydrateTheme,
  } = useThemeStore();
  const { hydrate: hydrateConfig, proxy } = useAppConfigStore();
  const checkUpdate = useUpdateStore((s) => s.check);
  const pendingPage = useUiStore((s) => s.pendingPage);
  const consumePage = useUiStore((s) => s.consumePage);
  const [page, setPage] = useState('links');
  const sysTheme = useSystemTheme();

  useEffect(() => {
    void hydrateTheme();
    void hydrateConfig();
  }, [hydrateTheme, hydrateConfig]);

  useEffect(() => {
    void checkUpdate(proxy);
  }, [checkUpdate, proxy]);

  useEffect(() => {
    if (!pendingPage) return;
    setPage(pendingPage);
    consumePage();
  }, [pendingPage, consumePage]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing || e.keyCode === 229) return;
      if (e.target instanceof Element && e.target.closest('[role=dialog]')) return;
      if (isEditableTarget(e.target)) return;
      if ((e.metaKey || e.ctrlKey) && e.key >= '1' && e.key <= '5') {
        const item = NAV_ITEMS[Number(e.key) - 1];
        if (!item) return;
        e.preventDefault();
        setPage(item.key);
        return;
      }
      if (e.key !== 'Escape' || isDialogOpen()) return;
      const running = Object.entries(useTaskStore.getState().tasks).filter(([, t]) => t.status === 'running');
      if (running.length === 0) return;
      e.preventDefault();
      const msg = running.length === 1
        ? `取消「${running[0][1].label}」？`
        : `取消全部 ${running.length} 个进行中的任务？`;
      void confirmation('取消任务', msg).then((ok) => {
        if (!ok) return;
        for (const [id] of running) void cancelTask(id);
      });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // 系统明暗变化 → 写入 store（仅 mode=system 时消费）。
  useEffect(() => {
    setSystemTheme(sysTheme);
  }, [sysTheme, setSystemTheme]);

  // 解析实际渲染模式（system → 跟随系统）。
  const resolved = resolveMode(mode, systemTheme);

  useEffect(() => {
    const root = document.documentElement;
    const next = paletteToVars(THEMES[theme][resolved], resolved);
    for (const [k, v] of Object.entries(next)) {
      root.style.setProperty(k, v);
    }
    root.style.setProperty('--bvt-anim', String(animSpeed));
    root.style.setProperty('--bvt-motif-opacity', String(motifOpacity));
    root.dataset.theme = theme;
    root.dataset.mode = resolved;
    const platform = detectPlatform();
    root.dataset.platform = platform;
    if (platform === 'mac') {
      // 玻璃填色交给 CSS，别让 JS 色板把原生材质盖死。
      root.style.removeProperty('--bvt-glass');
      root.style.removeProperty('--bvt-glass-2');
      root.style.removeProperty('--bvt-glass-input');
    }
  }, [theme, resolved, animSpeed, motifOpacity]);

  return (
    <ThemeProvider theme={{ theme, mode: resolved }}>
      <GlobalStyle />
      <div style={ROOT_STYLE}>
        <Titlebar />
        <div style={BODY_STYLE}>
          <Sidebar items={NAV_ITEMS} active={page} onNavigate={setPage} />
          <ContentWrap>
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
