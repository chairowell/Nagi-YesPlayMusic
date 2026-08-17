import type { Artist, Track, TrackPrivilege } from '@/types/domain';

function optionalText(field: string, value: unknown): Record<string, string> {
  return typeof value === 'string' ? { [field]: value } : {};
}

export function adaptTrackItems(items: Record<string, unknown>[]): Track[] {
  return items
    .filter(item => typeof item['id'] === 'number')
    .map(item => {
      const album =
        typeof item['album'] === 'object' && item['album'] !== null
          ? (item['album'] as Record<string, unknown>)
          : {};
      const track: Track = {
        id: item['id'] as number,
        ...optionalText('name', item['name']),
        ar: Array.isArray(item['artists']) ? (item['artists'] as Artist[]) : [],
        al: {
          id: typeof album['id'] === 'number' ? album['id'] : 0,
          ...optionalText('name', album['name']),
          ...optionalText('picUrl', album['picUrl']),
        },
        dt: typeof item['durationMs'] === 'number' ? item['durationMs'] : 0,
        alia: Array.isArray(item['alias']) ? (item['alias'] as string[]) : [],
        tns: Array.isArray(item['transNames'])
          ? (item['transNames'] as string[])
          : [],
        mark: typeof item['mark'] === 'number' ? item['mark'] : 0,
        ...(typeof item['fee'] === 'number' ? { fee: item['fee'] } : {}),
        // Presence alone marks "no copyright" downstream.
        ...(item['noCopyrightRcmd'] === true ? { noCopyrightRcmd: true } : {}),
        ...(typeof item['privilege'] === 'object' && item['privilege'] !== null
          ? { privilege: item['privilege'] as TrackPrivilege }
          : {}),
        ...(typeof item['cd'] === 'string' ? { cd: item['cd'] } : {}),
      };
      return track;
    });
}
