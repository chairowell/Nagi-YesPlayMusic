import defaultStorageState from '@/stores/defaults';
import pkg from '../../package.json';
import {
  decodeDataState,
  decodeSettingsState,
  decodeStoredRecord,
  readStoredJson,
} from '@/utils/persistedState';
import type { SettingsState } from '@/types/persistence';

export function mergeSettings(
  defaultSettings: SettingsState,
  savedSettings: unknown
): SettingsState {
  return decodeSettingsState(savedSettings, defaultSettings);
}

const updateSetting = () => {
  const settings = mergeSettings(
    defaultStorageState.settings,
    readStoredJson(localStorage, 'settings')
  );

  if (localStorage.getItem('appVersion') === '"0.3.9"') {
    settings.lyricsBackground = true;
  }

  localStorage.setItem('settings', JSON.stringify(settings));
};

const updateData = () => {
  const data = decodeDataState(
    readStoredJson(localStorage, 'data'),
    defaultStorageState.data
  );
  localStorage.setItem('data', JSON.stringify(data));
};

const updatePlayer = () => {
  let parsedData = decodeStoredRecord(readStoredJson(localStorage, 'player'));
  const appVersion = localStorage.getItem('appVersion');
  if (appVersion === `"0.2.5"`) parsedData = {}; // Player state changed in 0.2.6.
  const data = {
    ...parsedData,
  };
  localStorage.setItem('player', JSON.stringify(data));
};

const removeOldStuff = () => {
  // remove old indexedDB databases created by localforage
  indexedDB.deleteDatabase('tracks');
};

export default function updateApp(): void {
  updateSetting();
  updateData();
  updatePlayer();
  removeOldStuff();
  localStorage.setItem('appVersion', JSON.stringify(pkg.version));
}
