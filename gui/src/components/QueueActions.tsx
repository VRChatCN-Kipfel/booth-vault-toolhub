import styled from 'styled-components';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { boothItemUrl, extractBoothId } from '../lib/booth';

const Wrap = styled.span`
  display: inline-flex;
  gap: 4px;
  flex: none;
`;

const LinkBtn = styled.button`
  border: none;
  background: transparent;
  color: var(--bvt-accent);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  padding: 0 2px;
  &:hover { text-decoration: underline; }
  &:disabled { opacity: 0.4; cursor: not-allowed; }
`;

export function QueueActions({ id, path }: { id: string; path?: string }) {
  const boothId = extractBoothId(id);
  if (!boothId && !path) return null;
  return (
    <Wrap>
      {boothId && (
        <>
          <LinkBtn type="button" onClick={() => void navigator.clipboard.writeText(boothId)}>
            复制
          </LinkBtn>
          <LinkBtn type="button" onClick={() => void openUrl(boothItemUrl(boothId))}>
            打开
          </LinkBtn>
        </>
      )}
      {path && (
        <LinkBtn type="button" onClick={() => void revealItemInDir(path)}>
          文件夹
        </LinkBtn>
      )}
    </Wrap>
  );
}
