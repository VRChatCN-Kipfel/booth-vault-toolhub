/**
 * 主题化弹窗（对齐原版 pages/notify.py ThemeDialog）。
 *
 * 四种 kind：info / warn / error / ask。
 * 默认按钮：info/warn/error → [OK]；ask → [取消, 确定]。
 * 入场动画：opacity 淡入 + 轻微上移。
 */

import { useEffect, useRef, useState } from 'react';
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
let dialogOpen = false;

export function isDialogOpen(): boolean {
  return dialogOpen;
}

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
  background: var(--bvt-surface);
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
  font-family: var(--bvt-serif);
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
  const panelRef = useRef<HTMLDivElement>(null);
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];

  useEffect(() => {
    registerDialogOpener((s) => {
      dialogOpen = true;
      setState(s);
    });
    return () => {
      dialogOpen = false;
      registerDialogOpener(null);
    };
  }, []);

  useEffect(() => {
    if (!state) return;
    const root = panelRef.current;
    const focusables = () =>
      Array.from(root?.querySelectorAll<HTMLButtonElement>('button') ?? []).filter((el) => !el.disabled);
    const initial = focusables();
    initial[initial.length - 1]?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        dialogOpen = false;
        state.resolve(null);
        setState(null);
        return;
      }
      if (e.key !== 'Tab') return;
      const els = focusables();
      if (els.length === 0) return;
      const first = els[0];
      const last = els[els.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [state]);

  if (!state) return null;
  const isAsk = state.kind === 'ask';
  const accent = pal.accent;
  const close = (result: string | null) => {
    dialogOpen = false;
    state.resolve(result);
    setState(null);
  };

  return createPortal(
    <Overlay onMouseDown={(e) => { if (e.target === e.currentTarget) close(null); }}>
      <Dialog ref={panelRef} role="dialog" aria-modal="true" aria-labelledby="bvt-dialog-title">
        <Title id="bvt-dialog-title">{state.title}</Title>
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
