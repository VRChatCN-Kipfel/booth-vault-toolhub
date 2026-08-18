/**
 * 批量链接页：粘贴含 BOOTH 链接的文本 → 解析 ID → 下载归档。
 */

import { useState } from 'react';
import styled from 'styled-components';
import { AccentButton, SecondaryButton, TextArea, Input, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell, Lead } from '../components/ui';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { failedItems, useLatestTask } from '../store/taskStore';
import { cancelTask, retryFailed, runTask } from '../lib/task';
import { badgeKind, badgeLabel } from '../lib/booth';

const Row = styled.div`
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
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

const Checkbox = styled.input`
  width: 16px;
  height: 16px;
  accent-color: var(--bvt-accent);
`;

export function LinksPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const latest = useLatestTask('download');
  const task = latest?.task;
  const [text, setText] = useState('');
  const [shop, setShop] = useState('');
  const [dryRun, setDryRun] = useState(false);
  const [limit, setLimit] = useState('');
  const [starting, setStarting] = useState(false);
  const [hint, setHint] = useState('');

  const running = starting || task?.status === 'running';
  const queue = task?.items ?? [];
  const total = task?.total ?? 0;
  const done = task?.done ?? 0;
  const failed = task ? failedItems(task) : [];

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
    if (ids.length === 0 && !shop.trim()) {
      setHint('未解析到有效商品 ID，也未填写店铺');
      return;
    }
    setHint('');
    setStarting(true);
    try {
      const n = Number(limit);
      await runTask('download', {
        items: text.trim() ? [text] : [],
        shop: shop.trim() || null,
        out: boothRoot || null,
        limit: Number.isFinite(n) && n > 0 ? n : null,
        dryRun,
        cookie: cookie || null,
      });
    } catch (e) {
      setHint(String(e));
    } finally {
      setStarting(false);
    }
  }

  return (
    <PageShell>
      <PageTitle title="批量链接" />
      <Lead>把聊天记录或散链贴进来，按商品 ID 下载免费文件。也可填店铺 URL 整店拉取。</Lead>
      <TextArea
        rows={6}
        placeholder={'粘贴含 BOOTH 商品链接的文本（如聊天记录）…\n支持全 locale 链接：https://booth.pm/ja/items/1234567\n也支持裸 ID：1234567'}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <Row>
        <Input
          value={shop}
          onChange={(e) => setShop(e.target.value)}
          placeholder="店铺 URL 或子域名（可选）"
          style={{ flex: 1, minWidth: 180 }}
          disabled={running}
        />
        <Input
          value={limit}
          onChange={(e) => setLimit(e.target.value.replace(/[^\d]/g, ''))}
          placeholder="limit"
          style={{ width: 88 }}
          disabled={running}
        />
        <label style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--bvt-text2)', fontSize: 13 }}>
          <Checkbox type="checkbox" checked={dryRun} onChange={(e) => setDryRun(e.target.checked)} disabled={running} />
          预览（不下载）
        </label>
      </Row>
      <Row>
        <AccentButton onClick={() => void start()} disabled={running || (!text.trim() && !shop.trim())}>
          {dryRun ? '预览' : '开始归档'}
        </AccentButton>
        <SecondaryButton onClick={() => { setText(''); setShop(''); setLimit(''); }} disabled={running}>
          清空
        </SecondaryButton>
        {running && latest && (
          <SecondaryButton onClick={() => void cancelTask(latest.id)}>取消</SecondaryButton>
        )}
        {latest && task?.status === 'done' && failed.length > 0 && (
          <SecondaryButton onClick={() => void retryFailed(latest.id)}>重试失败（{failed.length}）</SecondaryButton>
        )}
        <CountLabel>{done}/{total}</CountLabel>
        {hint && <CountLabel>{hint}</CountLabel>}
      </Row>
      <ProgressBar>
        <div style={{ width: `${total ? (done / total) * 100 : 0}%` }} />
      </ProgressBar>
      <PanelLabel>下载队列</PanelLabel>
      <QueueList>
        {queue.map((it, i) => (
          <QueueRow key={`${it.id}-${i}`}>
            <Badge kind={badgeKind(it.status)}>{badgeLabel(it.status)}</Badge>
            <span>{it.id}</span>
            <span className="msg">{it.message}</span>
            <QueueActions id={it.id} />
          </QueueRow>
        ))}
        {queue.length === 0 && (
          <div style={{ color: 'var(--bvt-text3)', padding: 8 }}>
            {hint || '等待解析链接…'}
          </div>
        )}
      </QueueList>
    </PageShell>
  );
}
