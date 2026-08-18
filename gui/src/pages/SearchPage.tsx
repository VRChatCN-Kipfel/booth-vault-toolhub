/**
 * 实验检索页：文件名 → score_and_pick 候选 → 歧义人工选 → 原路径+forceId 归档。
 */

import { useMemo, useState } from 'react';
import styled from 'styled-components';
import { AccentButton, SecondaryButton, TextArea, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell, Lead } from '../components/ui';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { failedItems, useLatestTask } from '../store/taskStore';
import { cancelTask, retryFailed, runTask } from '../lib/task';
import { badgeKind, badgeLabel, formatPrice } from '../lib/booth';

const Row = styled.div`
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
`;

const StatusLabel = styled.div`
  color: var(--bvt-text2);
  font-size: 13px;
`;

const ResultList = styled(ObsPanel)`
  flex: 1;
  min-height: 0;
`;

const FileBlock = styled.div`
  border-bottom: 1px solid var(--bvt-border2);
  padding: 6px 0 8px;
`;

const FileHead = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  font-size: 12px;
  color: var(--bvt-text2);
  .path { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
`;

const ResultRow = styled.div<{ selected: boolean }>`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  cursor: pointer;
  background: ${({ selected }) => (selected ? 'var(--bvt-sel-bg)' : 'transparent')};
  color: ${({ selected }) => (selected ? 'var(--bvt-sel-text)' : 'var(--bvt-text)')};
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .price { color: var(--bvt-text2); font-size: 12px; }
  &:hover { background: var(--bvt-hover); }
`;

const AmbiguousHint = styled.div`
  color: var(--bvt-warn);
  font-size: 12px;
  padding: 0 10px 4px;
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
      <PageTitle title="实验检索" />
      <Lead>没有 ID 的文件，按名字去 BOOTH 上碰运气。歧义（同名不同价 / 分差&lt;30）必须人工选。</Lead>
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
        <StatusLabel>{status}</StatusLabel>
      </Row>
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
                  selected={chosen === c.id}
                  onClick={() => toggle(source, c.id)}
                >
                  <Badge kind="ok">ID</Badge>
                  <span>{c.id}</span>
                  <span className="name">{c.name}</span>
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
          <div style={{ color: 'var(--bvt-text3)', padding: 8 }}>输入关键词后点「检索」…</div>
        )}
      </ResultList>
      {(archiving || archiveQueue.length > 0) && (
        <>
          <ProgressBar>
            <div style={{ width: `${archiveTotal ? (archiveDone / archiveTotal) * 100 : 0}%` }} />
          </ProgressBar>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {archiveQueue.map((it, i) => (
              <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13 }}>
                <Badge kind={badgeKind(it.status)}>{badgeLabel(it.status)}</Badge>
                <span>{it.id}</span>
                <span style={{ color: 'var(--bvt-text2)', flex: 1 }}>{it.message}</span>
                <QueueActions id={it.id} path={it.path} />
              </div>
            ))}
          </div>
        </>
      )}
    </PageShell>
  );
}
