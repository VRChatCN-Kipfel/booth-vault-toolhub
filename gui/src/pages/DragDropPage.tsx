/**
 * 拖拽分类页：文件拖入或点选 → 提取 ID → 归档队列。
 */

import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge, PanelLabel, PageShell,
  Section, FlexSection, Row, ListRow, EmptyState, Counter, Checkbox, CheckLabel,
} from '../components/ui';
import { information } from '../components/Dialog';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUiStore } from '../store/uiStore';
import { failedItems, useLatestTask } from '../store/taskStore';
import { cancelTask, retryFailed, runTask } from '../lib/task';
import { useThemeStore, resolveMode } from '../store/themeStore';
import { THEMES } from '../theme/themes';
import { dropTile } from '../theme/motifs';
import { badgeKind, badgeLabel } from '../lib/booth';
import { invoke } from '@tauri-apps/api/core';

const DROP_KIND: Record<string, 'zhuyin' | 'liujin' | 'guwen'> = {
  zhuyin: 'zhuyin',
  liujin: 'liujin',
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

/** engine 判定项（`intake_scan` 的返回行）。 */
type ScanRow = { path: string; kind: string; id: string; reason: string };

/** 可执行归档的判定类别（sidecar / already-archived 只是跳过）。 */
function actionable(kind: string): boolean {
  return kind === 'with-id' || kind === 'search-by-name';
}

/**
 * 回推交给 engine 的根路径：文件取自身，目录取**第一级祖先**。
 *
 * 判定项是展开后的文件，而 `intake` 要的是「从哪开始扫」。用户拖进来的是
 * 目录时取它本身；拖进来的是散落文件时取它所在目录——两者的分界只能是
 * 「这个路径是不是用户直接给的」，故在入队时就把根记下来。
 */
function rootOf(path: string): string {
  const sep = path.includes('\\') ? '\\' : '/';
  const idx = path.lastIndexOf(sep);
  return idx > 0 ? path.slice(0, idx) : path;
}

/** 按判定类别分组（顺序即渲染顺序：带 ID → 按名检索 → 已在库 → 旁路文件 → 无法判定）。 */
function groupRows(rows: ScanRow[]): Record<string, ScanRow[]> {
  const out: Record<string, ScanRow[]> = {};
  for (const r of rows) {
    (out[r.kind] ??= []).push(r);
  }
  return out;
}

const KIND_LABEL: Record<string, string> = {
  'with-id': '带 ID',
  'search-by-name': '按名检索',
  'already-archived': '已在库',
  sidecar: '旁路文件',
  unknown: '无法判定',
};

/**
 * 交给 engine 判定，不在前端用正则猜。
 *
 * 此前这里只做 `hasFileProductId` 字符串匹配：路径含 7 位数字就当**一个条目**，
 * 目录名无 ID 就整包丢进「缺少 ID」，里面有什么文件完全没看（#54 现状第 3 条）。
 * 现在判定只有 engine 一份，前端按 `kind` 分组渲染。
 */
async function scan(paths: string[], out: string): Promise<ScanRow[]> {
  const res = await invoke<{ items: ScanRow[] }>('intake_scan', { dirs: paths, out: out || null });
  return res.items ?? [];
}

export function DragDropPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const [onUnknown, setOnUnknown] = useState('report');
  const [onAmbiguous, setOnAmbiguous] = useState('ask');
  const [yes, setYes] = useState(false);
  const sendToSearch = useUiStore((s) => s.sendToSearch);
  const { theme, mode, systemTheme } = useThemeStore();
  const resolved = resolveMode(mode, systemTheme);
  const pal = THEMES[theme][resolved];
  const latest = useLatestTask('organize');
  const task = latest?.task;
  const [dragging, setDragging] = useState(false);
  const [rows, setRows] = useState<ScanRow[]>([]);
  const [starting, setStarting] = useState(false);
  const [scanning, setScanning] = useState(false);
  const summarized = useRef<string | null>(null);

  const groups = groupRows(rows);
  const actionableCount = rows.filter((r) => actionable(r.kind)).length;
  const unknownRows = groups.unknown ?? [];

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

  async function addPaths(paths: string[]) {
    if (paths.length === 0) return;
    setScanning(true);
    try {
      const got = await scan(paths, boothRoot);
      setRows((prev) => {
        const seen = new Set(prev.map((r) => r.path));
        return [...prev, ...got.filter((r) => !seen.has(r.path))];
      });
    } catch (e) {
      void information('判定失败', String(e));
    } finally {
      setScanning(false);
    }
  }

  async function pickFiles() {
    const selected = await open({ multiple: true, title: '选择要归档的文件' });
    if (!selected) return;
    addPaths(Array.isArray(selected) ? selected : [selected]);
  }

  /** 交给 engine 执行：扫描 → 判定 → 归档一次完成（#54「一步到位」）。 */
  async function start() {
    const roots = [...new Set(rows.filter((r) => actionable(r.kind)).map((r) => rootOf(r.path)))];
    if (roots.length === 0) return;
    setStarting(true);
    try {
      await runTask(
        'intake',
        {
          dirs: roots,
          out: boothRoot || null,
          dryRun: false,
          onUnknown: onUnknown || null,
          onAmbiguous: onAmbiguous || null,
          confirm: yes,
          cookie: cookie || null,
        },
        { kind: 'intake' },
      );
      setRows([]);
    } catch (e) {
      void information('入库失败', String(e));
    } finally {
      setStarting(false);
    }
  }

  function clearAll() {
    setRows([]);
  }

  return (
    <PageShell>
      <PageTitle
        title="拖拽分类"
        desc="把混乱目录或散落压缩包拖进来：逐个判定身份后归档，无法判定的单独列出。"
      />

      <Section>
        <DropZone dragging={dragging} role="region" aria-label="拖入待入库的目录或文件">
          <Motif
            style={{ opacity: dragging ? 0.3 : 0.14 }}
            dangerouslySetInnerHTML={{ __html: dropTile(DROP_KIND[theme], pal.accent) }}
          />
          <div>拖入待入库的目录或压缩包</div>
          <div className="hint">
            目录会被递归展开并逐个判定（带 ID / 无 ID 压缩包 / 已在库 / 旁路文件 / 无法判定）
          </div>
        </DropZone>

        {scanning && <Counter>判定中…</Counter>}

        {Object.entries(groups)
          .filter(([kind]) => kind !== 'unknown')
          .map(([kind, list]) => (
            <div key={kind}>
              <PanelLabel>
                {KIND_LABEL[kind] ?? kind}（{list.length}）
              </PanelLabel>
              <NoList>
                {list.map((r) => (
                  <div className="path" key={r.path}>
                    {r.path} · {r.reason}
                  </div>
                ))}
              </NoList>
            </div>
          ))}

        {unknownRows.length > 0 && (
          <>
            <PanelLabel
              extra={
                <SecondaryButton onClick={() => sendToSearch(unknownRows.map((r) => r.path))}>
                  去实验检索
                </SecondaryButton>
              }
            >
              无法判定（{unknownRows.length}）
            </PanelLabel>
            <NoList>
              {unknownRows.map((r) => (
                <div className="path" key={r.path}>
                  {r.path} · {r.reason}
                </div>
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
          <AccentButton onClick={() => void start()} disabled={running || actionableCount === 0}>
            开始入库（{actionableCount}）
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
            待处理 {actionableCount} · 完成 {queue.filter((x) => x.status === 'ok').length}
          </Counter>
        </Row>
        <Row>
          <CheckLabel>
            <Checkbox
              type="checkbox"
              checked={yes}
              onChange={(e) => setYes(e.currentTarget.checked)}
            />
            已确认（按名检索需确认后才联网）
          </CheckLabel>
          <SecondaryButton
            onClick={() => setOnUnknown(onUnknown === 'report' ? 'move' : 'report')}
            disabled={running}
          >
            无法判定：{onUnknown === 'move' ? '移入 _待处理' : '仅报告'}
          </SecondaryButton>
          <SecondaryButton
            onClick={() => setOnAmbiguous(onAmbiguous === 'ask' ? 'force' : 'ask')}
            disabled={running}
          >
            歧义：{onAmbiguous === 'ask' ? '不自动归档' : '强制归档'}
          </SecondaryButton>
        </Row>
        <ProgressBar>
          <div style={{ width: `${total ? (queue.length / total) * 100 : 0}%` }} />
        </ProgressBar>
      </Section>

      <FlexSection>
        <PanelLabel>入库队列</PanelLabel>
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
            <EmptyState title="队列是空的" hint="拖入目录或文件，或点「选择文件」" />
          )}
        </QueueList>
      </FlexSection>
    </PageShell>
  );
}
