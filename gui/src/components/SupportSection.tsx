/**
 * 支持作者栏位模板（多作者，构建时注入）。
 *
 * 数据源：`gui/public/support.json`（仓库提交默认模板，工作流用 secrets 覆盖后打包）。
 * 结构：
 *   {
 *     "authors": [
 *       { "name", "title", "url", "links", "desc", "avatar", "avatarAlt", "image", "imageAlt" }
 *     ],
 *     "note": "底部说明文字（可选）"
 *   }
 *  - url：个人链接（可选，旧字段，等价单条 links）。
 *  - links：个人链接数组（可选，最多渲染 6 条）：[{ "url", "label" }]，label 作悬浮提示。
 *  - avatar：作者头像，显示在名字左侧（可选）。
 *  - avatarAlt：头像描述（可选，缺省回退 desc）。
 *  - image：二维码图，base64 data URL（secrets 注入时已编码）或 public 下相对路径。
 *  - imageAlt：二维码描述（可选，缺省回退 desc）。
 *  - 排序：由应用端决定（保持注入顺序），不在数据中硬编码。
 *
 * 未配置（文件缺失/空/拉取失败）时优雅降级为内置文案。
 */

import { useEffect, useState } from 'react';
import styled from 'styled-components';
import { PanelLabel } from './ui';

export interface SupportLink {
  /** 链接地址。 */
  url: string;
  /** 悬浮提示（可选）。 */
  label?: string;
}

export interface SupportAuthor {
  /** 作者名。 */
  name: string;
  /** 昵称/标题（可选）。 */
  title?: string;
  /** 个人链接（可选，等价单条 links）。 */
  url?: string;
  /** 个人链接数组（可选，最多渲染 6 条）。 */
  links?: SupportLink[];
  /** 一句话说明（可选）。 */
  desc?: string;
  /** 头像：data URL 或 public 相对路径（可选）。 */
  avatar?: string;
  /** 头像描述（可选，缺省回退 desc 再回退默认）。 */
  avatarAlt?: string;
  /** 二维码：base64 data URL 或 public 相对路径。 */
  image?: string;
  /** 二维码描述（可选，缺省回退 desc 再回退默认）。 */
  imageAlt?: string;
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
  object-fit: fill;
  flex: none;
`;

const Info = styled.div`
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
`;

const Head = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
`;

const Avatar = styled.img`
  width: 28px;
  height: 28px;
  border-radius: 50%;
  object-fit: contain;
  flex: none;
  border: 1px solid var(--bvt-border2);
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

const Links = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
`;

const LinkBtn = styled.a`
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 1px solid var(--bvt-border2);
  background: var(--bvt-surface2);
  display: flex;
  align-items: center;
  justify-content: center;
  text-decoration: none;
  transition: border-color calc(0.2s / var(--bvt-anim)) ease, transform calc(0.2s / var(--bvt-anim)) ease;
  &:hover { border-color: var(--bvt-accent); transform: translateY(-1px); }
  img { width: 14px; height: 14px; display: block; }
`;

/** 每作者可渲染的链接数上限（flex-wrap 溢出会换行，6 条内布局稳定）。 */
const MAX_LINKS = 6;

/** 站点头像内存缓存（按 host，防重复请求）。 */
const faviconCache = new Map<string, string>();

/** 站点头像 CDN 兜底：favicon 拉不到时用默认地球图标，避免破图。 */
const FAVICON_FALLBACK = 'data:image/svg+xml;base64,' + btoa(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7" fill="none" stroke="#8a7b6a" stroke-width="1.5"/><ellipse cx="8" cy="8" rx="3.4" ry="7" fill="none" stroke="#8a7b6a" stroke-width="1.5"/><line x1="1" y1="8" x2="15" y2="8" stroke="#8a7b6a" stroke-width="1.5"/></svg>'
);

/** 实时懒加载站点 favicon（google 兜底），内存缓存防重复请求。 */
function faviconSrc(url: string): string {
  try {
    const host = new URL(url).hostname;
    const cached = faviconCache.get(host);
    if (cached) return cached;
    const src = `https://www.google.com/s2/favicons?domain=${encodeURIComponent(host)}&sz=64`;
    faviconCache.set(host, src);
    return src;
  } catch {
    return FAVICON_FALLBACK;
  }
}

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

const CONTRIBUTOR_URL =
  'https://contrib.rocks/image?repo=VRChatCN-Kipfel/booth-vault-toolhub&columns=12';

/** 贡献者图谱：锁 12 列扁形横排，按原始尺寸渲染（不缩放），仅溢出时等比缩小兜底。 */
const Graph = styled.img`
  display: block;
  max-width: 100%;
  width: auto;
  height: auto;
  object-fit: contain;
  margin-top: 8px;
`;

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

/** 图像描述：自定义 alt → desc → 默认文案。 */
function imageAlt(a: SupportAuthor, kind: 'avatar' | 'image'): string {
  return a[`${kind}Alt`] || a.desc || (kind === 'avatar' ? '作者头像' : '收款码');
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
  const linksOf = (a: SupportAuthor): SupportLink[] => {
    const merged = [...(a.links ?? [])];
    if (a.url) merged.push({ url: a.url });
    return merged.slice(0, MAX_LINKS);
  };

  return (
    <Section>
      <PanelLabel>☕ 支持作者</PanelLabel>
      <Intro>{DEFAULT_INTRO}</Intro>
      {authors.length === 0 ? (
        <Fallback>（构建时未注入支持作者数据，见 SupportSection 注释）</Fallback>
      ) : (
        authors.map((a) => {
          const links = linksOf(a);
          return (
            <AuthorCard key={a.name + (a.title ?? '')}>
              {a.image && <Qr src={resolveImage(a.image)} alt={imageAlt(a, 'image')} />}
              <Info>
                <Head>
                  {a.avatar && <Avatar src={resolveImage(a.avatar)} alt={imageAlt(a, 'avatar')} />}
                  <Name>{a.name}</Name>
                </Head>
                {a.title && <Title>{a.title}</Title>}
                {a.desc && <Desc>{a.desc}</Desc>}
                {links.length > 0 && (
                  <Links>
                    {links.map((l) => (
                      <LinkBtn
                        key={l.url}
                        href={l.url}
                        target="_blank"
                        rel="noreferrer"
                        title={l.label ?? l.url}
                      >
                        <img
                          src={faviconSrc(l.url)}
                          alt=""
                          loading="lazy"
                          onError={(e) => { e.currentTarget.src = FAVICON_FALLBACK; }}
                        />
                      </LinkBtn>
                    ))}
                  </Links>
                )}
              </Info>
            </AuthorCard>
          );
        })
      )}
      <Note>{config?.note ?? DEFAULT_NOTE}</Note>
      <ContributorGraph />
    </Section>
  );
}
