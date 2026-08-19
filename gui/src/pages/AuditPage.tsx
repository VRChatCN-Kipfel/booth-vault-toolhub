/**
 * 目录巡检页：三件套完整性巡检 + 修复 + 版本巡检 + 错位纠正。
 */

import { useMemo, useState } from 'react';
import styled from 'styled-components';
import {
  AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge, PageShell, PanelLabel,
  Section, Row, ListRow, EmptyState, Checkbox, Muted,
} from '../components/ui';
import { information } from '../components/Dialog';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUiStore } from '../store/uiStore';
import { useLatestTask, type TaskKind } from '../store/taskStore';
import { cancelTask, runTask } from '../lib/task';
import { badgeKind, badgeLabel, extractBoothId } from '../lib/booth';
import { isLinux } from '../lib/platform';

const FilterBtn = styled.button<{ $on: boolean }>`
  height: 26px;
  padding: 0 var(--bvt-s3);
  border: 1px solid ${({ $on }) => ($on ? 'transparent' : 'var(--bvt-border)')};
  border-radius: var(--bvt-pill);
  background: ${({ $on }) => ($on ? 'var(--bvt-accent-light)' : 'transparent')};
  color: ${({ $on }) => ($on ? 'var(--bvt-accent-deep)' : 'var(--bvt-text2)')};
  font: inherit;
  font-size: var(--bvt-fz-sm);
  cursor: pointer;
  transition: background-color 0.16s var(--bvt-ease), border-color 0.16s var(--bvt-ease);
  &:hover { background: ${({ $on }) => ($on ? 'var(--bvt-accent-light)' : 'var(--bvt-hover)')}; }
`;

const ScanList = styled(ObsPanel)`
  max-height: 320px;
  min-height: 140px;
`;

type Filter = 'all' | 'missing' | 'updateable' | 'mismatch';

