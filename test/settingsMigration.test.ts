import { describe, expect, test } from 'bun:test';
import { mergeSettings } from '../src/utils/updateApp';
import defaultStorageState from '../src/stores/defaults';
import {
  normalizeLyricFontSize,
  normalizeMusicQuality,
} from '../src/utils/persistedState';

describe('设置版本迁移', () => {
  test('保留用户快捷键并补上新版新增项，不写入 null', () => {
    const defaults = {
      ...defaultStorageState.settings,
      cacheLimit: 8192,
      shortcuts: [
        {
          id: 'play',
          name: '播放/暂停',
          shortcut: 'CommandOrControl+P',
          globalShortcut: 'Alt+CommandOrControl+P',
        },
        {
          id: 'minimize',
          name: '隐藏/显示播放器',
          shortcut: 'CommandOrControl+M',
          globalShortcut: 'Alt+CommandOrControl+M',
        },
      ],
    };
    const saved = {
      cacheLimit: false,
      shortcuts: [{ id: 'play', shortcut: 'Space' }],
    };

    const settings = mergeSettings(defaults, saved);

    expect(settings.shortcuts).toEqual([
      {
        id: 'play',
        name: '播放/暂停',
        shortcut: 'Space',
        globalShortcut: 'Alt+CommandOrControl+P',
      },
      {
        id: 'minimize',
        name: '隐藏/显示播放器',
        shortcut: 'CommandOrControl+M',
        globalShortcut: 'Alt+CommandOrControl+M',
      },
    ]);
    expect(settings.shortcuts.every(Boolean)).toBe(true);
    expect(settings.cacheLimit).toBeNull();
  });

  test('快捷键数量相同但 ID 不同也会补齐缺失项', () => {
    const settings = mergeSettings(
      {
        ...defaultStorageState.settings,
        shortcuts: [
          {
            id: 'play',
            name: '播放/暂停',
            shortcut: 'P',
            globalShortcut: 'Alt+P',
          },
          {
            id: 'next',
            name: '下一首',
            shortcut: 'N',
            globalShortcut: 'Alt+N',
          },
        ],
      },
      {
        shortcuts: [
          { id: 'play', shortcut: 'Space' },
          { id: 'legacy', shortcut: 'L' },
        ],
      }
    );

    expect(settings.shortcuts.map(shortcut => shortcut.id)).toEqual([
      'play',
      'legacy',
      'next',
    ]);
  });

  test('数字选择器的 DOM 字符串会被归一化', () => {
    expect(normalizeMusicQuality('128000', 320000)).toBe(128000);
    expect(normalizeMusicQuality('flac', 320000)).toBe('flac');
    expect(normalizeMusicQuality('invalid', 320000)).toBe(320000);
    expect(normalizeLyricFontSize('16', 28)).toBe(16);
    expect(normalizeLyricFontSize('', 28)).toBe(28);

    const settings = mergeSettings(defaultStorageState.settings, {
      musicQuality: '192000',
      lyricFontSize: '36',
    });
    expect(settings.musicQuality).toBe(192000);
    expect(settings.lyricFontSize).toBe(36);
  });
});
