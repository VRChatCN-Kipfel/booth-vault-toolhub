/**
 * 主题系统：三主题 × 亮/暗 = 六套配色。
 *
 * 朱印＝宣纸印泥，鎏金＝骨色金缮，古纹＝青瓷青铜。
 * 通过 CSS variables 注入，styled-components 组件消费 var()。
 */

export type ThemeName = 'zhuyin' | 'liujin' | 'guwen';
export type ThemeMode = 'light' | 'dark';

export const THEME_NAMES: Record<ThemeName, string> = {
  zhuyin: '朱印',
  liujin: '鎏金',
  guwen: '古纹',
};

export const THEME_HINTS: Record<ThemeName, string> = {
  zhuyin: '印泥 · 方折 · 宣纸',
  liujin: '金缮 · 青海波 · 漆',
  guwen: '云雷 · 青铜 · 叶脉',
};

export const THEME_ORDER: ThemeName[] = ['zhuyin', 'liujin', 'guwen'];

export function motifBgSrc(theme: ThemeName, mode: ThemeMode): string {
  return `/motifs/${theme}-bg${mode === 'dark' ? '-dark' : ''}.jpg`;
}

export function motifSidebarSrc(theme: ThemeName, mode: ThemeMode): string {
  return `/motifs/${theme}-sidebar${mode === 'dark' ? '-dark' : ''}.jpg`;
}

export const DEFAULT_THEME: ThemeName = 'zhuyin';

export interface ThemePalette {
  bg: string;
  surface: string;
  surface2: string;
  text: string;
  text2: string;
  text3: string;
  border: string;
  border2: string;
  hover: string;
  inputBg: string;
  accent: string;
  accentDeep: string;
  accentLight: string;
  btnFill: string;
  btnFillHover: string;
  btnFillPress: string;
  onAccent: string;
  onBtn: string;
  success: string;
  successL: string;
  warn: string;
  warnL: string;
  danger: string;
  dangerL: string;
  selBg: string;
  selText: string;
}

export const BTN_TEXT = '#FAFAFA';

/**
 * 三层模型：纸（bg/surface/surface2）· 墨（text/text2/text3 + border）· 朱（accent 一色）。
 * 层次靠明度差，不靠饱和度；强调色只在当前态、主按钮、印记上出现。
 *
 * accentDeep 是「叠在 accentLight 上的文字色」：亮色下比 accent 更深，
 * 暗色下必须比 accent 更亮，否则暗底浅字会糊成一团。
 */
