/**
 * 支持作者栏位模板（多作者，构建时注入）。
 *
 * 数据源：`gui/public/support.json`（仓库提交默认模板，工作流用 secrets 覆盖后打包）。
 * 结构：
 *   {
 *     "authors": [
 *       { "name", "title", "url", "desc", "image" }
 *     ],
 *     "note": "底部说明文字（可选）"
 *   }
 *  - image：二维码图，base64 data URL（secrets 注入时已编码）或 public 下相对路径。
 *  - 排序：由应用端决定（保持注入顺序），不在数据中硬编码。
 *
 * 未配置（文件缺失/空/拉取失败）时优雅降级为内置文案。
 */

import { useEffect, useState } from 'react';
import styled from 'styled-components';
import { PanelLabel } from './ui';

export interface SupportAuthor {
  /** 作者名。 */
  name: string;
  /** 昵称/标题（可选）。 */
  title?: string;
  /** 支持链接（可选）。 */
  url?: string;
  /** 一句话说明（可选）。 */
  desc?: string;
  /** 二维码：base64 data URL 或 public 相对路径。 */
  image?: string;
}

export interface SupportConfig {
  authors?: SupportAuthor[];
  /** 底部说明文字。 */
  note?: string;
}

const Section = styled.div`
  margin-top: 4px;
`;

const Intro = styled.div`
  color: var(--bvt-text2);
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
`;

/** 作者条目卡片。 */
const AuthorCard = styled.div`
  display: flex;
  gap: 14px;
  align-items: center;
  padding: 10px 0;
  border-bottom: 1px solid var(--bvt-border2);
  &:last-of-type { border-bottom: none; }
`;

const Qr = styled.img`
  width: 96px;
  height: 96px;
  border: 1.5px solid var(--bvt-text3);
  border-radius: var(--bvt-radius, 0px);
  object-fit: cover;
  flex: none;
`;

const Info = styled.div`
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
`;

const Name = styled.div`
  font-size: 14px;
  font-weight: 600;
  color: var(--bvt-text);
`;

const Title = styled.div`
  font-size: 12px;
  color: var(--bvt-text2);
`;

const Desc = styled.div`
  font-size: 12px;
  color: var(--bvt-text3);
`;

const Link = styled.a`
  color: var(--bvt-accent);
  font-size: 12px;
  text-decoration: none;
  &:hover { text-decoration: underline; }
`;

const Fallback = styled.div`
  color: var(--bvt-text3);
  font-size: 12px;
`;

const Note = styled.div`
  color: var(--bvt-text3);
  font-size: 11px;
  line-height: 1.5;
  margin-top: 8px;
  white-space: pre-wrap;
`;

const DEFAULT_INTRO = '本工具使用Apache-2.0协议免费开源。\n如果你是付费得到该软件，请主动举报抵制不良商家倒卖。 ✨';
const DEFAULT_NOTE = '赞助将会用于支付作者们给吃软饭的大肥鱼购用token，以及日后可能计划的被库克打劫所获的macos开发者签名\n本工具不会进行强制化收费，不存在VIP档次等级等，自愿赞助，各位老爷赏点白饭吃吧pwp';

const GraphSection = styled.div`
  margin-top: 4px;
`;

const GraphIntro = styled.div`
  color: var(--bvt-text2);
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
`;

/** 贡献者图谱：页面限制内自动等比缩放，不拉伸不平铺。 */
const Graph = styled.img`
  display: block;
  max-width: 100%;
  max-height: 240px;
  width: auto;
  height: auto;
  object-fit: contain;
  margin-top: 8px;
`;

const CONTRIBUTOR_URL =
  'https://contrib.rocks/image?repo=VRChatCN-Kipfel/booth-vault-toolhub';

function ContributorGraph() {
  return (
    <GraphSection>
      <PanelLabel>贡献者</PanelLabel>
      <GraphIntro>感谢所有为这个开源项目贡献过的开发者。</GraphIntro>
      <Graph src={CONTRIBUTOR_URL} alt="项目贡献者" loading="lazy" />
    </GraphSection>
  );
}

/** 解析 image 字段 → src（data URL 原样；相对路径加 BASE_URL）。 */
function resolveImage(image: string): string {
  if (image.startsWith('data:') || image.startsWith('http')) return image;
  return `${import.meta.env.BASE_URL}${image}`;
}

export function SupportSection() {
  const [config, setConfig] = useState<SupportConfig | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetch(`${import.meta.env.BASE_URL}support.json`)
      .then((r) => (r.ok ? r.json() : null))
      .then((data: SupportConfig | null) => {
        if (!cancelled && data && Array.isArray(data.authors) && data.authors.length > 0) {
          setConfig(data);
        }
      })
      .catch(() => {
        // 拉取失败 → 保持 null → 降级
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const authors = config?.authors ?? [];

  return (
    <Section>
      <PanelLabel>☕ 支持作者</PanelLabel>
      <Intro>{DEFAULT_INTRO}</Intro>
      {authors.length === 0 ? (
        <Fallback>（构建时未注入支持作者数据，见 SupportSection 注释）</Fallback>
      ) : (
        authors.map((a) => (
          <AuthorCard key={a.name + (a.title ?? '')}>
            {a.image && <Qr src={resolveImage(a.image)} alt={`${a.name} 收款码`} />}
            <Info>
              <Name>{a.name}</Name>
              {a.title && <Title>{a.title}</Title>}
              {a.desc && <Desc>{a.desc}</Desc>}
              {a.url && (
                <Link href={a.url} target="_blank" rel="noreferrer">支持链接 ↗</Link>
              )}
            </Info>
          </AuthorCard>
        ))
      )}
      <Note>{config?.note ?? DEFAULT_NOTE}</Note>
      <ContributorGraph />
    </Section>
  );
}
