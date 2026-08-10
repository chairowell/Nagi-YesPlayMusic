import { describe, expect, test } from 'bun:test';
import {
  findLocalShortcutAction,
  isEditableShortcutTarget,
  matchesLocalShortcut,
  parseLocalShortcut,
  runLocalShortcutAction,
} from '../src/services/localShortcuts';
import type { LocalShortcutTarget } from '../src/services/localShortcuts';
import type { Shortcut } from '../src/types/persistence';

function keyboardEvent(
  overrides: Partial<Parameters<typeof matchesLocalShortcut>[1]> = {}
): Parameters<typeof matchesLocalShortcut>[1] {
  return {
    altKey: false,
    code: 'KeyP',
    ctrlKey: false,
    key: 'p',
    metaKey: false,
    shiftKey: false,
    ...overrides,
  };
}

const shortcuts: Shortcut[] = [
  {
    id: 'play',
    name: 'Play',
    shortcut: 'CommandOrControl+P',
    globalShortcut: '',
  },
  {
    id: 'next',
    name: 'Next',
    shortcut: 'Alt+Right',
    globalShortcut: '',
  },
  {
    id: 'unknown',
    name: 'Unknown',
    shortcut: 'F2',
    globalShortcut: '',
  },
];

describe('local shortcut parsing', () => {
  test('maps CommandOrControl to the current platform', () => {
    expect(parseLocalShortcut('CommandOrControl+Shift+P', true)).toEqual({
      alt: false,
      control: false,
      meta: true,
      shift: true,
      key: 'p',
    });
    expect(parseLocalShortcut('CommandOrControl+Shift+P', false)).toEqual({
      alt: false,
      control: true,
      meta: false,
      shift: true,
      key: 'p',
    });
  });

  test('rejects unknown, duplicate, and multi-key accelerators', () => {
    for (const invalid of [
      '',
      'CommandOrControl',
      'Hyper+P',
      'Control+Control+P',
      'P+N',
      'F13',
    ]) {
      expect(parseLocalShortcut(invalid, false)).toBeNull();
    }
  });

  test('requires an exact modifier set', () => {
    expect(
      matchesLocalShortcut(
        'CommandOrControl+P',
        keyboardEvent({ ctrlKey: true }),
        false
      )
    ).toBe(true);
    expect(
      matchesLocalShortcut(
        'CommandOrControl+P',
        keyboardEvent({ ctrlKey: true, shiftKey: true }),
        false
      )
    ).toBe(false);
    expect(
      matchesLocalShortcut(
        'CommandOrControl+P',
        keyboardEvent({ metaKey: true }),
        true
      )
    ).toBe(true);
    expect(
      matchesLocalShortcut(
        'Shift+~',
        keyboardEvent({
          code: 'Backquote',
          key: '~',
          shiftKey: true,
        }),
        false
      )
    ).toBe(true);
  });

  test('resolves only supported actions from current settings', () => {
    expect(
      findLocalShortcutAction(
        shortcuts,
        keyboardEvent({ ctrlKey: true }),
        false
      )
    ).toBe('play');
    expect(
      findLocalShortcutAction(
        shortcuts,
        keyboardEvent({ altKey: true, code: 'ArrowRight', key: 'ArrowRight' }),
        false
      )
    ).toBe('next');
    expect(
      findLocalShortcutAction(
        shortcuts,
        keyboardEvent({ code: 'F2', key: 'F2' }),
        false
      )
    ).toBeNull();
  });
});

test('editable controls and their descendants suppress local shortcuts', () => {
  const input = { tagName: 'input', parentElement: null };
  const editable = { isContentEditable: true, parentElement: null };
  const nested = { tagName: 'SPAN', parentElement: editable };
  const textbox = { role: 'textbox', parentElement: null };
  const ordinary = { tagName: 'BUTTON', parentElement: null };

  expect(isEditableShortcutTarget(input)).toBe(true);
  expect(isEditableShortcutTarget(nested)).toBe(true);
  expect(isEditableShortcutTarget(textbox)).toBe(true);
  expect(isEditableShortcutTarget(ordinary)).toBe(false);
});

test('every persisted local shortcut action reaches its player command', () => {
  const calls: unknown[] = [];
  const target: LocalShortcutTarget = {
    isPersonalFM: false,
    volume: 0.95,
    currentTrackId: 42,
    playOrPause: () => calls.push('play'),
    playNextFMTrack: () => calls.push('nextFM'),
    playNextTrack: () => calls.push('next'),
    playPrevTrack: () => calls.push('previous'),
    likeTrack: (id: number) => calls.push(['like', id]),
    minimize: () => calls.push('minimize'),
  };

  runLocalShortcutAction('play', target);
  runLocalShortcutAction('next', target);
  target.isPersonalFM = true;
  runLocalShortcutAction('next', target);
  runLocalShortcutAction('previous', target);
  runLocalShortcutAction('increaseVolume', target);
  expect(target.volume).toBe(1);
  runLocalShortcutAction('decreaseVolume', target);
  expect(target.volume).toBe(0.9);
  runLocalShortcutAction('like', target);
  runLocalShortcutAction('minimize', target);

  expect(calls).toEqual([
    'play',
    'next',
    'nextFM',
    'previous',
    ['like', 42],
    'minimize',
  ]);
});
