/**
 * 主题化弹窗（对齐原版 pages/notify.py ThemeDialog）。
 *
 * 四种 kind：info / warn / error / ask。
 * 默认按钮：info/warn/error → [OK]；ask → [取消, 确定]。
 * 入场动画：opacity 淡入 + 轻微上移。
 */

import { useEffect, useState } from 'react';
import styled from 'styled-components';
import { createPortal } from 'react-dom';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEMES } from '../theme/themes';
import { AccentButton, SecondaryButton } from './ui';

export type DialogKind = 'info' | 'warn' | 'error' | 'ask';

interface ThemeDialogState {
  kind: DialogKind;
  title: string;
  body: string;
  resolve: (result: string | null) => void;
}

let openDialog: ((state: ThemeDialogState) => void) | null = null;

/** 注册全局弹窗打开器（App 挂载时调用）；传 null 取消注册。 */
export function registerDialogOpener(fn: ((s: ThemeDialogState) => void) | null) {
  openDialog = fn;
}

/** 静态工厂。 */
export function showDialog(kind: DialogKind, title: string, body: string): Promise<string | null> {
  if (!openDialog) {
    // 未挂载时 fallback
    return Promise.resolve(kind === 'ask' ? '取消' : 'OK');
  }
  return new Promise((resolve) => {
    openDialog!({ kind, title, body, resolve });
  });
}

export const information = (title: string, body: string) => showDialog('info', title, body);
export const warning = (title: string, body: string) => showDialog('warn', title, body);
export const error = (title: string, body: string) => showDialog('error', title, body);
export const confirmation = async (title: string, body: string): Promise<boolean> => {
  const r = await showDialog('ask', title, body);
  return r === '确定';
};

const Overlay = styled.div`
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(8px) saturate(1.2);
  -webkit-backdrop-filter: blur(8px) saturate(1.2);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  animation: bvtOverlayIn calc(0.18s / var(--bvt-anim)) ease both;
  @keyframes bvtOverlayIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
`;

const Dialog = styled.div`
  min-width: 420px;
  max-width: 620px;
  background: var(--bvt-glass);
  backdrop-filter: blur(var(--bvt-glass-blur)) saturate(var(--bvt-glass-sat));
  -webkit-backdrop-filter: blur(var(--bvt-glass-blur)) saturate(var(--bvt-glass-sat));
  border: 1.5px solid var(--bvt-accent-deep);
  border-top: 3px solid var(--bvt-accent);
  border-radius: var(--bvt-radius, 0px);
  padding: 18px 22px;
  box-shadow: var(--bvt-glass-highlight);
  animation: bvtDialogIn calc(0.18s / var(--bvt-anim)) ease both;
  @keyframes bvtDialogIn {
    from { opacity: 0; transform: translateY(-8px); }
    to { opacity: 1; transform: translateY(0); }
  }
`;

const Title = styled.div`
  font-size: 15px;
  font-weight: 700;
  color: var(--bvt-text);
  font-family: 'Noto Serif CJK SC','Songti SC',serif;
  letter-spacing: var(--bvt-title-track, 0.1em);
  border-bottom: 1px solid var(--bvt-border);
  padding: 4px 0 8px;
  margin-bottom: 10px;
`;

const Body = styled.div`
  font-size: 13px;
  color: var(--bvt-text);
  line-height: 1.55;
  padding: 4px 0;
  white-space: pre-wrap;
  word-break: break-word;
`;

const Buttons = styled.div`
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
`;

/** 弹窗宿主（App 挂载一次）。 */
export function DialogHost() {
  const [state, setState] = useState<ThemeDialogState | null>(null);
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];

  useEffect(() => {
    registerDialogOpener((s) => setState(s));
    return () => registerDialogOpener(null);
  }, []);

  useEffect(() => {
    if (!state) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        state.resolve(null);
        setState(null);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [state]);

  if (!state) return null;
  const isAsk = state.kind === 'ask';
  const accent = pal.accent;
  const close = (result: string | null) => {
    state.resolve(result);
    setState(null);
  };

  return createPortal(
    <Overlay onMouseDown={(e) => { if (e.target === e.currentTarget) close(null); }}>
      <Dialog>
        <Title>{state.title}</Title>
        <Body>{state.body}</Body>
        <Buttons>
          {isAsk ? (
            <>
              <SecondaryButton onClick={() => close('取消')}>取消</SecondaryButton>
              <AccentButton onClick={() => close('确定')}>确定</AccentButton>
            </>
          ) : (
            <AccentButton style={{ borderColor: accent }} onClick={() => close('OK')}>OK</AccentButton>
          )}
        </Buttons>
      </Dialog>
    </Overlay>,
    document.body,
  );
}
