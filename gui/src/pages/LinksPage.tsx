/**
 * 批量链接页：粘贴含 BOOTH 链接的文本 → 解析 ID → 下载归档。
 * 进度走 Tauri Channel，支持取消。
 */

import { useRef, useState } from 'react';
import styled from 'styled-components';
import { invoke } from '@tauri-apps/api/core';
import { AccentButton, SecondaryButton, TextArea, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell, Lead } from '../components/ui';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { runTask } from '../lib/task';

interface QueueItem {
  id: string;
  message: string;
  status: 'ok' | 'warn' | 'err' | 'run';
}

const Row = styled.div`
  display: flex;
  gap: 10px;
  align-items: center;
`;

const CountLabel = styled.span`
  color: var(--bvt-text2);
  font-size: 13px;
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

export function LinksPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const [text, setText] = useState('');
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [running, setRunning] = useState(false);
  const [total, setTotal] = useState(0);
  const [done, setDone] = useState(0);
  const taskIdRef = useRef<string | null>(null);

  // 解析文本中的商品 ID（对齐旧版 URL_RE + BARE_ID_RE）。
  function parseIds(blob: string): string[] {
    const urlRe = /booth\.pm\/[^/\s]+\/items\/(\d{7})/g;
    const bareRe = /(?<!\d)\d{7}(?!\d)/g;
    const ids: string[] = [];
    for (const m of blob.matchAll(urlRe)) {
      if (!ids.includes(m[1])) ids.push(m[1]);
    }
    for (const m of blob.matchAll(bareRe)) {
      if (!ids.includes(m[0])) ids.push(m[0]);
    }
    return ids;
  }

  async function start() {
    const ids = parseIds(text);
    if (ids.length === 0) {
      setQueue([{ id: '-', message: '未解析到有效商品 ID', status: 'warn' }]);
      return;
    }
    setQueue([]);
    setTotal(ids.length);
    setDone(0);
    setRunning(true);

    try {
      const taskId = await runTask(
        'download',
        {
          items: [text],
          out: boothRoot || null,
          dryRun: false,
          cookie: cookie || null,
        },
        (evt) => {
          if (evt.type === 'taskStarted' && typeof evt.total === 'number') {
            setTotal(evt.total);
          } else if (evt.type === 'itemDone') {
            const st = evt.status === 'warn' ? 'warn' : 'ok';
            setQueue((q) => [
              ...q,
              { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: st },
            ]);
          } else if (evt.type === 'itemError') {
            setQueue((q) => [
              ...q,
              { id: String(evt.id ?? ''), message: String(evt.message ?? ''), status: 'err' },
            ]);
          } else if (evt.type === 'progress' && typeof evt.done === 'number') {
            setDone(evt.done);
          } else if (evt.type === 'finished' || evt.type === 'cancelled') {
            setRunning(false);
            taskIdRef.current = null;
          }
        },
      );
      taskIdRef.current = taskId;
    } catch (e) {
      setQueue([{ id: '-', message: String(e), status: 'err' }]);
      setRunning(false);
    }
  }

  async function cancel() {
    if (taskIdRef.current) {
      await invoke('cancel_task', { taskId: taskIdRef.current });
      setRunning(false);
    }
  }

  return (
    <PageShell>
      <PageTitle title="批量链接" />
      <Lead>把聊天记录或散链贴进来，按商品 ID 下载免费文件。</Lead>
      <TextArea
        rows={6}
        placeholder={'粘贴含 BOOTH 商品链接的文本（如聊天记录）…\n支持全 locale 链接：https://booth.pm/ja/items/1234567\n也支持裸 ID：1234567'}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <Row>
        <AccentButton onClick={() => void start()} disabled={running || !text.trim()}>
          开始归档
        </AccentButton>
        <SecondaryButton onClick={() => setText('')} disabled={running}>
          清空
        </SecondaryButton>
        {running && <SecondaryButton onClick={() => void cancel()}>取消</SecondaryButton>}
        <CountLabel>{done}/{total}</CountLabel>
      </Row>
      <ProgressBar>
        <div style={{ width: `${total ? (done / total) * 100 : 0}%` }} />
      </ProgressBar>
      <PanelLabel>下载队列</PanelLabel>
      <QueueList>
        {queue.map((it, i) => (
          <QueueRow key={i}>
            <Badge kind={it.status}>{it.status}</Badge>
            <span>{it.id}</span>
            <span className="msg">{it.message}</span>
          </QueueRow>
        ))}
        {queue.length === 0 && (
          <div style={{ color: 'var(--bvt-text3)', padding: 8 }}>
            等待解析链接…
          </div>
        )}
      </QueueList>
    </PageShell>
  );
}
