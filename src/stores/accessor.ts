import type { AppStore } from './app';

let activeAppStore: AppStore | null = null;

export function registerAppStore(store: AppStore) {
  activeAppStore = store;
}

export function getAppStore(): AppStore {
  if (!activeAppStore) {
    throw new Error('Pinia app store 尚未初始化');
  }
  return activeAppStore;
}

export function getAppStoreIfReady(): AppStore | null {
  return activeAppStore;
}
