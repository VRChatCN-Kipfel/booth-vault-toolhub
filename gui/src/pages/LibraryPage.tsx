/**
 * 库存页：scan_library 列表，按 ID / 标题 / 类目筛选；行内展开压缩包条目预览。
 */

import { Fragment, useCallback, useEffect, useMemo, useState } from 'react';
import styled from 'styled-components';
import { invoke } from '@tauri-apps/api/core';
import {
  AccentButton, Input, ObsPanel, Badge, PageShell, PanelLabel,
  Section, FlexSection, Row, ListRow, EmptyState, Counter, Muted, TextButton,
} from '../components/ui';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useLibraryStore } from '../store/libraryStore';

type EntryRow = {
  name: string;
  size: number;
  compressedSize: number | null;
  method: string;
};

type ArchivePreview = {
  path: string;
  name: string;
  format: string;
  totalEntries: number;
  truncated: boolean;
  entries: EntryRow[];
};

type PreviewPayload = {
  archives: ArchivePreview[];
  failures: string[];
};

const ScanList = styled(ObsPanel)`
  flex: 1;
`;

const PreviewBox = styled.div`
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 8px;
  padding: 10px 12px;
  background: var(--bvt-surface);
  border: 1px solid var(--bvt-border);
  border-radius: var(--bvt-radius);
  max-height: 420px;
  overflow-y: auto;
`;

const ArchiveBlock = styled.div`
  display: flex;
  flex-direction: column;
  gap: 6px;
`;

const ArchiveHead = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;

  .nm {
    font-weight: 600;
    word-break: break-all;
  }
`;

const EntryTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-family: var(--bvt-mono);
  font-size: var(--bvt-fz-sm);

  td {
    padding: 2px 6px;
    border-top: 1px solid var(--bvt-border);
    vertical-align: top;
  }

  td.nm {
    word-break: break-all;
  }

  td.num {
    text-align: right;
    white-space: nowrap;
    width: 1%;
  }
`;

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function LibraryPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const { items, loading, error: err, load } = useLibraryStore();
  const [q, setQ] = useState('');
  const [openPath, setOpenPath] = useState('');
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [previewErr, setPreviewErr] = useState('');
  const [previewLoading, setPreviewLoading] = useState(false);

  // 有缓存时 `load` 不会把界面打回加载态：先渲染缓存，后台静默刷新。
  useEffect(() => {
    void load(boothRoot);
  }, [boothRoot, load]);

  const reload = useCallback(() => load(boothRoot, true), [boothRoot, load]);

  const togglePreview = useCallback(async (path: string) => {
    if (openPath === path) {
      setOpenPath('');
      setPreview(null);
      setPreviewErr('');
      return;
    }
    setOpenPath(path);
    setPreview(null);
    setPreviewErr('');
    setPreviewLoading(true);
    try {
      const data = await invoke<PreviewPayload>('preview_dir', { dir: path });
      setPreview(data);
    } catch (e) {
      setPreviewErr(String(e));
    } finally {
      setPreviewLoading(false);
    }
  }, [openPath]);

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
          {!boothRoot && <Muted>先在设置里填归档根目录</Muted>}
          {boothRoot && err && <Muted>{err}</Muted>}
        </Row>
      </Section>

      <FlexSection>
        <PanelLabel>商品</PanelLabel>
        <ScanList>
          {view.map((it) => (
            <Fragment key={`${it.id}-${it.path}`}>
              <ListRow>
                <Badge kind="ok">{it.category || '未分类'}</Badge>
                <span>{it.id}</span>
                <span className="grow">{it.name}</span>
                <TextButton onClick={() => void togglePreview(it.path)}>
                  {openPath === it.path ? '收起' : '内容'}
                </TextButton>
                <QueueActions id={it.id} path={it.path} />
              </ListRow>
              {openPath === it.path && (
                <PreviewBox>
                  {previewLoading && <Muted>读取中…</Muted>}
                  {previewErr && <Muted>{previewErr}</Muted>}
                  {!previewLoading && !previewErr && preview?.archives.length === 0 && (
                    <Muted>该目录内没有可预览的压缩包</Muted>
                  )}
                  {preview?.archives.map((a) => (
                    <ArchiveBlock key={a.path}>
                      <ArchiveHead>
                        <Badge kind="run">{a.format}</Badge>
                        <span className="nm">{a.name}</span>
                        <Muted>{a.totalEntries} 条</Muted>
                      </ArchiveHead>
                      {a.entries.length > 0 && (
                        <EntryTable>
                          <tbody>
                            {a.entries.map((e, i) => (
                              <tr key={`${i}-${e.name}`}>
                                <td className="nm">{e.name}</td>
                                <td className="num">{fmtBytes(e.size)}</td>
                                <td className="num">
                                  {e.compressedSize == null ? '—' : fmtBytes(e.compressedSize)}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </EntryTable>
                      )}
                      {a.truncated && <Muted>已截断，仅列前 {a.entries.length} 条</Muted>}
                    </ArchiveBlock>
                  ))}
                  {preview?.failures.map((f) => <Muted key={f}>{f}</Muted>)}
                </PreviewBox>
              )}
            </Fragment>
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
