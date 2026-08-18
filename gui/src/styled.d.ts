import 'styled-components';
import type { ThemeMode, ThemeName } from './theme/themes';

declare module 'styled-components' {
  export interface DefaultTheme {
    theme: ThemeName;
    mode: ThemeMode;
  }
}