export function AuditPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const cookie = useAppConfigStore((s) => s.cookie);
  const setStatus = useUiStore((s) => s.setStatus);
  const scan = useLatestTask('audit');
  const fix = useLatestTask('audit_fix');
  const version = useLatestTask('version_audit');
  const backfill = useLatestTask('backfill_free');
  const mismatch = useLatestTask('mismatch_audit');
  const fixingMis = useLatestTask('fix_mismatch');
  const [starting, setStarting] = useState<TaskKind | null>(null);
  const [filter, setFilter] = useState<Filter>('all');
  const [picked, setPicked] = useState<string[]>([]);
  const linux = isLinux();

  const scanTask = scan?.task;
  const fixTask = fix?.task;
  const verTask = version?.task;
  const backfillTask = backfill?.task;
  const misTask = mismatch?.task;
  const fixMisTask = fixingMis?.task;

  const scanning = starting === 'audit' || scanTask?.status === 'running';
  const fixing = starting === 'audit_fix' || fixTask?.status === 'running';
  const versioning = starting === 'version_audit' || verTask?.status === 'running';
  const backfilling = starting === 'backfill_free' || backfillTask?.status === 'running';
  const mismatching = starting === 'mismatch_audit' || misTask?.status === 'running';
  const fixingMismatch = starting === 'fix_mismatch' || fixMisTask?.status === 'running';

  const scanItems = scanTask?.items ?? [];
  const missingItems = scanItems.filter((i) => i.status !== 'ok');
  const full = scanTask && scanTask.status !== 'running' ? scanItems.length - missingItems.length : 0;
  const missing = missingItems.length;
  const verItems = verTask?.items ?? [];
  const misItems = useMemo(() => {
    const fromDetect = misTask?.items ?? [];
    const fromFix = (fixMisTask?.items ?? []).concat(
      (fixMisTask?.logs ?? []).map((line) => ({ id: '', message: line, status: 'ok' })),
    );
    return fromDetect.length ? fromDetect : fromFix;
  }, [misTask, fixMisTask]);

  const verPct = verTask && verTask.total > 0 ? (verTask.done / verTask.total) * 100 : verTask?.status === 'done' ? 100 : 0;

  async function launch(kind: TaskKind, cmd: string, args: Record<string, unknown>, label?: string) {
    if (!boothRoot) return;
    setStarting(kind);
    try {
      await runTask(cmd, args, { kind, label });
    } catch (e) {
      setStatus(String(e));
    } finally {
      setStarting(null);
    }
  }

  function togglePick(id: string) {
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));
  }

  const showScan = filter === 'all' || filter === 'missing';
  const showMis = filter === 'all' || filter === 'mismatch';
  const showVer = filter === 'all' || filter === 'updateable';
  const scanView = filter === 'missing' ? missingItems : scanItems;

  return (
    <PageShell>
      <PageTitle
        title="目录巡检"
        desc="封面、图标、目录名，一次扫完再决定修不修。"
        actions={
          <Row>
            {(['all', 'missing', 'updateable', 'mismatch'] as Filter[]).map((k) => (
              <FilterBtn key={k} type="button" $on={filter === k} onClick={() => setFilter(k)}>
                {k === 'all' ? '全部' : k === 'missing' ? `缺失 ${missing}` : k === 'updateable' ? `可更新 ${verItems.length}` : `错位 ${misTask?.failed ?? misItems.length}`}
              </FilterBtn>
            ))}
          </Row>
        }
      />

      {showScan && (
        <Section>
          <PanelLabel
            extra={
              <Row>
                <Badge kind="ok">完整 {full}</Badge>
                <Badge kind="warn">缺失 {missing}</Badge>
              </Row>
            }
          >
            三件套完整性
          </PanelLabel>
          <Row>
            <AccentButton
              onClick={() => void launch('audit', 'audit', { base: boothRoot, dryRun: true, noFix: true })}
              disabled={scanning || !boothRoot}
            >
              {scanning ? '巡检中…' : '开始巡检'}
            </AccentButton>
            {scanning && scan && (
              <SecondaryButton onClick={() => void cancelTask(scan.id)}>取消</SecondaryButton>
            )}
            {!linux && (
              <>
                <SecondaryButton
                  onClick={() => void launch('audit_fix', 'audit', { base: boothRoot, dryRun: false, noFix: false }, '修复三件套')}
                  disabled={fixing || missing === 0}
                >
                  修复缺失三件套
                </SecondaryButton>
                {fixing && fix && (
                  <SecondaryButton onClick={() => void cancelTask(fix.id)}>取消</SecondaryButton>
                )}
              </>
            )}
            {linux && <Muted>当前系统无文件夹图标三件套，修复已隐藏。</Muted>}
            <Muted>
              {scanTask?.status === 'done'
                ? `共 ${scanItems.length} 件，${missing} 件缺失三件套`
                : scanning
                  ? '巡检中…'
                  : ''}
            </Muted>
          </Row>
          <ScanList>
            {scanView.map((it, i) => (
              <ListRow key={`${it.id}-${i}`}>
                <Badge kind={badgeKind(it.status)}>{it.status === 'ok' ? '完整' : '缺失'}</Badge>
                <span>{it.id}</span>
                <span className="msg">{it.message}</span>
                <QueueActions id={it.id} path={it.path} />
              </ListRow>
            ))}
            {scanView.length === 0 && (
              <EmptyState title="还没有巡检结果" hint="点「开始巡检」扫描归档根目录" />
            )}
          </ScanList>
        </Section>
      )}

      {showMis && (
        <Section>
          <PanelLabel>错位纠正</PanelLabel>
          <Muted>联网比对官方分类。可勾选后只纠正选中项；不选则全量。</Muted>
          <Row>
            <AccentButton
              onClick={() => void launch('mismatch_audit', 'mismatch_audit', { base: boothRoot })}
              disabled={scanning || mismatching || !boothRoot}
            >
              {mismatching ? '检测中…' : '检测错位'}
            </AccentButton>
            {mismatching && mismatch && (
              <SecondaryButton onClick={() => void cancelTask(mismatch.id)}>取消</SecondaryButton>
            )}
            <AccentButton
              onClick={() =>
                void launch('fix_mismatch', 'fix_mismatch', {
                  base: boothRoot,
                  ids: picked.length ? picked : null,
                })
              }
              disabled={fixingMismatch || !boothRoot}
            >
              {fixingMismatch ? '纠正中…' : picked.length ? `纠正选中（${picked.length}）` : '一键纠正错位'}
            </AccentButton>
            {fixingMismatch && fixingMis && (
              <SecondaryButton onClick={() => void cancelTask(fixingMis.id)}>取消</SecondaryButton>
            )}
            <Muted>
              {fixMisTask?.status === 'done'
                ? `纠正完成：${fixMisTask.done} 件`
                : misTask?.status === 'done'
                  ? misTask.failed > 0
                    ? `检测到 ${misTask.failed} 件错位`
                    : '无错位'
                  : ''}
            </Muted>
          </Row>
          <ScanList>
            {misItems.map((it, i) => {
              const id = extractBoothId(it.id) ?? '';
              return (
                <ListRow key={`${it.id}-${i}`}>
                  {id && (
                    <Checkbox
                      checked={picked.includes(id)}
                      onChange={() => togglePick(id)}
                      aria-label={`选中 ${id}`}
                    />
                  )}
                  <Badge kind={badgeKind(it.status || 'warn')}>{it.status === 'ok' ? '已纠正' : badgeLabel(it.status || 'warn')}</Badge>
                  <span>{it.id}</span>
                  <span className="msg">{it.message}</span>
                  <QueueActions id={it.id} path={it.path} />
                </ListRow>
              );
            })}
            {misItems.length === 0 && (
              <EmptyState title="还没有检测错位" hint="点「检测错位」比对官方分类" />
            )}
          </ScanList>
        </Section>
      )}

      {showVer && (
        <Section>
          <PanelLabel>版本巡检</PanelLabel>
          <Muted>比对远程免费文件名与本地文件名版本。付费缺口只开商品页，不自动下。</Muted>
          <Row>
            <AccentButton
              onClick={() => void launch('version_audit', 'version_audit', { base: boothRoot })}
              disabled={versioning || !boothRoot}
            >
              {versioning ? '版本巡检中…' : '开始版本巡检'}
            </AccentButton>
            {versioning && version && (
              <SecondaryButton onClick={() => void cancelTask(version.id)}>取消</SecondaryButton>
            )}
            <AccentButton
              onClick={() => {
                const folders = verItems.map((i) => i.path).filter((p): p is string => Boolean(p));
                if (folders.length === 0) return;
                if (!cookie.trim()) {
                  void information('需要 Cookie', '免费文件补全需要登录 Cookie，请到设置页填写。');
                  return;
                }
                void launch('backfill_free', 'backfill_free', {
                  folders,
                  cookie: cookie || null,
                });
              }}
              disabled={backfilling || verItems.length === 0}
            >
              {backfilling ? '补全中…' : `补免费文件（${verItems.length}）`}
            </AccentButton>
            {backfilling && backfill && (
              <SecondaryButton onClick={() => void cancelTask(backfill.id)}>取消</SecondaryButton>
            )}
            <Muted>
              {verTask?.status === 'done'
                ? (verTask.updateable || verItems.length) > 0
                  ? `发现 ${verTask.updateable || verItems.length} 件可更新`
                  : '未检测到更高版本'
                : versioning
                  ? '版本巡检中…'
                  : ''}
            </Muted>
          </Row>
          <ProgressBar>
            <div style={{ width: `${verPct}%` }} />
          </ProgressBar>
          <ScanList>
            {verItems.map((it, i) => (
              <ListRow key={`${it.id}-${i}`}>
                <Badge kind="warn">可更新</Badge>
                <span>{it.id}</span>
                <span className="msg">{it.message}</span>
                <QueueActions id={it.id} path={it.path} />
                {it.path && (
                  <SecondaryButton
                    onClick={() => {
                      if (!cookie.trim()) {
                        void information('需要 Cookie', '免费文件补全需要登录 Cookie，请到设置页填写。');
                        return;
                      }
                      void launch('backfill_free', 'backfill_free', {
                        folders: [it.path],
                        cookie: cookie || null,
                      });
                    }}
                    disabled={backfilling}
                  >
                    补免费文件
                  </SecondaryButton>
                )}
              </ListRow>
            ))}
            {verItems.length === 0 && (
              <EmptyState title="还没有版本巡检结果" hint="点「开始版本巡检」比对官方版本号" />
            )}
          </ScanList>
        </Section>
      )}
    </PageShell>
  );
}
