import { invoke } from '@tauri-apps/api/core';
import type { AppIconId } from './themes';

export async function applyAppIcon(id: AppIconId) {
  try {
    await invoke('set_app_icon', { id });
  } catch {
    // 浏览器预览没有窗口图标
  }
}
