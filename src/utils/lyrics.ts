export interface ParsedLyric {
  time: number;
  rawTime: string;
  content: string;
}

interface LyricPayload {
  lyric?: string;
}

export interface LyricsResponse {
  lrc?: LyricPayload;
  tlyric?: LyricPayload;
  romalrc?: LyricPayload;
  lyricUser?: unknown;
  transUser?: unknown;
}

export function lyricParser(lrc: LyricsResponse = {}) {
  return {
    lyric: parseLyric(lrc?.lrc?.lyric || ''),
    tlyric: parseLyric(lrc?.tlyric?.lyric || ''),
    romalyric: parseLyric(lrc?.romalrc?.lyric || ''),
    lyricuser: lrc.lyricUser,
    transuser: lrc.transUser,
  };
}

// Menu bar lyrics still need timing updates while the lyrics view is hidden.
export function shouldRunLyricClock(
  showLyrics: boolean,
  desktopRuntime: boolean
): boolean {
  return showLyrics || desktopRuntime;
}

// The lyrics view needs smooth updates; the menu bar needs only 4 FPS.
export function lyricClockInterval(showLyrics: boolean): number {
  return showLyrics ? 50 : 250;
}

export function findActiveLyricIndex(
  lyrics: unknown,
  progress: number
): number {
  if (!Array.isArray(lyrics) || !Number.isFinite(progress)) return -1;

  for (let index = lyrics.length - 1; index >= 0; index -= 1) {
    const lyric = (lyrics as ParsedLyric[])[index];
    if (lyric && progress >= lyric.time) return index;
  }
  return -1;
}

export function hasNoLyric(lyricCount: number, lyricLoading: boolean): boolean {
  return !lyricLoading && lyricCount === 0;
}

export function resolveLyricDisplay(
  currentLyric: string,
  lyricCount: number,
  lyricLoading: boolean
): string {
  if (currentLyric) return currentLyric;
  return hasNoLyric(lyricCount, lyricLoading) ? '纯音乐，请欣赏' : '';
}

// regexr.com/6e52n
const extractLrcRegex =
  /^(?<lyricTimestamps>(?:\[.+?\])+)(?!\[)(?<content>.+)$/gm;
const extractTimestampRegex =
  /\[(?<min>\d+):(?<sec>\d+)(?:\.|:)*(?<ms>\d+)*\]/g;

/**
 * @typedef {{time: number, rawTime: string, content: string}} ParsedLyric
 */

/**
 * Parse the lyric string.
 *
 * @param {string} lrc The `lrc` input.
 * @returns {ParsedLyric[]} The parsed lyric.
 * @example parseLyric("[00:00.00] Hello, World!\n[00:00.10] Test\n");
 */
export function parseLyric(lrc: string): ParsedLyric[] {
  /**
   * A sorted list of parsed lyric and its timestamp.
   *
   * @type {ParsedLyric[]}
   * @see binarySearch
   */
  const parsedLyrics: ParsedLyric[] = [];

  /**
   * Find the appropriate index to push our parsed lyric.
   * @param {ParsedLyric} lyric
   */
  const binarySearch = (lyric: ParsedLyric): number => {
    let time = lyric.time;

    let low = 0;
    let high = parsedLyrics.length - 1;

    while (low <= high) {
      const mid = Math.floor((low + high) / 2);
      const midTime = parsedLyrics[mid]?.time;
      if (midTime === undefined) return low;
      if (midTime === time) {
        return mid;
      } else if (midTime < time) {
        low = mid + 1;
      } else {
        high = mid - 1;
      }
    }

    return low;
  };

  for (const line of lrc.trim().matchAll(extractLrcRegex)) {
    const lyricTimestamps = line.groups?.['lyricTimestamps'];
    const content = line.groups?.['content'];
    if (!lyricTimestamps || content === undefined) continue;

    for (const timestamp of lyricTimestamps.matchAll(extractTimestampRegex)) {
      const min = timestamp.groups?.['min'];
      const sec = timestamp.groups?.['sec'];
      const ms = timestamp.groups?.['ms'];
      if (min === undefined || sec === undefined) continue;
      const validMs = ms?.slice(0, 2) ?? '00';
      const rawTime = `[${min}:${sec}.${validMs}]`;
      const time = Number(min) * 60 + Number(sec) + Number(validMs) * 0.01;

      /** @type {ParsedLyric} */
      const parsedLyric = { rawTime, time, content: trimContent(content) };
      parsedLyrics.splice(binarySearch(parsedLyric), 0, parsedLyric);
    }
  }

  return parsedLyrics;
}

/**
 * @param {string} content
 * @returns {string}
 */
function trimContent(content: string): string {
  let t = content.trim();
  return t.length < 1 ? content : t;
}

/**
 * @param {string} lyric
 */
export async function copyLyric(lyric: string): Promise<void> {
  const textToCopy = lyric;
  if (navigator.clipboard && navigator.clipboard.writeText) {
    try {
      await navigator.clipboard.writeText(textToCopy);
    } catch (err) {
      alert('复制失败，请手动复制！');
    }
  } else {
    const tempInput = document.createElement('textarea');
    tempInput.value = textToCopy;
    tempInput.style.position = 'absolute';
    tempInput.style.left = '-9999px';
    document.body.appendChild(tempInput);
    tempInput.select();
    try {
      document.execCommand('copy');
    } catch (err) {
      alert('复制失败，请手动复制！');
    }
    document.body.removeChild(tempInput);
  }
}
