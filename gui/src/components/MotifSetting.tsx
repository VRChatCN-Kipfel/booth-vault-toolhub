/**
 * 背景花纹：浓淡滑条 + 换图。
 *
 * 换图走 <input type="file"> + FileReader 存 data URL：webview 直接引用本地路径
 * 需要开 asset 协议并配作用域，为一张装饰图不值当；data URL 还顺带跟着设置一起持久化。
 */

import { useRef, type ChangeEvent } from 'react';
import styled from 'styled-components';
import { MOTIF_DEFAULT, MOTIF_MAX, useThemeStore } from '../store/themeStore';
import { Muted, PanelLabel, RangeInput, Row, SecondaryButton, TextButton } from './ui';
import { error } from './Dialog';

/** data URL 要整个塞进 settings.json，超过这个大小写盘和启动都开始肉眼可见地卡。 */
const MAX_BYTES = 3 * 1024 * 1024;

const Wrap = styled.div`
  display: flex;
  flex-direction: column;
  gap: var(--bvt-s2);
`;

const Slider = styled.div`
  width: 320px;
  max-width: 100%;
`;

const Value = styled.span`
  flex: none;
  color: var(--bvt-accent);
  font-size: var(--bvt-fz-md);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
`;

const Hidden = styled.input`
  display: none;
`;

export function MotifSetting() {
  const { motifOpacity, motifImage, setMotifOpacity, setMotifImage } = useThemeStore();
  const fileRef = useRef<HTMLInputElement>(null);

  function onPick(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    // 同一张图连选两次也要能触发 change。
    e.target.value = '';
    if (!file) return;
    if (!file.type.startsWith('image/')) {
      void error('换图失败', '请选择图片文件。');
      return;
    }
    if (file.size > MAX_BYTES) {
      void error('图片过大', `请选择小于 ${MAX_BYTES / 1024 / 1024} MB 的图片。`);
      return;
    }
    const reader = new FileReader();
    reader.onload = () => setMotifImage(String(reader.result));
    reader.onerror = () => void error('换图失败', '读取图片失败。');
    reader.readAsDataURL(file);
  }

  return (
    <Wrap>
      <PanelLabel extra={<Value>{Math.round(motifOpacity * 100)}%</Value>}>背景花纹</PanelLabel>
      <Muted>侧栏纹样的浓淡；调到 0 即纯色。</Muted>
      <Slider>
        <RangeInput
          min={0}
          max={Math.round(MOTIF_MAX * 100)}
          step={1}
          value={Math.round(motifOpacity * 100)}
          aria-label="背景花纹浓淡"
          onChange={(e) => setMotifOpacity(Number(e.target.value) / 100)}
        />
      </Slider>
      <Row>
        <SecondaryButton onClick={() => fileRef.current?.click()}>更换图片</SecondaryButton>
        {motifImage && <TextButton onClick={() => setMotifImage(null)}>恢复自带纹样</TextButton>}
        <Muted>{motifImage ? '正在用自定义图片' : '正在用主题自带纹样'}</Muted>
        {motifOpacity !== MOTIF_DEFAULT && (
          <TextButton onClick={() => setMotifOpacity(MOTIF_DEFAULT)}>浓淡复位</TextButton>
        )}
      </Row>
      <Hidden ref={fileRef} type="file" accept="image/*" onChange={onPick} />
    </Wrap>
  );
}
