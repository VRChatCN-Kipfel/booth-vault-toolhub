/**
 * 实验检索页：文件名 → score_and_pick 候选 → 歧义人工选 → 原路径+forceId 归档。
 */

import { useEffect, useMemo, useRef, useState } from 'react';
import styled from 'styled-components';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  AccentButton, SecondaryButton, TextArea, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell,
  Section, FlexSection, Row, ListRow, EmptyState, Muted, Counter,
} from '../components/ui';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { failedItems, useLatestTask } from '../store/taskStore';
import { cancelTask, retryFailed, runTask } from '../lib/task';
import { badgeKind, badgeLabel, boothItemUrl, formatPrice } from '../lib/booth';

const ResultList = styled(ObsPanel)`
  flex: 1;
`;

/** 一个来源文件一组：组头是文件路径，组内是候选商品。 */
const FileBlock = styled.div`
  border-bottom: 1px solid var(--bvt-border);
  &:last-child { border-bottom: none; }
`;

const FileHead = styled.div`
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  padding: var(--bvt-s2) var(--bvt-s3);
  background: var(--bvt-surface2);
  font-size: var(--bvt-fz-sm);
  color: var(--bvt-text2);
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
`;

const AmbiguousHint = styled.div`
  padding: var(--bvt-s1) var(--bvt-s3);
  color: var(--bvt-warn);
  font-size: var(--bvt-fz-sm);
`;

const ResultRow = styled(ListRow)`
  cursor: pointer;
  border-bottom: none;
  .price { flex: none; color: var(--bvt-text2); font-size: var(--bvt-fz-sm); }
`;

const ArchiveList = styled(ObsPanel)`
  max-height: 160px;
`;

function parseFiles(blob: string): string[] {
  return blob.split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
}

