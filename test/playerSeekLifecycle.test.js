import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { markRaw, reactive } from 'vue';
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

  test('FLAC 源的拖拽经编排器升级为 WAV 精确源，接线含代际与降级', () => {
    // 竞态规则的行为测试在 test/preciseSeekUpgrade.test.js；这里只钉接线。
    expect(playerSource).toContain('_canUpgradeSeekPrecision(time)');
    expect(playerSource).toContain("this._currentSourceMeta?.format === 'flac'");
    // 每次显式 seek 推进代际，升级任务凭它淘汰过期目标（防跳回旧位置）
    expect(playerSource).toContain('this._seekToken += 1;');

    const wiring = playerSource.slice(
      playerSource.indexOf('_getPreciseSeekUpgrader()'),
      playerSource.indexOf('  mute()')
    );
    expect(wiring).toContain('createPreciseSeekUpgrader({');
    // 低内存优先：先请求 sidecar 原生 afconvert，失败才在渲染进程内存转换
    expect(wiring.indexOf('requestPreciseWavURL')).toBeGreaterThan(-1);
    expect(wiring.indexOf('requestPreciseWavURL')).toBeLessThan(
      wiring.indexOf('decodeFlacToWavBlob')
    );
    // WAV 源标记 format:'wav'，避免升级自身再次触发升级
    expect(wiring).toContain("format: 'wav'");
    // 精确源必须用独立 origin：加载失败不能触发"缓存损坏"的删除逻辑
    expect(wiring).toContain("origin: 'precise-wav'");
    // 恢复播放要经过编排器的 resume 决策（含暂停意图），不得直接恢复
    expect(wiring).toContain('if (resume) player.play();');
    // 暂停手势的意图标记：fade 未完成期间禁止误恢复
    expect(playerSource).toContain('this._pausePending = true;');
  });

  test('精确 WAV 失效的重试从头解析音源且不删有效缓存，并恢复播放状态', () => {
    const retry = playerSource.slice(
      playerSource.indexOf('async _retryAudioSourceAfterFailure('),
      playerSource.indexOf('_getAudioSourceBlobURL(')
    );
    expect(retry).toContain(
      "failedSource.origin === 'precise-wav' ? null : failedSource.origin"
    );
    // 只有真正的缓存源损坏才允许删缓存
    expect(retry).toContain("if (failedSource.origin === 'cache')");
    expect(retry).toContain(
      'this._playAudioSource(fallback, autoplay || this._playing, ifUnplayableThen)'
    );
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
    state.player._seekToken = 7;
    state.player._pausePending = true;
    state.player._preciseSeekUpgrader = { request() {} };

    expect(persisted).toBe(0);
  });

  test('defineProperty 的不可写对象属性必须 markRaw，否则响应式读取抛 Proxy 不变量错', () => {
    // 生产 WKWebView 实测：_nextTrackPrefetcher 没 markRaw 时，每次切歌
    // 读它都抛 "Proxy handler's 'get' result ..."，预取失效、切歌
    // Promise 链被掐断（启动恢复进度因此丢失）。
    const makePlayer = wrapValue => {
      const raw = {
        initialize() {},
        saveSelfToLocalStorage() {},
        sendSelfToIpcMain() {},
      };
      Object.defineProperty(raw, '_gadget', {
        enumerable: false,
        value: wrapValue({ prefetch() {} }),
      });
      const state = reactive({ player: null });
      mountPlayerState({ state }, raw, {});
      return state.player;
    };

    expect(() => makePlayer(v => v)._gadget).toThrow(); // 未 markRaw：抛错特征
    expect(() => makePlayer(markRaw)._gadget.prefetch).not.toThrow();

    // 钉住 Player.js 的两个易感点都已 markRaw
    expect(playerSource).toContain('value: markRaw(createNextTrackPrefetcher({');
    expect(playerSource).toContain(
      'this._preciseSeekUpgrader = markRaw(createPreciseSeekUpgrader({'
    );
  });
});
