import styled from 'styled-components';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { boothItemUrl, extractBoothId } from '../lib/booth';
import { TextButton } from './ui';

const Wrap = styled.span`
  display: inline-flex;
  gap: var(--bvt-s2);
  flex: none;
`;

export function QueueActions({ id, path }: { id: string; path?: string }) {
  const boothId = extractBoothId(id);
  if (!boothId && !path) return null;
  return (
    <Wrap>
      {boothId && (
        <>
          <TextButton
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              void navigator.clipboard.writeText(boothId);
            }}
          >
            复制
          </TextButton>
          <TextButton
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              void openUrl(boothItemUrl(boothId));
            }}
          >
            打开
          </TextButton>
        </>
      )}
      {path && (
        <TextButton
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            void revealItemInDir(path);
          }}
        >
          文件夹
        </TextButton>
      )}
    </Wrap>
  );
}
