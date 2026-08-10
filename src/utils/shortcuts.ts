// Default local and global shortcuts.

export interface RecordedShortcutKey {
  code: string;
  key: string;
}

const MODIFIER_KEYS = new Set(['Alt', 'Control', 'Meta', 'Shift']);

export function getRecordedShortcutKeyIdentity({
  code,
  key,
}: RecordedShortcutKey): string {
  // Both physical modifier keys map to one logical accelerator.
  return MODIFIER_KEYS.has(key) ? `modifier:${key}` : `code:${code}`;
}

export default [
  {
    id: 'play',
    name: '播放/暂停',
    shortcut: 'CommandOrControl+P',
    globalShortcut: 'Alt+CommandOrControl+P',
  },
  {
    id: 'next',
    name: '下一首',
    shortcut: 'CommandOrControl+Right',
    globalShortcut: 'Alt+CommandOrControl+Right',
  },
  {
    id: 'previous',
    name: '上一首',
    shortcut: 'CommandOrControl+Left',
    globalShortcut: 'Alt+CommandOrControl+Left',
  },
  {
    id: 'increaseVolume',
    name: '增加音量',
    shortcut: 'CommandOrControl+Up',
    globalShortcut: 'Alt+CommandOrControl+Up',
  },
  {
    id: 'decreaseVolume',
    name: '减少音量',
    shortcut: 'CommandOrControl+Down',
    globalShortcut: 'Alt+CommandOrControl+Down',
  },
  {
    id: 'like',
    name: '喜欢歌曲',
    shortcut: 'CommandOrControl+L',
    globalShortcut: 'Alt+CommandOrControl+L',
  },
  {
    id: 'minimize',
    name: '隐藏/显示播放器',
    shortcut: 'CommandOrControl+M',
    globalShortcut: 'Alt+CommandOrControl+M',
  },
];
