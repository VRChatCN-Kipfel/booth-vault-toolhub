/**
 * 目录巡检页：三件套完整性巡检 + 修复 + 版本巡检 + 错位纠正。
 */

import { useMemo, useState } from 'react';
import styled from 'styled-components';
import {
  AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge, PageShell, Lead,
} from '../components/ui';
import { QueueActions } from '../components/QueueActions';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUiStore } from '../store/uiStore';
import { useLatestTask, type TaskKind } from '../store/taskStore';
import { cancelTask, runTask } from '../lib/task';
import { badgeKind, badgeLabel, extractBoothId } from '../lib/booth';
import { isLinux } from '../lib/platform';

const Row = styled.div`
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
`;

const StatText = styled.span`
  color: var(--bvt-text2);
  font-size: 13px;
  font-family: inherit;
`;

const Muted = styled.div`
  color: var(--bvt-text3);
  font-size: 12px;
  line-height: 1.6;
`;

const BadgeRow = styled.div`
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
`;

const FilterBtn = styled.button<{ $on: boolean }>`
  border: 1px solid ${({ $on }) => ($on ? 'var(--bvt-accent)' : 'var(--bvt-border2)')};
  background: ${({ $on }) => ($on ? 'var(--bvt-accent-light)' : 'transparent')};
  color: ${({ $on }) => ($on ? 'var(--bvt-accent-deep)' : 'var(--bvt-text2)')};
  border-radius: var(--bvt-radius, 2px);
  font: inherit;
  font-size: 12px;
  padding: 3px 8px;
  cursor: pointer;
`;

const SectionGap = styled.div`
  height: 8px;
`;

const ScanList = styled(ObsPanel)`
  min-height: 160px;
  font-size: 13px;
`;

const ListRow = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--bvt-border2);
  color: var(--bvt-text);
  .msg { color: var(--bvt-text2); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
`;

const Checkbox = styled.input`
  width: 14px;
  height: 14px;
  accent-color: var(--bvt-accent);
`;

type Filter = 'all' | 'missing' | 'updateable' | 'mismatch';

export function AuditPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const setStatus = useUiStore((s) => s.setStatus);
  const scan = useLatestTask('audit');
  const fix = useLatestTask('audit_fix');
  const version = useLatestTask('version_audit');
  const mismatch = useLatestTask('mismatch_audit');
  const fixingMis = useLatestTask('fix_mismatch');
  const [starting, setStarting] = useState<TaskKind | null>(null);
  const [filter, setFilter] = useState<Filter>('all');
  const [picked, setPicked] = useState<string[]>([]);
  const linux = isLinux();

  const scanTask = scan?.task;
  const fixTask = fix?.task;
  const verTask = version?.task;
  const misTask = mismatch?.task;
  const fixMisTask = fixingMis?.task;

  const scanning = starting === 'audit' || scanTask?.status === 'running';
  const fixing = starting === 'audit_fix' || fixTask?.status === 'running';
  const versioning = starting === 'version_audit' || verTask?.status === 'running';
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
      <PageTitle title="目录巡检" />
      <Lead>封面、图标、目录名，一次扫完再决定修不修。</Lead>

      <BadgeRow>
        {(['all', 'missing', 'updateable', 'mismatch'] as Filter[]).map((k) => (
          <FilterBtn key={k} $on={filter === k} onClick={() => setFilter(k)}>
            {k === 'all' ? '全部' : k === 'missing' ? `缺失 ${missing}` : k === 'updateable' ? `可更新 ${verItems.length}` : `错位 ${misTask?.failed ?? misItems.length}`}
          </FilterBtn>
        ))}
      </BadgeRow>

      {showScan && (
        <>
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
            <StatText>
              {scanTask?.status === 'done'
                ? `共 ${scanItems.length} 件，${missing} 件缺失三件套`
                : scanning
                  ? '巡检中…'
                  : ''}
            </StatText>
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
            {scanView.length === 0 && <ListRow>等待巡检…</ListRow>}
          </ScanList>
          <BadgeRow>
            <Badge kind="ok">完整 {full}</Badge>
            <Badge kind="warn">缺失 {missing}</Badge>
          </BadgeRow>
        </>
      )}

      {showMis && (
        <>
          <SectionGap />
          <Muted>错位纠正：联网比对官方分类。可勾选后只纠正选中项；不选则全量。</Muted>
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
            <StatText>
              {fixMisTask?.status === 'done'
                ? `纠正完成：${fixMisTask.done} 件`
                : misTask?.status === 'done'
                  ? misTask.failed > 0
                    ? `检测到 ${misTask.failed} 件错位`
                    : '无错位 ✓'
                  : ''}
            </StatText>
          </Row>
          <ScanList>
            {misItems.map((it, i) => {
              const id = extractBoothId(it.id) ?? '';
              return (
                <ListRow key={`${it.id}-${i}`}>
                  {id && (
                    <Checkbox
                      type="checkbox"
                      checked={picked.includes(id)}
                      onChange={() => togglePick(id)}
                    />
                  )}
                  <Badge kind={badgeKind(it.status || 'warn')}>{it.status === 'ok' ? '已纠正' : badgeLabel(it.status || 'warn')}</Badge>
                  <span>{it.id}</span>
                  <span className="msg">{it.message}</span>
                  <QueueActions id={it.id} path={it.path} />
                </ListRow>
              );
            })}
          </ScanList>
        </>
      )}

      {showVer && (
        <>
          <SectionGap />
          <Muted>实验性功能：联网比对官方商品名中的版本号，可能不准确，更新前请人工核对。</Muted>
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
            <StatText>
              {verTask?.status === 'done'
                ? (verTask.updateable || verItems.length) > 0
                  ? `发现 ${verTask.updateable || verItems.length} 件可更新`
                  : '未检测到更高版本'
                : versioning
                  ? '版本巡检中…'
                  : ''}
            </StatText>
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
              </ListRow>
            ))}
          </ScanList>
        </>
      )}
    </PageShell>
  );
}
