/**
 * 拖拽分类页：文件拖入 → 提取 ID → 归档队列。
 * 用 getCurrentWebview().onDragDropEvent 拿绝对路径。
 * 拖入区麻叶纹（asa_no_ha）+ 高亮，对齐原版 dragdrop_page.py。
 */

import { useEffect, useState } from 'react';
import styled from 'styled-components';
import { invoke } from '@tauri-apps/api/core';
import { Channel } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge, PanelLabel } from '../components/ui';
import { confirmation } from '../components/Dialog';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEMES } from '../theme/themes';

interface QueueItem {
  id: string;
  message: string;
  status: 'ok' | 'warn' | 'err' | 'run' | 'wait';
}

const Page = styled.div`
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  height: 100%;
  overflow-y: auto;
`;

/** 麻叶纹（对齐 theme.py:352-371 asa_no_ha，28×28 平铺）。 */
function asaNoHa(color: string, op: number): string {
  const paths = [
    'M6 26 L6 8 L10 4',
    'M6 8 L14 16',
    'M14 16 L22 8',
    'M22 8 L22 26',
    'M8 14 L18 24',
    'M12 6 L12 26',
  ];
  const body = paths
    .map(
      (d) =>
        `<path d="${d}" fill="none" stroke="${color}" stroke-width="1.2" stroke-opacity="${op}"/>`,
    )
    .join('');
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 28" width="28" height="28">` +
    body +
    `</svg>`
  );
}

/** 麻叶纹拖入区（垫底纹样 + 高亮）。 */
const DropZone = styled.div<{ dragging: boolean }>`
  height: 180px;
  border: ${({ dragging }) => (dragging ? '2px solid var(--bvt-accent)' : '2px dashed var(--bvt-border)')};
  background: ${({ dragging }) => (dragging ? 'var(--bvt-accent-light)' : 'var(--bvt-surface)')};
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 10px;
  color: var(--bvt-text2);
  transition: all 0.2s;
  border-radius: 2px;
  font-size: 14px;
  position: relative;
  .hint { font-size: 12px; color: var(--bvt-text3); }
`;

const Motif = styled.div`
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  opacity: 0.2;
  svg { width: 28px; height: 28px; }
`;

const NoList = styled(ObsPanel)`
  max-height: 84px;
  min-height: 40px;
`;

const QueueList = styled(ObsPanel)`
  flex: 1;
  min-height: 0;
`;

const QueueRow = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--bvt-border2);
  font-size: 13px;
  color: var(--bvt-text);
  .msg { color: var(--bvt-text2); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
`;

export function DragDropPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  const [dragging, setDragging] = useState(false);
  const [pending, setPending] = useState<string[]>([]);
  const [noId, setNoId] = useState<string[]>([]);
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [running, setRunning] = useState(false);
  const [total, setTotal] = useState(0);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      unlisten = await getCurrentWebview().onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === 'over') setDragging(true);
        else if (t === 'leave') setDragging(false);
        else if (t === 'drop') {
          setDragging(false);
          const paths = event.payload.paths ?? [];
          const good: string[] = [];
          const bad: string[] = [];
          for (const p of paths) {
            const m = p.match(/(?<!\d)(\d{7})(?!\d)/);
            if (m) good.push(p);
            else bad.push(p);
          }
          if (good.length) setPending((prev) => [...new Set([...prev, ...good])]);
          if (bad.length) setNoId((prev) => [...new Set([...prev, ...bad])]);
        }
      });
    };
    void setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  async function start() {
    if (pending.length === 0) return;
    setQueue([]);
    setTotal(pending.length);
    setRunning(true);
    const onEvent = new Channel<Record<string, unknown>>();
    const taskId = await invoke<string>('organize', {
      archives: pending,
      out: boothRoot || null,
      dryRun: false,
      onEvent,
    }).catch((e) => {
      setQueue([{ id: '-', message: String(e), status: 'err' }]);
      setRunning(false);
      return null;
    });
    if (taskId === null) return;

    onEvent.onmessage = (evt) => {
      const type = evt.type as string;
      if (type === 'itemDone') {
        const status = String(evt.status ?? 'ok');
        if (status === 'exists') {
          void confirmation('已存在', `${evt.id}\n目标目录已存在同名文件，跳过移动。`);
          setQueue((q) => [...q, { id: String(evt.id ?? ''), message: '目标已存在，跳过', status: 'warn' }]);
        } else if (status === 'mismatch') {
          void confirmation('错位', `${evt.id}\n同 ID 已在其他类目。是否重新归档到正确分类？`);
          setQueue((q) => [...q, { id: String(evt.id ?? ''), message: '已归档（可能错位）', status: 'ok' }]);
        } else {
          setQueue((q) => [...q, { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'ok' }]);
        }
        setPending((p) => p.filter((x) => x !== String(evt.id ?? '')));
      } else if (type === 'itemError') {
        setQueue((q) => [...q, { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'err' }]);
      } else if (type === 'finished' || type === 'cancelled') {
        setRunning(false);
      }
    };
  }

  function clearAll() {
    setPending([]);
    setNoId([]);
    setQueue([]);
  }

  return (
    <Page>
      <PageTitle title="拖拽分类" />
      <DropZone dragging={dragging}>
        <Motif dangerouslySetInnerHTML={{ __html: asaNoHa(pal.accent, dragging ? 0.32 : 0.18) }} />
        <div>拖入 BOOTH 压缩包文件</div>
        <div className="hint">文件名需含 7 位商品 ID（如 1234567_xxx.zip），支持多选</div>
      </DropZone>
      {noId.length > 0 && (
        <div>
          <PanelLabel style={{ color: 'var(--bvt-warn)' }}>缺少 ID 的文件（补名后重拖）</PanelLabel>
          <NoList>
            {noId.map((p, i) => (
              <div key={i} style={{ fontSize: 12, color: 'var(--bvt-text2)', padding: '2px 6px' }}>
                {p}
              </div>
            ))}
          </NoList>
        </div>
      )}
      <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
        <AccentButton onClick={() => void start()} disabled={running || pending.length === 0}>
          开始归档（{pending.length}）
        </AccentButton>
        <SecondaryButton onClick={clearAll} disabled={running}>
          清空
        </SecondaryButton>
        <span style={{ color: 'var(--bvt-text2)', fontSize: 13 }}>
          队列 {pending.length} / 完成 {queue.filter((x) => x.status === 'ok').length}
        </span>
      </div>
      <PanelLabel>待归档队列</PanelLabel>
      <ProgressBar>
        <div style={{ width: `${total ? (queue.length / total) * 100 : 0}%` }} />
      </ProgressBar>
      <QueueList>
        {queue.map((it, i) => (
          <QueueRow key={i}>
            <Badge kind={it.status}>{it.status}</Badge>
            <span>{it.id}</span>
            <span className="msg">{it.message}</span>
          </QueueRow>
        ))}
        {queue.length === 0 && (
          <div style={{ color: 'var(--bvt-text3)', padding: 8 }}>等待拖入文件…</div>
        )}
      </QueueList>
    </Page>
  );
}
