import { markRaw } from 'vue';

const TRANSIENT_PLAYER_FIELDS = new Set([
  '_howler',
  '_progress',
  '_audioDuration',
  '_initialized',
]);

/**
 * 先把 Player 放进 Vue 的响应式状态，再启动内部时钟。
 * Vue 3 不会替原始对象上的异步写入补发响应式通知，初始化顺序反过来会让进度条停住。
 */
export function mountPlayerState(store, rawPlayer, exposureTarget) {
  const persistedPlayer = new Proxy(rawPlayer, {
    set(target, prop, value) {
      // Howler 会用实例身份过滤过期的异步回调；被 Vue 深层代理后，
      // 同一实例也会与创建时的 raw object 严格不等，导致结束事件被误丢弃。
      target[prop] = prop === '_howler' && value ? markRaw(value) : value;
      if (TRANSIENT_PLAYER_FIELDS.has(prop)) return true;
      target.saveSelfToLocalStorage();
      target.sendSelfToIpcMain();
      return true;
    },
  });

  store.state.player = persistedPlayer;
  const reactivePlayer = store.state.player;
  reactivePlayer.initialize();

  if (exposureTarget) exposureTarget.player = reactivePlayer;
  return reactivePlayer;
}
