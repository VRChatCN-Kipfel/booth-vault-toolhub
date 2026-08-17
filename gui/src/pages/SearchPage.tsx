/**
 * 实验检索页：文件名/路径/关键词 → 搜索候选 → 评分选优 → 归档。
 */

import { useState } from 'react';
import styled from 'styled-components';
import { AccentButton, SecondaryButton, TextArea, ObsPanel, ProgressBar, Badge, PanelLabel } from '../components/ui';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { runTask } from '../lib/task';

interface ResultItem {
  id: string;
  name: string;
  priceText: string;
}

const Page = styled.div`
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  height: 100%;
  overflow-y: auto;
`;

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

  async function search() {
    if (!text.trim()) return;
    setStatus('搜索中…');
    setResults([]);
    setRunning(true);
    try {
      await runTask(
        'search',
        {
          files: [text.trim()],
          baseDir: boothRoot || null,
          dryRun: true,
          cookie: cookie || null,
        },
        (evt) => {
          if (evt.type === 'itemDone') {
            setResults((r) => [...r, { id: String(evt.id ?? ''), name: String(evt.message ?? ''), priceText: '' }]);
          } else if (evt.type === 'finished' || evt.type === 'cancelled') {
            setRunning(false);
            setStatus('搜索完成');
          }
        },
      );
    } catch (e) {
      setStatus(String(e));
      setRunning(false);
    }
  }

  function toggleSelect(id: string) {
    setSelected((sel) => (sel.includes(id) ? sel.filter((x) => x !== id) : [...sel, id]));
  }

  async function archiveSelected() {
    if (selected.length === 0) return;
    setArchiving(true);
    setTotal(selected.length);
    try {
      await runTask(
        'search',
        {
          files: selected,
          baseDir: boothRoot || null,
          dryRun: false,
          cookie: cookie || null,
        },
        (evt) => {
          if (evt.type === 'itemDone') {
            setQueue((q) => [...q, { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'ok' }]);
          } else if (evt.type === 'itemError') {
            setQueue((q) => [...q, { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'err' }]);
          } else if (evt.type === 'finished' || evt.type === 'cancelled') {
            setArchiving(false);
            setSelected([]);
          }
        },
      );
    } catch (e) {
      setQueue([{ id: '-', message: String(e), status: 'err' }]);
      setArchiving(false);
    }
  }

  return (
    <Page>
      <PageTitle title="实验检索" />
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
        <AccentButton onClick={() => void archiveSelected()} disabled={archiving || selected.length === 0}>
          归档选中（{selected.length}）
        </AccentButton>
        <SecondaryButton onClick={() => setSelected([])} disabled={archiving}>
          清空
        </SecondaryButton>
        <StatusLabel>{status}</StatusLabel>
      </Row>
      <PanelLabel>检索结果（可多选）</PanelLabel>
      <ResultList>
        {results.map((it) => (
          <ResultRow
            key={it.id}
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
    </Page>
  );
}
