export function isLinux(): boolean {
  return /Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent);
}
