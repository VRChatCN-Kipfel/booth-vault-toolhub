/**
 * 设置页：主题三选 + 明暗 + 归档根目录 + 代理 + Cookie + 保存。
 */

import styled from 'styled-components';
import { open } from '@tauri-apps/plugin-dialog';
import {
  AccentButton, SecondaryButton, Input, PanelLabel, SegSlider, PageShell, Lead,
} from '../components/ui';
import { SpeedSlider } from '../components/SpeedSlider';
import { SupportSection } from '../components/SupportSection';
import { PageTitle } from '../components/PageTitle';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUpdateStore } from '../store/updateStore';
import { FONTS, motifSidebarSrc, THEME_HINTS, THEME_NAMES, THEME_ORDER, THEMES } from '../theme/themes';
import { brandMark, themeRadius } from '../theme/chrome';
import { openUrl } from '@tauri-apps/plugin-opener';
import { error, information } from '../components/Dialog';

const ThemeGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
`;

const ThemeCard = styled.button<{ $active: boolean; $bg: string; $border: string; $radius: string; $motif: string }>`
  text-align: left;
  padding: 0;
  overflow: hidden;
  border: 1px solid ${({ $active, $border }) => ($active ? 'var(--bvt-accent)' : $border)};
  background-color: ${({ $bg }) => $bg};
  background-image: linear-gradient(${({ $bg }) => $bg}99, ${({ $bg }) => $bg}c4), url(${({ $motif }) => $motif});
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  border-radius: ${({ $radius }) => $radius};
  cursor: pointer;
  font-family: inherit;
  box-shadow: ${({ $active }) =>
    $active
      ? 'var(--bvt-glass-highlight), 0 0 0 1px var(--bvt-accent), inset 0 0 0 1px color-mix(in srgb, var(--bvt-accent) 30%, transparent)'
      : 'var(--bvt-glass-highlight)'};
`;

const CardHead = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 12px 8px;
  .mark { width: 28px; height: 28px; flex: none; }
  .mark svg { width: 100%; height: 100%; display: block; }
  .name {
    font-family: ${FONTS.serif};
    font-size: 16px;
    letter-spacing: 0.16em;
  }
  .hint { font-size: 11px; margin-top: 3px; letter-spacing: 0.04em; }
`;

const Swatches = styled.div`
  display: grid;
  grid-template-columns: 1.4fr 0.8fr 0.6fr;
  height: 10px;
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

const VersionCard = styled.div`
  border: 1px solid var(--bvt-glass-border);
  border-top: 2px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  padding: 12px 14px;
  background: var(--bvt-surface2);
  box-shadow: var(--bvt-glass-highlight);
  .ver {
    font-family: ${FONTS.serif};
    font-size: 18px;
    letter-spacing: 0.12em;
  }
  .sub { color: var(--bvt-text2); font-size: 12px; margin-top: 4px; }
  .notes {
    margin-top: 10px;
    color: var(--bvt-text2);
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    max-height: 140px;
    overflow: auto;
  }
`;

export function SettingsPage() {
  const { theme, mode, systemTheme, setTheme, setMode } = useThemeStore();
  const {
    boothRoot, setBoothRoot,
    proxy, setProxy, proxyUrl, setProxyUrl,
    cookie, setCookie, save,
  } = useAppConfigStore();
  const { checking, info, check } = useUpdateStore();

  const resolved = resolveMode(mode, systemTheme);

  async function pickRoot() {
    const dir = await open({ directory: true, title: '选择 BOOTH 归档根目录' });
    if (dir) setBoothRoot(String(dir));
  }

  return (
    <PageShell>
      <PageTitle title="设置" />
      <Lead>主题先定调，路径和代理再填。</Lead>

      <Label>主题</Label>
      <ThemeGrid>
        {THEME_ORDER.map((t) => {
          const pal = THEMES[t][resolved];
          const active = theme === t;
          return (
            <ThemeCard
              key={t}
              type="button"
              $active={active}
              $bg={pal.surface}
              $border={pal.border}
              $radius={themeRadius(t)}
              $motif={motifSidebarSrc(t, resolved)}
              onClick={() => setTheme(t)}
            >
              <CardHead>
                <span
                  className="mark"
                  dangerouslySetInnerHTML={{ __html: brandMark(t, pal.accent) }}
                />
                <div>
                  <div className="name" style={{ color: pal.text }}>{THEME_NAMES[t]}</div>
                  <div className="hint" style={{ color: pal.text3 }}>{THEME_HINTS[t]}</div>
                </div>
              </CardHead>
              <Swatches>
                <div style={{ background: pal.bg }} />
                <div style={{ background: pal.accent }} />
                <div style={{ background: pal.btnFill }} />
              </Swatches>
            </ThemeCard>
          );
        })}
      </ThemeGrid>

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
      <Label>软件版本</Label>
      <VersionCard>
        <div className="ver">{info?.local_version || '…'}</div>
        <div className="sub">
          {checking && '正在查询 GitHub Releases…'}
          {!checking && info?.error && `查不到：${info.error}`}
          {!checking && info && !info.error && info.has_update && (
            <>有新版本 {info.remote_version}{info.release_title ? ` · ${info.release_title}` : ''}</>
          )}
          {!checking && info && !info.error && !info.has_update && '已是最新'}
        </div>
        {info?.release_body && info.has_update && (
          <div className="notes">{info.release_body}</div>
        )}
        <Row style={{ marginTop: 10 }}>
          <SecondaryButton onClick={() => void check(proxy)} disabled={checking}>
            {checking ? '检查中…' : '检查更新'}
          </SecondaryButton>
          {info?.url && (
            <SecondaryButton onClick={() => void openUrl(info.url)}>
              打开发布页
            </SecondaryButton>
          )}
        </Row>
      </VersionCard>

      <Divider />
      <AccentButton
        onClick={() => {
          void save()
            .then(() => information('已保存', '设置已写入本地配置。'))
            .catch((e) => error('保存失败', String(e)));
        }}
      >
        保存设置
      </AccentButton>

      <Divider />
      <SupportSection />
    </PageShell>
  );
}
