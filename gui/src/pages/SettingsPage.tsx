/**
 * 设置页：主题三选 + 明暗 + 归档根目录 + 代理 + Cookie + 保存。
 */

import styled from 'styled-components';
import { open } from '@tauri-apps/plugin-dialog';
import {
  AccentButton, SecondaryButton, Input, PanelLabel, SegSlider, PageShell,
  Section, Row, Checkbox, CheckLabel, Muted,
} from '../components/ui';
import { SpeedSlider } from '../components/SpeedSlider';
import { MotifSetting } from '../components/MotifSetting';
import { SupportSection } from '../components/SupportSection';
import { PageTitle } from '../components/PageTitle';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUpdateStore } from '../store/updateStore';
import { FONTS, motifSidebarSrc, THEME_HINTS, THEME_NAMES, THEME_ORDER, THEMES } from '../theme/themes';
import { brandMark } from '../theme/chrome';
import { openUrl } from '@tauri-apps/plugin-opener';
import { error, information } from '../components/Dialog';

const ThemeGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--bvt-s3);
  @media (max-width: 720px) {
    grid-template-columns: 1fr;
  }
`;

const ThemeCard = styled.button<{ $active: boolean; $bg: string; $border: string; $motif: string }>`
  text-align: left;
  padding: 0;
  overflow: hidden;
  border: 1px solid ${({ $active, $border }) => ($active ? 'var(--bvt-accent)' : $border)};
  background-color: ${({ $bg }) => $bg};
  background-image: linear-gradient(${({ $bg }) => $bg}e0, ${({ $bg }) => $bg}f2), url(${({ $motif }) => $motif});
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  border-radius: var(--bvt-radius);
  cursor: pointer;
  font-family: inherit;
  transition: box-shadow 0.15s ease, border-color 0.15s ease;
  box-shadow: ${({ $active }) => ($active ? '0 0 0 1px var(--bvt-accent)' : 'var(--bvt-shadow-1)')};
  &:hover { border-color: color-mix(in srgb, var(--bvt-accent) 45%, var(--bvt-border)); }
`;

const CardHead = styled.div`
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  padding: var(--bvt-s3) var(--bvt-s3) var(--bvt-s2);
  .mark { width: 26px; height: 26px; flex: none; }
  .mark svg { width: 100%; height: 100%; display: block; }
  .name {
    font-family: ${FONTS.serif};
    font-size: var(--bvt-fz-lg);
    line-height: 1.3;
  }
  .hint { font-size: var(--bvt-fz-xs); line-height: 1.4; }
`;

/** 色板条：纸 / 朱 / 按钮填色，一眼看出这套主题的三个主色。 */
const Swatches = styled.div`
  display: grid;
  grid-template-columns: 1.4fr 0.8fr 0.6fr;
  height: 8px;
`;

const VersionCard = styled.div`
  padding: var(--bvt-s4);
  background: var(--bvt-surface2);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius);
  .ver {
    font-family: ${FONTS.serif};
    font-size: var(--bvt-fz-title);
    line-height: 1.3;
    font-variant-numeric: tabular-nums;
  }
  .sub { margin-top: var(--bvt-s1); color: var(--bvt-text2); font-size: var(--bvt-fz-sm); }
  .notes {
    margin-top: var(--bvt-s3);
    padding-top: var(--bvt-s3);
    border-top: 1px solid var(--bvt-border2);
    color: var(--bvt-text2);
    font-size: var(--bvt-fz-sm);
    line-height: 1.7;
    white-space: pre-wrap;
    max-height: 160px;
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
      <PageTitle title="设置" desc="主题先定调，路径和代理再填。" />

      <Section>
        <PanelLabel>主题</PanelLabel>
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
      </Section>

      <Section>
        <PanelLabel>明暗</PanelLabel>
        <SegSlider
          options={['亮色', '系统', '深色']}
          value={mode === 'dark' ? 2 : mode === 'system' ? 1 : 0}
          accent={THEMES[theme][resolved].accent}
          onChange={(i) => {
            if (i === 1) setMode('system');
            else setMode(i === 0 ? 'light' : 'dark');
          }}
        />
      </Section>

      <Section>
        <MotifSetting />
      </Section>

      <Section>
        <SpeedSlider />
      </Section>

      <Section>
        <PanelLabel>归档根目录</PanelLabel>
        <Row>
          <Input
            value={boothRoot}
            onChange={(e) => setBoothRoot(e.target.value)}
            style={{ flex: 1, minWidth: 200 }}
            placeholder="BOOTH 归档根目录"
          />
          <SecondaryButton onClick={() => void pickRoot()}>浏览</SecondaryButton>
        </Row>
      </Section>

      <Section>
        <PanelLabel extra={
          <CheckLabel>
            <Checkbox checked={proxy} onChange={(e) => setProxy(e.target.checked)} />
            启用
          </CheckLabel>
        }>
          网络代理
        </PanelLabel>
        <Muted>访问 BOOTH 多数情况需要代理。留空则按系统代理走。</Muted>
        <Input
          value={proxyUrl}
          onChange={(e) => setProxyUrl(e.target.value)}
          disabled={!proxy}
          placeholder="http://127.0.0.1:7890"
        />
      </Section>

      <Section>
        <PanelLabel>Booth Cookie</PanelLabel>
        <Muted>可选，仅在访问受限商品时填写。只存本地。</Muted>
        <Input
          type="password"
          value={cookie}
          onChange={(e) => setCookie(e.target.value)}
          placeholder="留空即可"
        />
      </Section>

      <Section>
        <PanelLabel>软件版本</PanelLabel>
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
          <Row style={{ marginTop: 'var(--bvt-s3)' }}>
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
      </Section>

      <Section>
        <AccentButton
          style={{ alignSelf: 'flex-start' }}
          onClick={() => {
            void save()
              .then(() => information('已保存', '设置已写入本地配置。'))
              .catch((e) => error('保存失败', String(e)));
          }}
        >
          保存设置
        </AccentButton>
      </Section>

      <SupportSection />
    </PageShell>
  );
}
