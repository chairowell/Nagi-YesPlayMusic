export function shouldOpenLibraryOnStartup(
  showLibraryDefault: boolean,
  currentRouteName: unknown
): boolean {
  return showLibraryDefault && currentRouteName === 'home';
}