export const THEMES: Record<ThemeName, Record<ThemeMode, ThemePalette>> = {
  zhuyin: {
    light: {
      bg: '#F5F2EC', surface: '#FFFFFF', surface2: '#EAE5DC',
      text: '#1C1A17', text2: '#5A554E', text3: '#918B81',
      border: '#DDD7CC', border2: '#ECE7DE', hover: '#F0ECE3', inputBg: '#FFFFFF',
      accent: '#A8322A', accentDeep: '#7E231D', accentLight: '#F5E7E4',
      btnFill: '#A8322A', btnFillHover: '#8E2822', btnFillPress: '#741E19',
      onAccent: '#FFFFFF', onBtn: '#FFFFFF',
      success: '#3B6B50', successL: '#E2EDE6',
      warn: '#8A6420', warnL: '#F3EAD3',
      danger: '#A33325', dangerL: '#F5E2DE',
      selBg: '#F5E7E4', selText: '#8C2A22',
    },
    dark: {
      bg: '#14120F', surface: '#1C1916', surface2: '#221E19',
      text: '#EDE7DC', text2: '#A79E91', text3: '#746C61',
      border: '#332D26', border2: '#262119', hover: '#272219', inputBg: '#191510',
      accent: '#D9564A', accentDeep: '#F0A69C', accentLight: '#33201C',
      btnFill: '#B03B31', btnFillHover: '#C9483C', btnFillPress: '#8E2C24',
      onAccent: '#FFFFFF', onBtn: '#FFFFFF',
      success: '#6FA588', successL: '#1B2621',
      warn: '#C99B4E', warnL: '#2A2214',
      danger: '#DF6A5A', dangerL: '#2C1A16',
      selBg: '#33201C', selText: '#F0A69C',
    },
  },
  liujin: {
    light: {
      bg: '#F3EFE6', surface: '#FDFBF6', surface2: '#E7E1D3',
      text: '#211E15', text2: '#5E574A', text3: '#948C7B',
      border: '#DCD4C3', border2: '#EBE6DA', hover: '#EFEADF', inputBg: '#FFFDF8',
      accent: '#9A7B28', accentDeep: '#6F5718', accentLight: '#F0E9D2',
      btnFill: '#2A2418', btnFillHover: '#3B3323', btnFillPress: '#191409',
      onAccent: '#FFFFFF', onBtn: '#F7F2E4',
      success: '#3F6B4E', successL: '#E3EDE3',
      warn: '#8E6C1E', warnL: '#F3EBD0',
      danger: '#933A22', dangerL: '#F2E1D9',
      selBg: '#F0E9D2', selText: '#6B5312',
    },
    dark: {
      bg: '#14120C', surface: '#1C1913', surface2: '#221E15',
      text: '#EFE6CD', text2: '#A99E82', text3: '#7A7159',
      border: '#332D20', border2: '#262114', hover: '#272115', inputBg: '#191509',
      accent: '#D4AC53', accentDeep: '#E9C87F', accentLight: '#2E2716',
      btnFill: '#B8912F', btnFillHover: '#CFA742', btnFillPress: '#957425',
      onAccent: '#17130A', onBtn: '#17130A',
      success: '#72A583', successL: '#1B241C',
      warn: '#CBA152', warnL: '#2A2314',
      danger: '#CE6B49', dangerL: '#2A1B13',
      selBg: '#2E2716', selText: '#E9C87F',
    },
  },
  guwen: {
    light: {
      bg: '#EDF0EB', surface: '#FBFCFA', surface2: '#E0E6DE',
      text: '#1A1F19', text2: '#4E574B', text3: '#838C7E',
      border: '#D2D9CF', border2: '#E5EAE2', hover: '#E9EDE7', inputBg: '#FDFEFC',
      accent: '#386B4C', accentDeep: '#265036', accentLight: '#DEEAE0',
      btnFill: '#386B4C', btnFillHover: '#2B5A3E', btnFillPress: '#204731',
      onAccent: '#FFFFFF', onBtn: '#FFFFFF',
      success: '#386B4C', successL: '#DEEAE0',
      warn: '#856129', warnL: '#F0E7D0',
      danger: '#8E3626', dangerL: '#F0E0DA',
      selBg: '#DEEAE0', selText: '#265036',
    },
    dark: {
      bg: '#101412', surface: '#171C18', surface2: '#1D231E',
      text: '#D8E2D6', text2: '#96A292', text3: '#6C766A',
      border: '#2A322A', border2: '#212821', hover: '#222A23', inputBg: '#131815',
      accent: '#5FA87C', accentDeep: '#8CC7A2', accentLight: '#1C2A21',
      btnFill: '#3E7A58', btnFillHover: '#4F9169', btnFillPress: '#2C5C42',
      onAccent: '#0D120F', onBtn: '#FFFFFF',
      success: '#5FA87C', successL: '#18231C',
      warn: '#C09A52', warnL: '#262014',
      danger: '#CB6A50', dangerL: '#291A15',
      selBg: '#1C2A21', selText: '#8CC7A2',
    },
  },
};

export type MotifKind = 'zhuyin' | 'gold' | 'guwen';

export const DEFAULT_MODE_PER_THEME: Record<ThemeName, ThemeMode> = {
  zhuyin: 'light',
  liujin: 'light',
  guwen: 'light',
};

/**
 * 字体栈按平台自然降级：先系统 UI 字体，再各平台中文字库，最后通用兜底。
 * 不假设任何一端装了 Noto —— Windows 上没有 Noto Sans CJK 时会掉到雅黑。
 */
export const FONTS = {
  serif: `'Songti SC','Noto Serif CJK SC','Source Han Serif SC','STSong','SimSun','Georgia',serif`,
  sans: `system-ui,-apple-system,'Segoe UI','PingFang SC','Microsoft YaHei UI','Microsoft YaHei','Noto Sans CJK SC','Source Han Sans SC','Hiragino Sans GB',sans-serif`,
  mono: `ui-monospace,'SF Mono','JetBrains Mono','Cascadia Code','Consolas','Sarasa Mono SC',monospace`,
};
