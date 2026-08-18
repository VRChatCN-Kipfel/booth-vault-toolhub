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

export const THEMES: Record<ThemeName, Record<ThemeMode, ThemePalette>> = {
  zhuyin: {
    light: {
      bg: '#F3E6CF', surface: '#FBF3E4', surface2: '#E6D3B4',
      text: '#1A1410', text2: '#5A4E42', text3: '#8A7B6A',
      border: '#C4A97A', border2: '#DCC9A4', hover: '#EBD9B8', inputBg: '#FDF6E8',
      accent: '#C41A14', accentDeep: '#8A100C', accentLight: '#F4C4BC',
      btnFill: '#C41A14', btnFillHover: '#8A100C', btnFillPress: '#6E0C0A',
      onAccent: '#FFF8F2', onBtn: '#FFF8F2',
      success: '#2A5644', successL: '#D4E4D8',
      warn: '#9A6A18', warnL: '#F0E0B4',
      danger: '#A11410', dangerL: '#F4C4BC',
      selBg: '#F0C8C0', selText: '#8A100C',
    },
    dark: {
      bg: '#0D0B09', surface: '#161310', surface2: '#211C17',
      text: '#F0E6D4', text2: '#B09A82', text3: '#6E5C4A',
      border: '#3D3228', border2: '#2A231C', hover: '#2A231C', inputBg: '#120F0C',
      accent: '#E24A38', accentDeep: '#C42E22', accentLight: '#F4C4BC',
      btnFill: '#C42E22', btnFillHover: '#E24A38', btnFillPress: '#8A1C16',
      onAccent: '#FFF8F2', onBtn: '#FFF8F2',
      success: '#6A9A84', successL: '#1A2420',
      warn: '#D4A84A', warnL: '#2A2214',
      danger: '#E05848', dangerL: '#2A1410',
      selBg: '#C42E22', selText: '#FFF8F2',
    },
  },
  liujin: {
    light: {
      bg: '#EBE2CC', surface: '#F6EED8', surface2: '#DCCDA8',
      text: '#221A0C', text2: '#6A5A38', text3: '#96845C',
      border: '#C4B078', border2: '#D8C89A', hover: '#E4D4A8', inputBg: '#F8F0DC',
      accent: '#C9A018', accentDeep: '#8A6C14', accentLight: '#F0E0A4',
      btnFill: '#241C0E', btnFillHover: '#3A2E16', btnFillPress: '#161008',
      onAccent: '#1A1408', onBtn: '#F6EED8',
      success: '#3A6248', successL: '#D4E4D0',
      warn: '#B88620', warnL: '#F0E4BC',
      danger: '#9A3018', dangerL: '#F0D4C8',
      selBg: '#EEDC98', selText: '#5A440C',
    },
    dark: {
      bg: '#0E0C08', surface: '#17140E', surface2: '#221C12',
      text: '#F2E6C4', text2: '#B8A474', text3: '#7A6A48',
      border: '#3A3020', border2: '#2A2416', hover: '#2A2416', inputBg: '#120E08',
      accent: '#E0B44A', accentDeep: '#C49828', accentLight: '#F0E0A4',
      btnFill: '#C49828', btnFillHover: '#E0B44A', btnFillPress: '#8A6C18',
      onAccent: '#1A1408', onBtn: '#1A1408',
      success: '#6A9A78', successL: '#162018',
      warn: '#D4A84A', warnL: '#241C10',
      danger: '#D05038', dangerL: '#241410',
      selBg: '#E0B44A', selText: '#1A1408',
    },
  },
  guwen: {
    light: {
      bg: '#DEE6DC', surface: '#ECF2EA', surface2: '#C8D6C8',
      text: '#1A2218', text2: '#4E5C4A', text3: '#7A8874',
      border: '#A8B8A4', border2: '#C4D0C0', hover: '#D4E0D2', inputBg: '#F4F8F2',
      accent: '#2E5C42', accentDeep: '#1E4030', accentLight: '#C4DCC8',
      btnFill: '#2E5C42', btnFillHover: '#1E4030', btnFillPress: '#163024',
      onAccent: '#F2F8F2', onBtn: '#F2F8F2',
      success: '#2E5C42', successL: '#C8DCC8',
      warn: '#8A5C28', warnL: '#E8D8B8',
      danger: '#8A2818', dangerL: '#E8D0C8',
      selBg: '#C0D8C4', selText: '#1E4030',
    },
    dark: {
      bg: '#090C0A', surface: '#121612', surface2: '#1A221A',
      text: '#D4E2D4', text2: '#94A890', text3: '#647060',
      border: '#2A342A', border2: '#202820', hover: '#1E281E', inputBg: '#0E120E',
      accent: '#5CA87A', accentDeep: '#3E7A58', accentLight: '#C4DCC8',
      btnFill: '#3E7A58', btnFillHover: '#5CA87A', btnFillPress: '#2A5840',
      onAccent: '#0A0E0A', onBtn: '#F2F8F2',
      success: '#5CA87A', successL: '#142018',
      warn: '#C49A4A', warnL: '#221C10',
      danger: '#C85838', dangerL: '#221410',
      selBg: '#5CA87A', selText: '#0A0E0A',
    },
  },
};

export type MotifKind = 'zhuyin' | 'gold' | 'guwen';

export const DEFAULT_MODE_PER_THEME: Record<ThemeName, ThemeMode> = {
  zhuyin: 'light',
  liujin: 'light',
  guwen: 'light',
};

export const FONTS = {
  serif: `'Noto Serif CJK SC','Source Han Serif SC','Songti SC','SimSun','STSong',serif`,
  sans: `'Noto Sans CJK SC','PingFang SC','Hiragino Sans GB','Microsoft YaHei','Heiti SC',sans-serif`,
  mono: `'JetBrains Mono','Cascadia Code','Sarasa Mono SC','Consolas',monospace`,
};
