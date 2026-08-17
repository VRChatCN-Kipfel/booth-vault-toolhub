/**
 * 设置页：主题三选 + 明暗 + 归档根目录 + 代理 + Cookie + 保存。
 * 对齐原版 settings_page.py。
 */

import styled from 'styled-components';
import { open } from '@tauri-apps/plugin-dialog';
import {
  AccentButton, SecondaryButton, Input, PanelLabel, SegSlider,
} from '../components/ui';
import { SpeedSlider } from '../components/SpeedSlider';
import { SupportSection } from '../components/SupportSection';
import { PageTitle } from '../components/PageTitle';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { useAppConfigStore } from '../store/appConfigStore';
import { THEME_NAMES, THEME_ORDER, THEMES } from '../theme/themes';

const Page = styled.div`
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow-y: auto;
`;

const SubTitle = styled.div`
  color: var(--bvt-text2);
  font-size: 13px;
  margin-top: -8px;
`;

const Row = styled.div`
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
`;

const Label = styled(PanelLabel)`
  padding-top: 8px;
`;

const Checkbox = styled.input`
  width: 16px;
  height: 16px;
  accent-color: var(--bvt-accent);
`;

const Divider = styled.div`
  height: 1px;
  background: var(--bvt-border2);
  margin: 4px 0;
`;

export function SettingsPage() {
  const { theme, mode, systemTheme, setTheme, setMode } = useThemeStore();
  const {
    boothRoot, setBoothRoot,
    proxy, setProxy, proxyUrl, setProxyUrl,
    cookie, setCookie, save,
  } = useAppConfigStore();

  const resolved = resolveMode(mode, systemTheme);

  async function pickRoot() {
    const dir = await open({ directory: true, title: '选择 BOOTH 归档根目录' });
    if (dir) setBoothRoot(String(dir));
  }

  return (
    <Page>
      <PageTitle title="设置" />
      <SubTitle>主题、归档路径与网络</SubTitle>

      <Label>主题（三选一，整套视觉意境切换）</Label>
      <SegSlider
        options={THEME_ORDER.map((t) => THEME_NAMES[t])}
        value={THEME_ORDER.indexOf(theme)}
        accent={THEMES[theme][resolved].accent}
        onChange={(i) => setTheme(THEME_ORDER[i])}
      />

      <Divider />

      <Label>明暗（亮色 / 系统 / 深色）</Label>
      <SegSlider
        options={['亮色', '系统', '深色']}
        value={mode === 'dark' ? 2 : mode === 'system' ? 1 : 0}
        accent={THEMES[theme][resolved].accent}
        onChange={(i) => {
          if (i === 1) setMode('system');
          else setMode(i === 0 ? 'light' : 'dark');
        }}
      />

      <Divider />

      <Label>动画速度</Label>
      <SpeedSlider />

      <Divider />

      <Label>BOOTH 归档根目录</Label>
      <Row>
        <Input
          value={boothRoot}
          onChange={(e) => setBoothRoot(e.target.value)}
          style={{ flex: 1, minWidth: 200 }}
          placeholder="BOOTH 归档根目录"
        />
        <SecondaryButton onClick={() => void pickRoot()}>浏览</SecondaryButton>
      </Row>

      <Label>网络代理（访问 Booth 多数需代理）</Label>
      <Row>
        <Checkbox
          type="checkbox"
          checked={proxy}
          onChange={(e) => setProxy(e.target.checked)}
        />
        <span style={{ color: 'var(--bvt-text2)', fontSize: 13 }}>启用代理</span>
      </Row>
      <Input
        value={proxyUrl}
        onChange={(e) => setProxyUrl(e.target.value)}
        disabled={!proxy}
        style={{ width: '100%' }}
        placeholder="http://127.0.0.1:7890"
      />

      <Label>Booth Cookie（可选，访问受限商品）</Label>
      <Input
        type="password"
        value={cookie}
        onChange={(e) => setCookie(e.target.value)}
        style={{ width: '100%' }}
        placeholder="留空即可，仅在受限时填写"
      />

      <Divider />
      <AccentButton
        onClick={() => {
          void save().catch((e) => {
            window.alert(`保存失败：${e}`);
          });
        }}
      >
        保存设置
      </AccentButton>

      <Divider />
      <SupportSection />
    </Page>
  );
}
