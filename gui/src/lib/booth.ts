export function parseDiscrete(text: string): string[] {
  const ids: string[] = [];
  for (const m of text.matchAll(/\/items\/(\d+)/g)) {
    if (!ids.includes(m[1])) ids.push(m[1]);
  }
  for (const m of text.replace(/,/g, ' ').matchAll(/(?<!\d)\d{5,}(?!\d)/g)) {
    if (!ids.includes(m[0])) ids.push(m[0]);
  }
  return ids;
}

export function extractBoothId(raw: string): string | null {
  const m = raw.match(/(?<!\d)(\d{5,})(?!\d)/);
  return m ? m[1] : null;
}

export function hasFileProductId(path: string): boolean {
  return /(?<!\d)\d{7,}(?!\d)/.test(path);
}

export function boothItemUrl(id: string): string {
  return `https://booth.pm/ja/items/${id}`;
}

export function formatPrice(price: number): string {
  return price === 0 ? '免费' : `¥${price}`;
}

export function badgeKind(status: string): 'ok' | 'run' | 'wait' | 'warn' | 'err' {
  if (status === 'err' || status === 'error') return 'err';
  if (status === 'run') return 'run';
  if (
    status === 'warn' ||
    status === 'exists' ||
    status === 'mismatch' ||
    status === 'ambiguous'
  ) {
    return 'warn';
  }
  if (status === 'ok') return 'ok';
  return 'wait';
}

export function badgeLabel(status: string): string {
  switch (status) {
    case 'exists':
      return '已存在';
    case 'mismatch':
      return '错位';
    case 'ambiguous':
      return '歧义';
    case 'err':
    case 'error':
      return '失败';
    case 'ok':
      return '完成';
    case 'run':
      return '进行中';
    case 'warn':
      return '注意';
    default:
      return status;
  }
}
