/**
 * 目录巡检页：三件套完整性巡检 + 修复 + 版本巡检 + 错位纠正。
 * 对齐原版 audit_page.py 的完整功能。
 */

import { useState } from 'react';
import styled from 'styled-components';
import {
  AccentButton, SecondaryButton, ObsPanel, ProgressBar, Badge,
} from '../components/ui';
import { PageTitle } from '../components/PageTitle';
import { useAppConfigStore } from '../store/appConfigStore';
import { useUiStore } from '../store/uiStore';
import { runTask } from '../lib/task';

const Page = styled.div`
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  height: 100%;
  overflow-y: auto;
`;

const SubTitle = styled.div`
  color: var(--bvt-text2);
  font-size: 13px;
  margin-top: -8px;
`;

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
  const [verProgress, setVerProgress] = useState(0);

  async function runScan() {
    if (!boothRoot) return;
    setScanList([]);
    setMismatchList([]);
    setStat('巡检中…');
    setMstat('错位检测中…');
    setScanning(true);
    setStatus('巡检中…');
    try {
      await runTask(
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
            setStatus(`巡检完成：${total} 件，${miss} 件待修复`);
          } else if (evt.type === 'cancelled') {
            setScanning(false);
          }
        },
      );
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
      await runTask(
        'audit',
        { base: boothRoot, dryRun: false, noFix: false },
        (evt) => {
          if (evt.type === 'itemDone') {
            setScanList((l) => [...l, `已修复 ${evt.id} · ${evt.message}`]);
          } else if (evt.type === 'finished' || evt.type === 'cancelled') {
            setFixing(false);
            setStatus('三件套修复完成');
          }
        },
      );
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
      await runTask(
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
          } else if (evt.type === 'cancelled') {
            setVersioning(false);
          }
        },
      );
    } catch (e) {
      setVstat(String(e));
      setVersioning(false);
    }
  }

  async function runMismatch() {
    if (!boothRoot) return;
    setMismatchList([]);
    setMstat('错位检测中…');
    try {
      await runTask(
        'mismatch_audit',
        { base: boothRoot },
        (evt) => {
          if (evt.type === 'itemDone') {
            setMismatchList((l) => [...l, `${evt.id}   ${evt.message}`]);
          } else if (evt.type === 'finished') {
            const n = evt.failed ?? 0;
            setMstat(n > 0 ? `检测到 ${n} 件错位` : '无错位 ✓');
          }
        },
      );
    } catch (e) {
      setMstat(String(e));
    }
  }

  async function runFixMismatch() {
    if (!boothRoot) return;
    setFixingMismatch(true);
    setMstat('正在纠正…');
    try {
      await runTask(
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
            setStatus(`错位纠正完成：${n} 件`);
          } else if (evt.type === 'cancelled') {
            setFixingMismatch(false);
          }
        },
      );
    } catch (e) {
      setMstat(String(e));
      setFixingMismatch(false);
    }
  }

  return (
    <Page>
      <PageTitle title="目录巡检" />
      <SubTitle>巡检 BOOTH 库的三件套完整性与命名规范</SubTitle>

      <Row>
        <AccentButton onClick={() => void runScan()} disabled={scanning || !boothRoot}>
          {scanning ? '巡检中…' : '开始巡检'}
        </AccentButton>
        <SecondaryButton onClick={() => void runFix()} disabled={fixing || missing === 0}>
          修复缺失三件套
        </SecondaryButton>
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
        <AccentButton onClick={() => void runMismatch()} disabled={scanning || !boothRoot}>
          检测错位
        </AccentButton>
        <AccentButton onClick={() => void runFixMismatch()} disabled={fixingMismatch || !boothRoot}>
          {fixingMismatch ? '纠正中…' : '一键纠正错位'}
        </AccentButton>
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
    </Page>
  );
}
