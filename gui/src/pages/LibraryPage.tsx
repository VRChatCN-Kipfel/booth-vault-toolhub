/**
 * 库存页：scan_library 列表，按 ID / 标题 / 类目筛选。
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import styled from 'styled-components';
import { invoke } from '@tauri-apps/api/core';
import {
  AccentButton, Input, ObsPanel, Badge, PageShell, PanelLabel,
  Section, FlexSection, Row, ListRow, EmptyState, Counter, Muted,
} from '../components/ui';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';

type LibraryRow = {
  id: string;
  name: string;
  category: string;
  path: string;
};

const ScanList = styled(ObsPanel)`
  flex: 1;
`;

export function LibraryPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const [items, setItems] = useState<LibraryRow[]>([]);
  const [q, setQ] = useState('');
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState('');

  const reload = useCallback(async () => {
    if (!boothRoot) {
      setErr('先在设置里填归档根目录');
      setItems([]);
      return;
    }
    setLoading(true);
    setErr('');
    try {
      const rows = await invoke<LibraryRow[]>('list_library', { base: boothRoot });
      setItems(rows);
    } catch (e) {
      setErr(String(e));
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, [boothRoot]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const view = useMemo(() => {
    const needle = q.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((it) =>
      it.id.includes(needle)
      || it.name.toLowerCase().includes(needle)
      || it.category.toLowerCase().includes(needle),
    );
  }, [items, q]);

  return (
    <PageShell>
      <PageTitle
        title="库存"
        desc="已归档的商品目录。类目取 ID 目录的父文件夹名，不联网。"
        actions={<Counter>{view.length}/{items.length}</Counter>}
      />

      <Section>
        <PanelLabel>筛选</PanelLabel>
        <Row>
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="ID / 标题 / 类目"
            style={{ flex: 1, minWidth: 200 }}
          />
          <AccentButton onClick={() => void reload()} disabled={loading || !boothRoot}>
            {loading ? '扫描中…' : '刷新'}
          </AccentButton>
          {err && <Muted>{err}</Muted>}
        </Row>
      </Section>

      <FlexSection>
        <PanelLabel>商品</PanelLabel>
        <ScanList>
          {view.map((it) => (
            <ListRow key={`${it.id}-${it.path}`}>
              <Badge kind="ok">{it.category || '未分类'}</Badge>
              <span>{it.id}</span>
              <span className="grow">{it.name}</span>
              <QueueActions id={it.id} path={it.path} />
            </ListRow>
          ))}
          {view.length === 0 && (
            <EmptyState
              title={loading ? '正在扫描…' : '库存是空的'}
              hint={boothRoot ? '点「刷新」或先下载/归档' : '先在设置里填归档根目录'}
            />
          )}
        </ScanList>
      </FlexSection>
    </PageShell>
  );
}
