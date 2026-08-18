/**
 * 实验检索页：文件名/路径/关键词 → 搜索候选 → 评分选优 → 归档。
 */

import { useRef, useState } from 'react';
import styled from 'styled-components';
import { invoke } from '@tauri-apps/api/core';
import { AccentButton, SecondaryButton, TextArea, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell, Lead } from '../components/ui';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { runTask, type ProgressEvt } from '../lib/task';

interface ResultItem {
  id: string;
  name: string;
  priceText: string;
  sourcePath: string;
}

const Row = styled.div`
  display: flex;
  gap: 10px;
  align-items: center;
`;

const StatusLabel = styled.div`
  color: var(--bvt-text2);
  font-size: 13px;
`;

const ResultList = styled(ObsPanel)`
  flex: 1;
  min-height: 0;
`;

const ResultRow = styled.div<{ selected: boolean }>`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  cursor: pointer;
  background: ${({ selected }) => (selected ? 'var(--bvt-sel-bg)' : 'transparent')};
  color: ${({ selected }) => (selected ? 'var(--bvt-sel-text)' : 'var(--bvt-text)')};
  border-bottom: 1px solid var(--bvt-border2);
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .price { color: var(--bvt-text2); font-size: 12px; }
  &:hover { background: var(--bvt-hover); }
`;

function parseFiles(blob: string): string[] {
  return blob.split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
}

export function SearchPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const [text, setText] = useState('');
  const [results, setResults] = useState<ResultItem[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [status, setStatus] = useState('');
  const [queue, setQueue] = useState<Array<{ id: string; message: string; status: 'ok' | 'err' }>>([]);
  const [running, setRunning] = useState(false);
  const [archiving, setArchiving] = useState(false);
  const [total, setTotal] = useState(0);
  const taskIdRef = useRef<string | null>(null);
  const cancelRequestedRef = useRef(false);

  function runUntilDone(
    cmd: string,
    args: Record<string, unknown>,
    onEvent: (evt: ProgressEvt) => void,
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      void runTask(cmd, args, (evt) => {
        onEvent(evt);
        if (evt.type === 'finished' || evt.type === 'cancelled') {
          taskIdRef.current = null;
          resolve();
        }
      })
        .then((id) => {
          taskIdRef.current = id;
          if (cancelRequestedRef.current) {
            void invoke('cancel_task', { taskId: id });
          }
        })
        .catch(reject);
    });
  }

  async function search() {
    const files = parseFiles(text);
    if (files.length === 0) return;
    cancelRequestedRef.current = false;
    setStatus('搜索中…');
    setResults([]);
    setSelected([]);
    setRunning(true);
    try {
      for (const file of files) {
        if (cancelRequestedRef.current) break;
        await runUntilDone(
          'search',
          {
            files: [file],
            baseDir: boothRoot || null,
            dryRun: true,
            cookie: cookie || null,
          },
          (evt) => {
            if (evt.type === 'itemDone') {
              setResults((r) => [
                ...r,
                {
                  id: String(evt.id ?? ''),
                  name: String(evt.message ?? ''),
                  priceText: '',
                  sourcePath: file,
                },
              ]);
            }
          },
        );
      }
      setStatus(cancelRequestedRef.current ? '已取消' : '搜索完成');
    } catch (e) {
      setStatus(String(e));
    } finally {
      setRunning(false);
      taskIdRef.current = null;
    }
  }

  function toggleSelect(id: string) {
    setSelected((sel) => (sel.includes(id) ? sel.filter((x) => x !== id) : [...sel, id]));
  }

  async function archiveSelected() {
    const items = results.filter((r) => selected.includes(r.id));
    if (items.length === 0) return;
    cancelRequestedRef.current = false;
    setArchiving(true);
    setTotal(items.length);
    try {
      for (const it of items) {
        if (cancelRequestedRef.current) break;
        await runUntilDone(
          'search',
          {
            files: [it.sourcePath],
            baseDir: boothRoot || null,
            dryRun: false,
            forceId: it.id,
            cookie: cookie || null,
          },
          (evt) => {
            if (evt.type === 'itemDone') {
              setQueue((q) => [...q, { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'ok' }]);
            } else if (evt.type === 'itemError') {
              setQueue((q) => [...q, { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'err' }]);
            }
          },
        );
      }
      setSelected([]);
    } catch (e) {
      setQueue([{ id: '-', message: String(e), status: 'err' }]);
    } finally {
      setArchiving(false);
      taskIdRef.current = null;
    }
  }

  async function cancel() {
    cancelRequestedRef.current = true;
    if (taskIdRef.current) {
      await invoke('cancel_task', { taskId: taskIdRef.current });
    }
    setRunning(false);
    setArchiving(false);
  }

  return (
    <PageShell>
      <PageTitle title="实验检索" />
      <Lead>没有 ID 的文件，按名字去 BOOTH 上碰运气。</Lead>
      <TextArea
        rows={3}
        placeholder={'输入本地文件名 / 完整路径 / 关键词，或直接贴文件路径\n如：LunariaPaperFan.zip  或  D:\\BOOTH\\xxx.zip  或  Lunaria Paper Fan'}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <Row>
        <AccentButton onClick={() => void search()} disabled={running || archiving || !text.trim()}>
          检索
        </AccentButton>
        <AccentButton onClick={() => void archiveSelected()} disabled={archiving || running || selected.length === 0}>
          归档选中（{selected.length}）
        </AccentButton>
        <SecondaryButton onClick={() => setSelected([])} disabled={archiving || running}>
          清空
        </SecondaryButton>
        {(running || archiving) && <SecondaryButton onClick={() => void cancel()}>取消</SecondaryButton>}
        <StatusLabel>{status}</StatusLabel>
      </Row>
      <PanelLabel>检索结果（可多选）</PanelLabel>
      <ResultList>
        {results.map((it) => (
          <ResultRow
            key={`${it.sourcePath}:${it.id}`}
            selected={selected.includes(it.id)}
            onClick={() => toggleSelect(it.id)}
          >
            <Badge kind="ok">ID</Badge>
            <span>{it.id}</span>
            <span className="name">{it.name}</span>
            <span className="price">{it.priceText}</span>
          </ResultRow>
        ))}
        {results.length === 0 && (
          <div style={{ color: 'var(--bvt-text3)', padding: 8 }}>输入关键词后点「检索」…</div>
        )}
      </ResultList>
      {(archiving || queue.length > 0) && (
        <>
          <ProgressBar>
            <div style={{ width: `${total ? (queue.length / total) * 100 : 0}%` }} />
          </ProgressBar>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {queue.map((it, i) => (
              <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13 }}>
                <Badge kind={it.status}>{it.status}</Badge>
                <span>{it.id}</span>
                <span style={{ color: 'var(--bvt-text2)' }}>{it.message}</span>
              </div>
            ))}
          </div>
        </>
      )}
    </PageShell>
  );
}
