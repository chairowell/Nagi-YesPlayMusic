export interface AppShell {
  restoreScrollPosition(): void;
  scrollMainTo(optionsOrX?: ScrollToOptions | number, y?: number): void;
}
