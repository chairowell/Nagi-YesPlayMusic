import dayjs from 'dayjs';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import locale from '@/locale';
import { buildArtworkURL } from '@/utils/artwork';

dayjs.extend(duration);
dayjs.extend(relativeTime);

const currentLocale = () => locale.global.locale;

export function formatTime(
  Milliseconds: number | null | undefined,
  format: 'HH:MM:SS' | 'Human' = 'HH:MM:SS'
): string {
  if (!Milliseconds) return '';

  let time = dayjs.duration(Milliseconds);
  let hours = time.hours().toString();
  let mins = time.minutes().toString();
  let seconds = time.seconds().toString().padStart(2, '0');

  if (format === 'HH:MM:SS') {
    return hours !== '0'
      ? `${hours}:${mins.padStart(2, '0')}:${seconds}`
      : `${mins}:${seconds}`;
  } else if (format === 'Human') {
    let hoursUnit, minitesUnit;
    switch (currentLocale()) {
      case 'zh-CN':
        hoursUnit = '小时';
        minitesUnit = '分钟';
        break;
      case 'zh-TW':
        hoursUnit = '小時';
        minitesUnit = '分鐘';
        break;
      default:
        hoursUnit = 'hr';
        minitesUnit = 'min';
        break;
    }
    return hours !== '0'
      ? `${hours} ${hoursUnit} ${mins} ${minitesUnit}`
      : `${mins} ${minitesUnit}`;
  }
  return '';
}

export function formatDate(
  timestamp: dayjs.ConfigType,
  format = 'MMM D, YYYY'
): string {
  if (!timestamp) return '';
  if (currentLocale() === 'zh-CN') format = 'YYYY年MM月DD日';
  else if (currentLocale() === 'zh-TW') format = 'YYYY年MM月DD日';
  return dayjs(timestamp).format(format);
}

export function formatAlbumType(
  type: string | null | undefined,
  album: { size?: number }
): string {
  if (!type) return '';
  if (type === 'EP/Single') {
    return album.size === 1 ? 'Single' : 'EP';
  } else if (type === 'Single') {
    return 'Single';
  } else if (type === '专辑') {
    return 'Album';
  } else {
    return type;
  }
}

export function resizeImage(imgUrl: unknown, size = 512): string {
  return buildArtworkURL(imgUrl, size);
}

export function formatPlayCount(
  count: number | null | undefined
): string | number {
  if (!count) return '';
  if (currentLocale() === 'zh-CN') {
    if (count > 100000000) {
      return `${Math.floor((count / 100000000) * 100) / 100}亿`; // Example: 2.32 hundred million.
    }
    if (count > 100000) {
      return `${Math.floor((count / 10000) * 10) / 10}万`; // Example: 232.1 ten-thousands.
    }
    if (count > 10000) {
      return `${Math.floor((count / 10000) * 100) / 100}万`; // Example: 2.3 ten-thousands.
    }
    return count;
  } else if (currentLocale() === 'zh-TW') {
    if (count > 100000000) {
      return `${Math.floor((count / 100000000) * 100) / 100}億`; // Example: 2.32 hundred million.
    }
    if (count > 100000) {
      return `${Math.floor((count / 10000) * 10) / 10}萬`; // Example: 232.1 ten-thousands.
    }
    if (count > 10000) {
      return `${Math.floor((count / 10000) * 100) / 100}萬`; // Example: 2.3 ten-thousands.
    }
    return count;
  } else {
    if (count > 10000000) {
      return `${Math.floor((count / 1000000) * 10) / 10}M`; // 233.2M
    }
    if (count > 1000000) {
      return `${Math.floor((count / 1000000) * 100) / 100}M`; // 2.3M
    }
    if (count > 1000) {
      return `${Math.floor((count / 1000) * 100) / 100}K`; // 233.23K
    }
    return count;
  }
}

export function toHttps(url: string | null | undefined): string {
  if (!url) return '';
  return url.replace(/^http:/, 'https:');
}

export default {
  formatAlbumType,
  formatDate,
  formatPlayCount,
  formatTime,
  resizeImage,
  toHttps,
};
