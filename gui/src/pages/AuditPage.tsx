/**
 * 目录巡检页：三件套完整性巡检 + 修复 + 版本巡检 + 错位纠正。
 * 对齐原版 audit_page.py 的完整功能。
 */

import { useRef, useState } from 'react';
import styled from 'styled-components';
import { invoke } from '@tauri-apps/api/core';
import {
  AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge, PageShell, Lead,
} from '../components/ui';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUiStore } from '../store/uiStore';
import { runTask } from '../lib/task';

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
`;

const SectionGap = styled.div`
  height: 8px;
`;

const ScanList = styled(ObsPanel)`
  min-height: 200px;
  font-size: 13px;
`;

const VerList = styled(ObsPanel)`
  min-height: 150px;
  font-size: 13px;
`;

const MismatchList = styled(ObsPanel)`
  min-height: 120px;
  font-size: 13px;
`;

const ListRow = styled.div`
  padding: 4px 8px;
  border-bottom: 1px solid var(--bvt-border2);
  color: var(--bvt-text);
`;

export function AuditPage() {
  const boothRoot = useAppConfigStore((s) => s.boothRoot);
  const setStatus = useUiStore((s) => s.setStatus);
  const [scanList, setScanList] = useState<string[]>([]);
  const [mismatchList, setMismatchList] = useState<string[]>([]);
  const [verList, setVerList] = useState<string[]>([]);
  const [stat, setStat] = useState('');
  const [vstat, setVstat] = useState('');
  const [mstat, setMstat] = useState('');
  const [full, setFull] = useState(0);
  const [missing, setMissing] = useState(0);
  const [scanning, setScanning] = useState(false);
  const [fixing, setFixing] = useState(false);
  const [versioning, setVersioning] = useState(false);
  const [fixingMismatch, setFixingMismatch] = useState(false);
  const [mismatching, setMismatching] = useState(false);
  const [verProgress, setVerProgress] = useState(0);
  const scanTaskRef = useRef<string | null>(null);
  const fixTaskRef = useRef<string | null>(null);
  const versionTaskRef = useRef<string | null>(null);
  const mismatchTaskRef = useRef<string | null>(null);

  async function cancelTask(ref: { current: string | null }, stop: () => void) {
    if (ref.current) {
      await invoke('cancel_task', { taskId: ref.current });
      stop();
    }
  }

  async function runScan() {
    if (!boothRoot) return;
    setScanList([]);
    setMismatchList([]);
    setStat('巡检中…');
    setMstat('错位检测中…');
    setScanning(true);
    setStatus('巡检中…');
    try {
      const taskId = await runTask(
        'audit',
        { base: boothRoot, dryRun: true, noFix: true },
        (evt) => {
          if (evt.type === 'itemDone') {
            setScanList((l) => [...l, `${evt.id}   [${evt.message}]`]);
          } else if (evt.type === 'finished') {
            const total = evt.done ?? 0;
            const miss = evt.failed ?? 0;
            setFull(total - miss);
            setMissing(miss);
            setStat(`共 ${total} 件，${miss} 件缺失三件套`);
            setScanning(false);
            scanTaskRef.current = null;
            setStatus(`巡检完成：${total} 件，${miss} 件待修复`);
          } else if (evt.type === 'cancelled') {
            setScanning(false);
            scanTaskRef.current = null;
            setStat('已取消');
          }
        },
      );
      scanTaskRef.current = taskId;
    } catch (e) {
      setStat(String(e));
      setScanning(false);
    }
  }

  async function runFix() {
    if (!boothRoot) return;
    setFixing(true);
    setScanList((l) => [...l, '── 修复日志 ──']);
    try {
      const taskId = await runTask(
        'audit',
        { base: boothRoot, dryRun: false, noFix: false },
        (evt) => {
          if (evt.type === 'itemDone') {
            setScanList((l) => [...l, `已修复 ${evt.id} · ${evt.message}`]);
          } else if (evt.type === 'finished') {
            setFixing(false);
            fixTaskRef.current = null;
            setStatus('三件套修复完成');
          } else if (evt.type === 'cancelled') {
            setFixing(false);
            fixTaskRef.current = null;
          }
        },
      );
      fixTaskRef.current = taskId;
    } catch (e) {
      setScanList((l) => [...l, String(e)]);
      setFixing(false);
    }
  }

  async function runVersion() {
    if (!boothRoot) return;
    setVerList([]);
    setVerProgress(0);
    setVersioning(true);
    setVstat('版本巡检中…');
    try {
      const taskId = await runTask(
        'version_audit',
        { base: boothRoot },
        (evt) => {
          if (evt.type === 'taskStarted' && typeof evt.total === 'number' && evt.total > 0) {
            setVerProgress(0);
          } else if (evt.type === 'itemDone') {
            setVerList((l) => [...l, `${evt.id} · ${evt.message}`]);
          } else if (evt.type === 'log') {
            setStatus(String(evt.line ?? ''));
          } else if (evt.type === 'finished') {
            const n = evt.failed ?? 0;
            setVstat(n > 0 ? `发现 ${n} 件可更新` : '未检测到更高版本');
            setVerProgress(100);
            setVersioning(false);
            versionTaskRef.current = null;
          } else if (evt.type === 'cancelled') {
            setVersioning(false);
            versionTaskRef.current = null;
            setVstat('已取消');
          }
        },
      );
      versionTaskRef.current = taskId;
    } catch (e) {
      setVstat(String(e));
      setVersioning(false);
    }
  }

  async function runMismatch() {
    if (!boothRoot) return;
    setMismatchList([]);
    setMstat('错位检测中…');
    setMismatching(true);
    try {
      const taskId = await runTask(
        'mismatch_audit',
        { base: boothRoot },
        (evt) => {
          if (evt.type === 'itemDone') {
            setMismatchList((l) => [...l, `${evt.id}   ${evt.message}`]);
          } else if (evt.type === 'finished') {
            const n = evt.failed ?? 0;
            setMstat(n > 0 ? `检测到 ${n} 件错位` : '无错位 ✓');
            setMismatching(false);
            mismatchTaskRef.current = null;
          } else if (evt.type === 'cancelled') {
            setMismatching(false);
            mismatchTaskRef.current = null;
            setMstat('已取消');
          }
        },
      );
      mismatchTaskRef.current = taskId;
    } catch (e) {
      setMstat(String(e));
      setMismatching(false);
    }
  }

  async function runFixMismatch() {
    if (!boothRoot) return;
    setFixingMismatch(true);
    setMstat('正在纠正…');
    try {
      const taskId = await runTask(
        'fix_mismatch',
        { base: boothRoot },
        (evt) => {
          if (evt.type === 'log') {
            setMismatchList((l) => [...l, String(evt.line ?? '')]);
          } else if (evt.type === 'itemError') {
            setMismatchList((l) => [...l, `错误 ${evt.id}：${evt.message}`]);
          } else if (evt.type === 'finished') {
            const n = evt.done ?? 0;
            setMstat(`纠正完成：${n} 件已重归档`);
            setFixingMismatch(false);
            mismatchTaskRef.current = null;
            setStatus(`错位纠正完成：${n} 件`);
          } else if (evt.type === 'cancelled') {
            setFixingMismatch(false);
            mismatchTaskRef.current = null;
          }
        },
      );
      mismatchTaskRef.current = taskId;
    } catch (e) {
      setMstat(String(e));
      setFixingMismatch(false);
    }
  }

  return (
    <PageShell>
      <PageTitle title="目录巡检" />
      <Lead>封面、图标、目录名，一次扫完再决定修不修。</Lead>

      <Row>
        <AccentButton onClick={() => void runScan()} disabled={scanning || !boothRoot}>
          {scanning ? '巡检中…' : '开始巡检'}
        </AccentButton>
        {scanning && (
          <SecondaryButton onClick={() => void cancelTask(scanTaskRef, () => setScanning(false))}>
            取消
          </SecondaryButton>
        )}
        <SecondaryButton onClick={() => void runFix()} disabled={fixing || missing === 0}>
          修复缺失三件套
        </SecondaryButton>
        {fixing && (
          <SecondaryButton onClick={() => void cancelTask(fixTaskRef, () => setFixing(false))}>
            取消
          </SecondaryButton>
        )}
        <StatText>{stat}</StatText>
      </Row>
      <ScanList>
        {scanList.map((line, i) => (
          <ListRow key={i}>{line}</ListRow>
        ))}
        {scanList.length === 0 && (
          <ListRow>等待巡检…</ListRow>
        )}
      </ScanList>

      <BadgeRow>
        <Badge kind="ok">完整 {full}</Badge>
        <Badge kind="warn">缺失 {missing}</Badge>
      </BadgeRow>

      <SectionGap />

      <Muted>错位纠正：扫描时同步联网比对官方分类，错位项目列于此。点下方按钮一键重归档。</Muted>
      <Row>
        <AccentButton onClick={() => void runMismatch()} disabled={scanning || mismatching || !boothRoot}>
          {mismatching ? '检测中…' : '检测错位'}
        </AccentButton>
        {mismatching && (
          <SecondaryButton onClick={() => void cancelTask(mismatchTaskRef, () => setMismatching(false))}>
            取消
          </SecondaryButton>
        )}
        <AccentButton onClick={() => void runFixMismatch()} disabled={fixingMismatch || !boothRoot}>
          {fixingMismatch ? '纠正中…' : '一键纠正错位'}
        </AccentButton>
        {fixingMismatch && (
          <SecondaryButton onClick={() => void cancelTask(mismatchTaskRef, () => setFixingMismatch(false))}>
            取消
          </SecondaryButton>
        )}
        <StatText>{mstat}</StatText>
      </Row>
      <MismatchList>
        {mismatchList.map((line, i) => (
          <ListRow key={i}>{line}</ListRow>
        ))}
      </MismatchList>

      <SectionGap />

      <Muted>实验性功能：联网比对官方商品名中的版本号，可能不准确，更新前请人工核对。</Muted>
      <Row>
        <AccentButton onClick={() => void runVersion()} disabled={versioning || !boothRoot}>
          {versioning ? '版本巡检中…' : '开始版本巡检'}
        </AccentButton>
        {versioning && (
          <SecondaryButton onClick={() => void cancelTask(versionTaskRef, () => setVersioning(false))}>
            取消
          </SecondaryButton>
        )}
        <StatText>{vstat}</StatText>
      </Row>
      <ProgressBar>
        <div style={{ width: `${verProgress}%` }} />
      </ProgressBar>
      <VerList>
        {verList.map((line, i) => (
          <ListRow key={i}>{line}</ListRow>
        ))}
      </VerList>
    </PageShell>
  );
}
