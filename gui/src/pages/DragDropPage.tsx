/**
 * 拖拽分类页：文件拖入或点选 → 提取 ID → 归档队列。
 */

import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell,
  Section, FlexSection, Row, ListRow, EmptyState, Counter,
} from '../components/ui';
import { information } from '../components/Dialog';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { failedItems, useLatestTask } from '../store/taskStore';
import { cancelTask, retryFailed, runTask } from '../lib/task';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEMES } from '../theme/themes';
import { dropTile } from '../theme/motifs';
import { badgeKind, badgeLabel } from '../lib/booth';

const DROP_KIND: Record<string, 'zhuyin' | 'gold' | 'guwen'> = {
  zhuyin: 'zhuyin',
  liujin: 'gold',
  guwen: 'guwen',
};

const DropZone = styled.div<{ dragging: boolean }>`
  position: relative;
  height: 168px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--bvt-s2);
  border: 1px dashed
    ${({ dragging }) =>
      dragging ? 'var(--bvt-accent)' : 'color-mix(in srgb, var(--bvt-text3) 45%, transparent)'};
  border-radius: var(--bvt-radius);
  background: ${({ dragging }) => (dragging ? 'var(--bvt-accent-light)' : 'var(--bvt-input-bg)')};
  color: var(--bvt-text2);
  font-size: var(--bvt-fz-md);
  transition: border-color 0.2s var(--bvt-ease), background-color 0.2s var(--bvt-ease);
  &:hover { border-color: color-mix(in srgb, var(--bvt-accent) 50%, transparent); }
  .hint { font-size: var(--bvt-fz-sm); color: var(--bvt-text3); }
`;

const Motif = styled.div`
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  svg { width: 26px; height: 26px; }
`;

const NoList = styled(ObsPanel)`
  max-height: 96px;
  padding: var(--bvt-s1) 0;
  .path {
    padding: 2px var(--bvt-s3);
    font-size: var(--bvt-fz-sm);
    color: var(--bvt-text2);
    word-break: break-all;
  }
`;

const QueueList = styled(ObsPanel)`
  flex: 1;
`;

function ingest(paths: string[]): { good: string[]; bad: string[] } {
  const good: string[] = [];
  const bad: string[] = [];
  for (const p of paths) {
    if (/(?<!\d)\d{7}(?!\d)/.test(p)) good.push(p);
    else bad.push(p);
  }
  return { good, bad };
}

export function DragDropPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  const latest = useLatestTask('organize');
  const task = latest?.task;
  const [dragging, setDragging] = useState(false);
  const [pending, setPending] = useState<string[]>([]);
  const [noId, setNoId] = useState<string[]>([]);
  const [starting, setStarting] = useState(false);
  const summarized = useRef<string | null>(null);

  const running = starting || task?.status === 'running';
  const queue = task?.items ?? [];
  const total = task?.total ?? 0;
  const failed = task ? failedItems(task) : [];

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      unlisten = await getCurrentWebview().onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === 'over') setDragging(true);
        else if (t === 'leave') setDragging(false);
        else if (t === 'drop') {
          setDragging(false);
          addPaths(event.payload.paths ?? []);
        }
      });
    };
    void setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (!latest || !task || task.status !== 'done') return;
    if (summarized.current === latest.id) return;
    summarized.current = latest.id;
    const exists = task.items.filter((i) => i.status === 'exists');
    const mismatch = task.items.filter((i) => i.status === 'mismatch');
    if (exists.length === 0 && mismatch.length === 0) return;
    const lines = [
      exists.length ? `已存在 ${exists.length} 件：${exists.map((i) => i.id).join('、')}` : '',
      mismatch.length ? `错位 ${mismatch.length} 件：${mismatch.map((i) => i.id).join('、')}` : '',
    ].filter(Boolean);
    void information('归档汇总', lines.join('\n'));
  }, [latest, task]);

  function addPaths(paths: string[]) {
    const { good, bad } = ingest(paths);
    if (good.length) setPending((prev) => [...new Set([...prev, ...good])]);
    if (bad.length) setNoId((prev) => [...new Set([...prev, ...bad])]);
  }

  async function pickFiles() {
    const selected = await open({ multiple: true, title: '选择要归档的文件' });
    if (!selected) return;
    addPaths(Array.isArray(selected) ? selected : [selected]);
  }

  async function start() {
    if (pending.length === 0) return;
    setStarting(true);
    try {
      await runTask('organize', {
        archives: pending,
        out: boothRoot || null,
        dryRun: false,
        cookie: cookie || null,
      });
      setPending([]);
    } catch (e) {
      void information('归档失败', String(e));
    } finally {
      setStarting(false);
    }
  }

  function clearAll() {
    setPending([]);
    setNoId([]);
  }

  return (
    <PageShell>
      <PageTitle
        title="拖拽分类"
        desc="文件名带 7 位商品 ID 的压缩包，拖进来或点选后归档。"
      />

      <Section>
        <DropZone dragging={dragging} role="region" aria-label="拖入 BOOTH 压缩包文件">
          <Motif
            style={{ opacity: dragging ? 0.3 : 0.14 }}
            dangerouslySetInnerHTML={{ __html: dropTile(DROP_KIND[theme], pal.accent) }}
          />
          <div>拖入 BOOTH 压缩包文件</div>
          <div className="hint">文件名需含 7 位商品 ID（如 1234567_xxx.zip），支持多选</div>
        </DropZone>

        {noId.length > 0 && (
          <>
            <PanelLabel>缺少 ID 的文件 · 补名后重拖</PanelLabel>
            <NoList>
              {noId.map((p, i) => (
                <div className="path" key={i}>{p}</div>
              ))}
            </NoList>
          </>
        )}
      </Section>

      <Section>
        <Row>
          <SecondaryButton onClick={() => void pickFiles()} disabled={running}>
            选择文件
          </SecondaryButton>
          <AccentButton onClick={() => void start()} disabled={running || pending.length === 0}>
            开始归档（{pending.length}）
          </AccentButton>
          <SecondaryButton onClick={clearAll} disabled={running}>
            清空
          </SecondaryButton>
          {running && latest && (
            <SecondaryButton onClick={() => void cancelTask(latest.id)}>取消</SecondaryButton>
          )}
          {latest && task?.status === 'done' && failed.length > 0 && (
            <SecondaryButton onClick={() => void retryFailed(latest.id)}>重试失败（{failed.length}）</SecondaryButton>
          )}
          <Counter>
            待处理 {pending.length} · 完成 {queue.filter((x) => x.status === 'ok').length}
          </Counter>
        </Row>
        <ProgressBar>
          <div style={{ width: `${total ? (queue.length / total) * 100 : 0}%` }} />
        </ProgressBar>
      </Section>

      <FlexSection>
        <PanelLabel>归档队列</PanelLabel>
        <QueueList>
          {queue.map((it, i) => (
            <ListRow key={`${it.id}-${i}`}>
              <Badge kind={badgeKind(it.status)}>{badgeLabel(it.status)}</Badge>
              <span>{it.id}</span>
              <span className="msg">{it.message}</span>
              <QueueActions id={it.id} path={it.path} />
            </ListRow>
          ))}
          {queue.length === 0 && (
            <EmptyState title="队列是空的" hint="拖入文件或点「选择文件」" />
          )}
        </QueueList>
      </FlexSection>
    </PageShell>
  );
}
