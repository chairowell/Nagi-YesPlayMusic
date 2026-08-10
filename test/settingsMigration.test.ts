import { describe, expect, test } from 'bun:test';
import { mergeSettings } from '../src/utils/updateApp';
import defaultStorageState from '../src/stores/defaults';

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
});
