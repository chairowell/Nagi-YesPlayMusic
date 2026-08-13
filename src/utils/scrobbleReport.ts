export interface NeteaseScrobble {
  id: number;
  sourceid: number | string;
  time: number;
}

type Sender = (params: NeteaseScrobble) => Promise<unknown>;
type FailureHandler = (error: unknown) => void;

const logFailure: FailureHandler = () => {
  console.warn('[Player] 网易云听歌记录上报失败');
};

/** Fire-and-forget reporting: playback never waits for the network request. */
export function reportNeteaseScrobble(
  params: NeteaseScrobble,
  send: Sender,
  onFailure: FailureHandler = logFailure
): void {
  void send(params).catch(onFailure);
}
