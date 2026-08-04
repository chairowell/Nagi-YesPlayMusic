import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { reactive } from 'vue';
import { startHowlerSeek } from '../src/utils/playbackSeek';
import { mountPlayerState } from '../src/utils/playerState';

const playerSource = readFileSync(
  fileURLToPath(new URL('../src/utils/Player.js', import.meta.url)),
  'utf8'
);

describe('seek 事务与 Howler 实例生命周期', () => {
  test('落在已 loaderror 的 Howl 上的 seek 事务永远不会自行 settle', () => {
    // howler html5 加载失败后 _state 停在 'loading'，load/loaderror 都已消费；
    // 此时 once() 挂上的监听永远不响，事务只能靠外部 cancel 收尾。
    const deadHowler = {
      _sounds: [
        {
          _node: {
            currentTime: 37.2,
            seeking: false,
            error: { code: 2 },
            addEventListener() {},
            removeEventListener() {},
          },
        },
      ],
      _state: 'loading',
      once() {},
      off() {},
      seek() {
        return 37.2;
      },
    };

    const settled = [];
    const transaction = startHowlerSeek(deadHowler, 95, v => settled.push(v));

    expect(transaction.pending).toBe(true);
    expect(settled).toEqual([]);
  });

  test('替换 Howler 实例的路径必须先取消挂起的 seek 事务并复位 _seeking', () => {
    // 换源重试(_retryAudioSourceAfterFailure→_playAudioSource)替换实例时若不清理，
    // 上面那种永不 settle 的事务会让 _seeking 永久为 true：
    // 歌词时钟(lyrics.vue "if (this.player.seeking) return")与进度心跳全部冻结。
    const playAudioSource = playerSource.slice(
      playerSource.indexOf('_playAudioSource('),
      playerSource.indexOf('Howler.unload()', playerSource.indexOf('_playAudioSource('))
    );

    expect(playAudioSource).toContain('this._pendingSeekCancel?.();');
    expect(playAudioSource).toContain('this._pendingSeekCancel = null;');
    expect(playAudioSource).toContain('this._seeking = false;');
  });

  test('FLAC 源的拖拽升级为 WAV 精确源，且带淘汰保护与降级路径', () => {
    // AVPlayer 对 FLAC 的 seek 落点偏早且 currentTime 谎报请求值；
    // 升级把缓存离线转成 WAV 后播放仍走系统媒体栈，seek 即算术。
    expect(playerSource).toContain('_canUpgradeSeekPrecision(time)');
    expect(playerSource).toContain("this._currentSourceMeta?.format === 'flac'");

    const upgrade = playerSource.slice(
      playerSource.indexOf('async _seekWithPreciseUpgrade('),
      playerSource.indexOf('  mute()')
    );
    // 淘汰保护：等待缓存/解码期间切歌或换源，本次升级必须作废
    expect(upgrade).toContain(
      'this._howler !== howlerBefore || this.currentTrackID !== trackId'
    );
    // 缓存未写完或解码失败时回退统一 seek 事务
    expect(upgrade).toContain('this._startSeekTransaction(target, sendMpris)');
    // 升级换源后要恢复播放
    expect(upgrade).toContain('if (this._playing) this.play();');
    // WAV 源标记 format:'wav'，避免升级自身再次触发升级
    expect(upgrade).toContain("format: 'wav'");
  });

  test('音源元数据与升级中间态不触发整份播放器持久化', () => {
    const state = reactive({ player: null });
    let persisted = 0;
    const rawPlayer = {
      initialize() {},
      saveSelfToLocalStorage() {
        persisted += 1;
      },
      sendSelfToIpcMain() {},
    };

    mountPlayerState({ state }, rawPlayer, {});
    state.player._currentSourceMeta = { origin: 'cache', format: 'flac' };
    state.player._pendingPreciseSeekTime = 95;
    state.player._preciseUpgradeInFlight = true;

    expect(persisted).toBe(0);
  });
});