export function SearchPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const preview = useLatestTask('search');
  const archive = useLatestTask('search_archive');
  const [text, setText] = useState('');
  const [selected, setSelected] = useState<Record<string, string>>({});
  const [starting, setStarting] = useState(false);
  const [status, setStatus] = useState('');

  // 输入框支持文件/文件夹拖放：拖入即把本地绝对路径按行追加（对齐拖拽分类页既有模式）。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      unlisten = await getCurrentWebview().onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t !== 'drop') return;
        const paths = event.payload.paths ?? [];
        if (paths.length === 0) return;
        setText((cur) => {
          const lines = [
            ...(cur ? cur.split(/\r?\n/) : []),
            ...paths,
          ].map((s) => s.trim()).filter(Boolean);
          return [...new Set(lines)].join('\n');
        });
      });
    };
    void setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const previewTask = preview?.task;
  const archiveTask = archive?.task;
  const searching = starting || previewTask?.status === 'running';
  const archiving = archiveTask?.status === 'running';
  const groups = useMemo(
    () => (previewTask?.items ?? []).filter((i) => i.candidates),
    [previewTask],
  );
  const errors = (previewTask?.items ?? []).filter((i) => i.status === 'err' && !i.candidates);
  const archiveQueue = archiveTask?.items ?? [];
  const archiveFailed = archiveTask ? failedItems(archiveTask) : [];
  const archiveTotal = archiveTask?.total ?? 0;
  const archiveDone = archiveTask?.done ?? archiveQueue.length;

  const blocked = groups.some((g) => g.ambiguous && g.source && !selected[g.source]);
  const picks = groups.flatMap((g) => {
    if (!g.source) return [];
    if (g.ambiguous) {
      const id = selected[g.source];
      return id ? [{ source: g.source, id }] : [];
    }
    const id = selected[g.source] || g.picked;
    return id ? [{ source: g.source, id }] : [];
  });

  async function search() {
    const files = parseFiles(text);
    if (files.length === 0) return;
    setSelected({});
    setStatus('搜索中…');
    setStarting(true);
    try {
      await runTask(
        'search',
        {
          files,
          baseDir: boothRoot || null,
          dryRun: true,
          cookie: cookie || null,
        },
        { kind: 'search' },
      );
      setStatus('搜索完成');
    } catch (e) {
      setStatus(String(e));
    } finally {
      setStarting(false);
    }
  }

  function toggle(source: string, id: string) {
    setSelected((sel) => (sel[source] === id ? { ...sel, [source]: '' } : { ...sel, [source]: id }));
  }

  const clickPending = useRef<{
    source: string;
    id: string;
    timer: ReturnType<typeof setTimeout>;
  } | null>(null);

  useEffect(() => () => {
    if (clickPending.current) clearTimeout(clickPending.current.timer);
  }, []);

  function handleRowClick(source: string, id: string) {
    const pending = clickPending.current;
    if (pending && pending.source === source && pending.id === id) {
      clearTimeout(pending.timer);
      clickPending.current = null;
      return;
    }
    if (pending) {
      clearTimeout(pending.timer);
      clickPending.current = null;
      toggle(pending.source, pending.id);
    }
    clickPending.current = {
      source,
      id,
      timer: setTimeout(() => {
        clickPending.current = null;
        toggle(source, id);
      }, 250),
    };
  }

  function handleRowDoubleClick(id: string) {
    if (clickPending.current) {
      clearTimeout(clickPending.current.timer);
      clickPending.current = null;
    }
    void openUrl(boothItemUrl(id));
  }

  async function archiveSelected() {
    if (picks.length === 0 || blocked) return;
    setStarting(true);
    try {
      await runTask(
        'search',
        {
          files: picks.map((p) => p.source),
          baseDir: boothRoot || null,
          dryRun: false,
          forceIds: picks.map((p) => p.id),
          cookie: cookie || null,
        },
        { kind: 'search_archive', label: '检索归档' },
      );
    } catch (e) {
      setStatus(String(e));
    } finally {
      setStarting(false);
    }
  }

  const running = searching || archiving;

  return (
    <PageShell>
      <PageTitle
        title="实验检索"
        desc={<>没有 ID 的文件，按名字去 BOOTH 上碰运气。歧义（同名不同价 / 分差&lt;30）必须人工选。</>}
      />

      <Section>
        <PanelLabel>待检索</PanelLabel>
        <TextArea
          rows={3}
          placeholder={'输入本地文件名 / 完整路径 / 关键词，或直接贴文件路径\n如：LunariaPaperFan.zip  或  D:\\BOOTH\\xxx.zip  或  Lunaria Paper Fan'}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <Row>
          <AccentButton onClick={() => void search()} disabled={running || !text.trim()}>
            检索
          </AccentButton>
          <AccentButton onClick={() => void archiveSelected()} disabled={running || picks.length === 0 || blocked}>
            归档选中（{picks.length}）
          </AccentButton>
          <SecondaryButton onClick={() => setSelected({})} disabled={running}>
            清空选择
          </SecondaryButton>
          {running && (preview || archive) && (
            <SecondaryButton
              onClick={() => {
                if (previewTask?.status === 'running' && preview) void cancelTask(preview.id);
                if (archiveTask?.status === 'running' && archive) void cancelTask(archive.id);
              }}
            >
              取消
            </SecondaryButton>
          )}
          {archive && archiveTask?.status === 'done' && archiveFailed.length > 0 && (
            <SecondaryButton onClick={() => void retryFailed(archive.id)}>
              重试失败（{archiveFailed.length}）
            </SecondaryButton>
          )}
          {status && <Muted>{status}</Muted>}
        </Row>
      </Section>

      <FlexSection>
        <PanelLabel>检索结果</PanelLabel>
        <ResultList>
          {groups.map((g) => {
            const source = g.source ?? '';
            const chosen = selected[source] || (!g.ambiguous ? g.picked : undefined);
            return (
              <FileBlock key={source || g.id}>
                <FileHead>
                  <Badge kind={badgeKind(g.status)}>{g.ambiguous ? '歧义' : badgeLabel(g.status)}</Badge>
                  <span className="path">{source}</span>
                </FileHead>
                {g.ambiguous && (
                  <AmbiguousHint>同名不同价 / 分差&lt;30，请人工选择后再归档</AmbiguousHint>
                )}
                {(g.candidates ?? []).map((c) => (
                  <ResultRow
                    key={`${source}:${c.id}`}
                    $selected={chosen === c.id}
                    onClick={() => handleRowClick(source, c.id)}
                    onDoubleClick={() => handleRowDoubleClick(c.id)}
                    title={chosen === c.id ? '单击取消 / 双击浏览器核对' : '单击选中 / 双击浏览器核对'}
                  >
                    <Badge kind="ok">ID</Badge>
                    <span>{c.id}</span>
                    <span className="grow">{c.name}</span>
                    <span className="price">{formatPrice(c.price)}</span>
                    <QueueActions id={c.id} />
                  </ResultRow>
                ))}
              </FileBlock>
            );
          })}
          {errors.map((it, i) => (
            <FileHead key={`err-${i}`}>
              <Badge kind="err">失败</Badge>
              <span className="path">{it.id} {it.message}</span>
            </FileHead>
          ))}
          {groups.length === 0 && errors.length === 0 && (
            <EmptyState title="还没有检索结果" hint="输入文件名或关键词后点「检索」" />
          )}
        </ResultList>
      </FlexSection>

      {(archiving || archiveQueue.length > 0) && (
        <Section>
          <PanelLabel extra={<Counter>{archiveDone}/{archiveTotal}</Counter>}>归档进度</PanelLabel>
          <ProgressBar>
            <div style={{ width: `${archiveTotal ? (archiveDone / archiveTotal) * 100 : 0}%` }} />
          </ProgressBar>
          <ArchiveList>
            {archiveQueue.map((it, i) => (
              <ListRow key={i}>
                <Badge kind={badgeKind(it.status)}>{badgeLabel(it.status)}</Badge>
                <span>{it.id}</span>
                <span className="msg">{it.message}</span>
                <QueueActions id={it.id} path={it.path} />
              </ListRow>
            ))}
          </ArchiveList>
        </Section>
      )}
    </PageShell>
  );
}
