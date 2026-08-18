/**
 * 页题：左侧一道朱记竖线 + 宋体标题，下面跟一行说明。
 * 花饰全部去掉——留白和这道竖线就是识别度。
 */

import styled from 'styled-components';
import type { ReactNode } from 'react';

const Wrap = styled.header`
  flex: none;
  display: flex;
  align-items: flex-start;
  gap: var(--bvt-s4);
`;

const Main = styled.div`
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--bvt-s2);
  border-left: var(--bvt-mark-w) solid var(--bvt-accent);
  padding-left: var(--bvt-s4);
`;

const Title = styled.h2`
  font-family: var(--bvt-serif);
  font-size: var(--bvt-fz-title);
  font-weight: 600;
  line-height: 1.2;
  letter-spacing: var(--bvt-title-track);
  color: var(--bvt-text);
`;

const Desc = styled.p`
  color: var(--bvt-text2);
  font-size: var(--bvt-fz-sm);
  line-height: 1.7;
  max-width: 68ch;
`;

const Actions = styled.div`
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--bvt-s2);
  padding-top: 2px;
`;

export function PageTitle({
  title,
  desc,
  actions,
}: {
  title: string;
  desc?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <Wrap>
      <Main>
        <Title>{title}</Title>
        {desc && <Desc>{desc}</Desc>}
      </Main>
      {actions && <Actions>{actions}</Actions>}
    </Wrap>
  );
}
