import type { Shortcut } from '@/types/persistence';

const LOCAL_SHORTCUT_ACTIONS = [
  'play',
  'next',
  'previous',
  'increaseVolume',
  'decreaseVolume',
  'like',
  'minimize',
] as const;

export type LocalShortcutAction = (typeof LOCAL_SHORTCUT_ACTIONS)[number];

interface ParsedShortcut {
  alt: boolean;
  control: boolean;
  meta: boolean;
  shift: boolean;
  key: string;
}

interface KeyboardShortcutEvent {
  altKey: boolean;
  code: string;
  ctrlKey: boolean;
  key: string;
  metaKey: boolean;
  shiftKey: boolean;
}

export interface LocalShortcutTarget {
  isPersonalFM: boolean;
  volume: number;
  currentTrackId: number;
  playOrPause(): unknown;
  playNextFMTrack(): unknown;
  playNextTrack(): unknown;
  playPrevTrack(): unknown;
  likeTrack(id: number): unknown;
  minimize(): unknown;
}

const KEY_ALIASES: Readonly<Record<string, string>> = {
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ArrowUp: 'up',
  Down: 'down',
  Left: 'left',
  Right: 'right',
  Space: 'space',
  Up: 'up',
};

const PUNCTUATION_KEYS = new Set([
  '=',
  '-',
  '~',
  '[',
  ']',
  ';',
  "'",
  ',',
  '.',
  '/',
]);

const KEY_CODES: Readonly<Record<string, string>> = {
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ArrowUp: 'up',
  Backquote: '~',
  BracketLeft: '[',
  BracketRight: ']',
  Comma: ',',
  Equal: '=',
  Minus: '-',
  Period: '.',
  Quote: "'",
  Semicolon: ';',
  Slash: '/',
  Space: 'space',
};

function normalizedKey(token: string): string | null {
  const alias = KEY_ALIASES[token];
  if (alias) return alias;
  if (/^[A-Z]$/i.test(token)) return token.toLowerCase();
  if (/^[0-9]$/.test(token)) return token;
  if (/^F(?:[1-9]|1[0-2])$/i.test(token)) return token.toLowerCase();
  if (PUNCTUATION_KEYS.has(token)) return token;
  return null;
}

export function parseLocalShortcut(
  accelerator: string,
  isMac: boolean
): ParsedShortcut | null {
  const tokens = accelerator.split('+');
  if (tokens.length === 0 || tokens.some(token => token.length === 0)) {
    return null;
  }

  const parsed: ParsedShortcut = {
    alt: false,
    control: false,
    meta: false,
    shift: false,
    key: '',
  };
  for (const token of tokens) {
    if (token === 'Alt') {
      if (parsed.alt) return null;
      parsed.alt = true;
    } else if (token === 'Control') {
      if (parsed.control) return null;
      parsed.control = true;
    } else if (token === 'Command') {
      if (parsed.meta) return null;
      parsed.meta = true;
    } else if (token === 'CommandOrControl') {
      if (isMac ? parsed.meta : parsed.control) return null;
      if (isMac) parsed.meta = true;
      else parsed.control = true;
    } else if (token === 'Shift') {
      if (parsed.shift) return null;
      parsed.shift = true;
    } else {
      const key = normalizedKey(token);
      if (!key || parsed.key) return null;
      parsed.key = key;
    }
  }
  return parsed.key ? parsed : null;
}

function eventKey(event: KeyboardShortcutEvent): string {
  const codeKey = KEY_CODES[event.code];
  if (codeKey) return codeKey;
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(event.code)) return event.code.slice(5);
  if (/^F(?:[1-9]|1[0-2])$/.test(event.code)) {
    return event.code.toLowerCase();
  }
  if (event.key === ' ') return 'space';
  return (KEY_ALIASES[event.key] ?? event.key).toLowerCase();
}

export function matchesLocalShortcut(
  accelerator: string,
  event: KeyboardShortcutEvent,
  isMac: boolean
): boolean {
  const shortcut = parseLocalShortcut(accelerator, isMac);
  return (
    shortcut !== null &&
    shortcut.alt === event.altKey &&
    shortcut.control === event.ctrlKey &&
    shortcut.meta === event.metaKey &&
    shortcut.shift === event.shiftKey &&
    shortcut.key === eventKey(event)
  );
}

function isLocalShortcutAction(value: string): value is LocalShortcutAction {
  return LOCAL_SHORTCUT_ACTIONS.some(action => action === value);
}

export function findLocalShortcutAction(
  shortcuts: readonly Shortcut[],
  event: KeyboardShortcutEvent,
  isMac: boolean
): LocalShortcutAction | null {
  const match = shortcuts.find(
    shortcut =>
      isLocalShortcutAction(shortcut.id) &&
      matchesLocalShortcut(shortcut.shortcut, event, isMac)
  );
  return match && isLocalShortcutAction(match.id) ? match.id : null;
}

export function runLocalShortcutAction(
  action: LocalShortcutAction,
  target: LocalShortcutTarget
): void {
  switch (action) {
    case 'play':
      target.playOrPause();
      break;
    case 'next':
      if (target.isPersonalFM) target.playNextFMTrack();
      else target.playNextTrack();
      break;
    case 'previous':
      target.playPrevTrack();
      break;
    case 'increaseVolume':
      target.volume = Math.min(1, target.volume + 0.1);
      break;
    case 'decreaseVolume':
      target.volume = Math.max(0, target.volume - 0.1);
      break;
    case 'like':
      target.likeTrack(target.currentTrackId);
      break;
    case 'minimize':
      target.minimize();
      break;
  }
}

function readProperty(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null
    ? Reflect.get(value, key)
    : undefined;
}

export function isEditableShortcutTarget(target: unknown): boolean {
  let current: unknown = target;
  while (typeof current === 'object' && current !== null) {
    const tagName = readProperty(current, 'tagName');
    if (
      (typeof tagName === 'string' &&
        ['INPUT', 'TEXTAREA', 'SELECT'].includes(tagName.toUpperCase())) ||
      readProperty(current, 'isContentEditable') === true ||
      readProperty(current, 'contentEditable') === 'true' ||
      readProperty(current, 'role') === 'textbox'
    ) {
      return true;
    }
    current = readProperty(current, 'parentElement');
  }
  return false;
}
