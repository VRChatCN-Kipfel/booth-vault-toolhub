/**
 * 主题系统：三主题 × 亮/暗 = 六套配色。
 *
 * 色值来自原 booth-keeper theme.py 的 THEMES 表（1:1 复刻）。
 * 通过 CSS variables 注入，styled-components 组件消费 var()。
 */

export type ThemeName = 'zhuyin' | 'liujin' | 'guwen';
export type ThemeMode = 'light' | 'dark';

export const THEME_NAMES: Record<ThemeName, string> = {
  zhuyin: '朱印',
  liujin: '鎏金',
  guwen: '古纹',
};

export const THEME_ORDER: ThemeName[] = ['zhuyin', 'liujin', 'guwen'];

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
  success: string;
  successL: string;
  warn: string;
  warnL: string;
  danger: string;
  dangerL: string;
  selBg: string;
  selText: string;
}

/** 按钮白字（全主题通用）。 */
export const BTN_TEXT = '#FAFAFA';

export const THEMES: Record<ThemeName, Record<ThemeMode, ThemePalette>> = {
  zhuyin: {
    light: {
      bg: '#FAF6EE', surface: '#FFFDF8', surface2: '#F3ECDD',
      text: '#2A2622', text2: '#6B6256', text3: '#9A9183',
      border: '#D9CFBE', border2: '#E8E0D2', hover: '#EFE7D8', inputBg: '#FCFAF4',
      accent: '#B83A2E', accentDeep: '#8F2C22', accentLight: '#F5DCD6',
      btnFill: '#B83A2E', btnFillHover: '#8F2C22', btnFillPress: '#8F2C22',
      success: '#2A5F4F', successL: '#D6E3D9',
      warn: '#8C6A2A', warnL: '#ECDFB6',
      danger: '#9E2B20', dangerL: '#F5DCD6',
      selBg: '#F5DCD6', selText: '#8F2C22',
    },
    dark: {
      bg: '#0F0E0C', surface: '#1A1815', surface2: '#211E1A',
      text: '#E8E2D4', text2: '#A89E8C', text3: '#6E6557',
      border: '#322E28', border2: '#2A2620', hover: '#2A2620', inputBg: '#16140F',
      accent: '#C8453A', accentDeep: '#A8332A', accentLight: '#F5DCD6',
      btnFill: '#C8453A', btnFillHover: '#A8332A', btnFillPress: '#A8332A',
      success: '#5E8C7A', successL: '#1F2A25',
      warn: '#C8A24A', warnL: '#2A2418',
      danger: '#D25543', dangerL: '#2A1A16',
      selBg: '#C8453A', selText: '#FAFAFA',
    },
  },
  liujin: {
    light: {
      bg: '#F4F1EA', surface: '#FBF7EF', surface2: '#ECE3D2',
      text: '#2B2415', text2: '#6E6147', text3: '#9C8E70',
      border: '#D8C9A6', border2: '#E6DBC2', hover: '#ECE1C9', inputBg: '#FAF5EA',
      accent: '#B8902F', accentDeep: '#8C6A2A', accentLight: '#F2E4BE',
      btnFill: '#2B2415', btnFillHover: '#3A3220', btnFillPress: '#1C1810',
      success: '#3E6B4F', successL: '#DDE7D6',
      warn: '#B8862F', warnL: '#F0E6C8',
      danger: '#9E3B22', dangerL: '#F3DFD6',
      selBg: '#F2E4BE', selText: '#6E5220',
    },
    dark: {
      bg: '#23211C', surface: '#2A261E', surface2: '#322C20',
      text: '#EDE3C8', text2: '#B6A884', text3: '#7C715A',
      border: '#3A3122', border2: '#2A2417', hover: '#2E281C', inputBg: '#1A160F',
      accent: '#C9A24B', accentDeep: '#B8862F', accentLight: '#F2E4BE',
      btnFill: '#7A5C28', btnFillHover: '#8C6A2A', btnFillPress: '#5A4420',
      success: '#5E8C6A', successL: '#1C2A22',
      warn: '#C8A24A', warnL: '#2A2418',
      danger: '#C8553C', dangerL: '#2A1813',
      selBg: '#C9A24B', selText: '#1A140A',
    },
  },
  guwen: {
    light: {
      bg: '#F1EFE6', surface: '#F8F6ED', surface2: '#E7E3D5',
      text: '#2A2E26', text2: '#5E6452', text3: '#8C917E',
      border: '#CFCAB6', border2: '#DEDAC9', hover: '#E9E5D8', inputBg: '#FCFBF6',
      accent: '#3F6B52', accentDeep: '#2F5142', accentLight: '#D6E3D9',
      btnFill: '#3F6B52', btnFillHover: '#2F5142', btnFillPress: '#2F5142',
      success: '#3F6B52', successL: '#D6E3D9',
      warn: '#9C6B3F', warnL: '#EBDDC8',
      danger: '#9E3B22', dangerL: '#F3DFD6',
      selBg: '#D6E3D9', selText: '#2F5142',
    },
    dark: {
      bg: '#0E1210', surface: '#161A16', surface2: '#1E231D',
      text: '#DDE3D6', text2: '#9DA892', text3: '#6B7163',
      border: '#2C332B', border2: '#232A22', hover: '#20271F', inputBg: '#12160F',
      accent: '#5C9A7C', accentDeep: '#4F8068', accentLight: '#D6E3D9',
      btnFill: '#5C9A7C', btnFillHover: '#4F8068', btnFillPress: '#4F8068',
      success: '#5C9A7C', successL: '#16241C',
      warn: '#B08A4A', warnL: '#262015',
      danger: '#C0623C', dangerL: '#2A1813',
      selBg: '#5C9A7C', selText: '#0E1210',
    },
  },
};

/** 母题类型（对应 theme.py 的 kind）。 */
export type MotifKind = 'zhuyin' | 'gold' | 'guwen';

/** 默认明暗（theme.py: DEFAULT_MODE_PER_THEME，全 light）。 */
export const DEFAULT_MODE_PER_THEME: Record<ThemeName, ThemeMode> = {
  zhuyin: 'light',
  liujin: 'light',
  guwen: 'light',
};

/** 字体栈（对齐 theme.py）。 */
export const FONTS = {
  serif: `'Noto Serif CJK SC','Source Han Serif SC','Songti SC','SimSun','STSong',serif`,
  sans: `'Noto Sans CJK SC','Microsoft YaHei','PingFang SC','Heiti SC',sans-serif`,
  mono: `'JetBrains Mono','Cascadia Code','Sarasa Mono SC','Consolas',monospace`,
};
