/**
 * 设置页：主题三选 + 明暗 + 归档根目录 + 代理 + Cookie + 保存。
 */

import { useState } from 'react';
import styled from 'styled-components';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
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
import { brandMark } from '../theme/chrome';
import { APP_ICON_NAMES, APP_ICON_ORDER, APP_ICON_SRC, FONTS, motifSidebarSrc, THEME_HINTS, THEME_NAMES, THEME_ORDER, THEMES } from '../theme/themes';
import { openUrl } from '@tauri-apps/plugin-opener';
import { error, information } from '../components/Dialog';

/** Cookie 获取说明：多行、可读性优先；代码字样等宽突出。 */
const CookieHelp = styled.div`
  color: var(--bvt-text3);
  font-size: var(--bvt-fz-sm);
  line-height: 1.75;
  b { font-weight: 600; }
  code {
    font-family: var(--bvt-mono);
    font-size: var(--bvt-fz-xs);
    background: var(--bvt-surface2);
    border: 1px solid var(--bvt-border);
    border-radius: var(--bvt-radius-sm);
    padding: 0 4px;
    white-space: nowrap;
  }
`;

/** Cookie 检测结论：成败着色。 */
const CkResult = styled.span<{ $ok: boolean }>`
  color: ${({ $ok }) => ($ok ? 'var(--bvt-success)' : 'var(--bvt-danger)')};
  font-size: var(--bvt-fz-sm);
  line-height: 1.5;
`;

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
  .mark {
    width: 26px;
    height: 26px;
    flex: none;
    border-radius: 6px;
    object-fit: cover;
  }
  .mark svg { width: 100%; height: 100%; display: block; }
  .name {
    font-family: ${FONTS.serif};
    font-size: var(--bvt-fz-lg);
    line-height: 1.3;
  }
  .hint { font-size: var(--bvt-fz-xs); line-height: 1.4; }
`;

/** 色板条：纸 / 朱 / 按钮填色，一眼看出这套主题的三个主色。 */
const IconGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--bvt-s3);
  @media (max-width: 720px) {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
`;

const IconCard = styled.button<{ $active: boolean; $bg: string; $border: string }>`
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--bvt-s2);
  padding: var(--bvt-s3) var(--bvt-s2);
  border: 1px solid ${({ $active, $border }) => ($active ? 'var(--bvt-accent)' : $border)};
  background: ${({ $bg }) => $bg};
  border-radius: var(--bvt-radius);
  cursor: pointer;
  font-family: inherit;
  box-shadow: ${({ $active }) => ($active ? '0 0 0 1px var(--bvt-accent)' : 'none')};
  &:hover { border-color: color-mix(in srgb, var(--bvt-accent) 45%, var(--bvt-border)); }
  .preview {
    width: 48px;
    height: 48px;
    border-radius: 11px;
    object-fit: cover;
  }
  .name {
    font-family: ${FONTS.serif};
    font-size: var(--bvt-fz-sm);
  }
`;

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

type CookieCheck = {
  state: string;
  ok: boolean;
  detail: string;
  pair_count: number;
  dropped_count: number;
  has_session: boolean;
};

export function SettingsPage() {
  const { theme, mode, systemTheme, appIcon, setTheme, setMode, setAppIcon } = useThemeStore();
  const {
    boothRoot, setBoothRoot,
    proxy, setProxy, proxyUrl, setProxyUrl,
    cookie, setCookie, save,
    keepFailedDownloads, setKeepFailedDownloads,
  } = useAppConfigStore();
  const { checking, info, check } = useUpdateStore();
  const [ck, setCk] = useState<CookieCheck | null>(null);
  const [ckBusy, setCkBusy] = useState(false);

  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];

  async function pickRoot() {
    const dir = await open({ directory: true, title: '选择 BOOTH 归档根目录' });
    if (dir) setBoothRoot(String(dir));
  }

  // 检测当前输入框的值（未保存也能测）；为空则让后端读已保存的配置。
  async function doCheckCookie() {
    setCkBusy(true);
    try {
      const r = await invoke<CookieCheck>('check_cookie', { cookie });
      setCk(r);
    } catch (e) {
      setCk({
        state: 'error',
        ok: false,
        detail: String(e),
        pair_count: 0,
        dropped_count: 0,
        has_session: false,
      });
    } finally {
      setCkBusy(false);
    }
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
                <span className="mark" dangerouslySetInnerHTML={{ __html: brandMark(t, pal.accent) }} />
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
        <PanelLabel>程序图标</PanelLabel>
        <Muted>和主题分开选，侧栏主印和窗口图标用这一张。</Muted>
        <IconGrid>
          {APP_ICON_ORDER.map((id) => (
              <IconCard
                key={id}
                type="button"
                $active={appIcon === id}
                $bg={pal.surface}
                $border={pal.border}
                onClick={() => setAppIcon(id)}
              >
                <img className="preview" src={APP_ICON_SRC[id]} alt="" />
                <div className="name" style={{ color: pal.text }}>{APP_ICON_NAMES[id]}</div>
              </IconCard>
          ))}
        </IconGrid>
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
        <CookieHelp>
          免费文件下载也需要登录 Cookie，只存本地。
          <br />
          <b>获取方式</b>：在 BOOTH 任意页面按 <b>F12</b> → 顶部选 <b>Application</b> →
          左侧 <b>Storage → Cookies → https://booth.pm</b> → 在表格里<b>全选复制</b>，
          整块粘到这里即可（不用刷新页面，也不用挑请求）。
          <br />
          也支持这两种：<b>Network</b> → 刷新 → 点任一 <code>.json</code> →
          <b> Headers → Request Headers → Cookie</b> 那一行的值；或右键请求 →
          <b> Copy as cURL</b> 整段粘贴。
          <br />
          以上格式会自动识别，统计类内容自动剔除，不必手动挑。
          登录态依赖的是 <code>_plaza_session_nktz7u</code> 这一条——
          粘贴内容里若没有它，「检测」会直接告诉您。
          <br />
          Cookie 会过期，下载报错时先点一下「检测」。
        </CookieHelp>
        <Input
          type="password"
          value={cookie}
          onChange={(e) => {
            setCookie(e.target.value);
            setCk(null);
          }}
          placeholder="从浏览器复制 BOOTH 登录 Cookie"
        />
        <Row style={{ marginTop: 'var(--bvt-s2)' }}>
          <SecondaryButton onClick={() => void doCheckCookie()} disabled={ckBusy}>
            {ckBusy ? '检测中…' : '检测 Cookie'}
          </SecondaryButton>
          {ck && (
            <CkResult $ok={ck.ok}>
              {ck.ok ? '✓ ' : '✗ '}
              {ck.detail}
            </CkResult>
          )}
        </Row>
      </Section>

      <Section>
        <PanelLabel extra={
          <CheckLabel>
            <Checkbox
              checked={keepFailedDownloads}
              onChange={(e) => setKeepFailedDownloads(e.target.checked)}
            />
            保留
          </CheckLabel>
        }>
          失败临时文件
        </PanelLabel>
        <Muted>下载失败时保留 .part 供取证，只存本地。</Muted>
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
