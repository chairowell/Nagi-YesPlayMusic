import { markRaw, toRaw } from 'vue';
import type Player from '@/utils/Player';

const TRANSIENT_PLAYER_FIELDS = new Set([
  '_howler',
  '_progress',
  '_audioDuration',
  '_seeking',
  '_pendingSeekCancel',
  '_initialized',
  '_currentSourceMeta',
  '_seekToken',
  '_pausePending',
  '_preciseSeekUpgrader',
]);

/**
 * Make Player reactive before starting timers so async writes notify Vue.
 */
export function mountPlayerState(
  appStore: { player: unknown },
  rawPlayer: Player,
  exposureTarget?: { player?: Player }
): Player {
  // Wrap the raw identity so Vue preserves both reactivity and persistence traps.
  const persistedPlayer = new Proxy(toRaw(rawPlayer), {
    set(target, prop, value) {
      // Howler uses strict identity to reject stale asynchronous callbacks.
      Reflect.set(
        target,
        prop,
        prop === '_howler' && value ? markRaw(value) : value
      );
      if (typeof prop === 'string' && TRANSIENT_PLAYER_FIELDS.has(prop)) {
        return true;
      }
      target.saveSelfToLocalStorage();
      return true;
    },
  });

  appStore.player = persistedPlayer;
  const reactivePlayer = appStore.player as Player;
  reactivePlayer.initialize();

  if (exposureTarget) exposureTarget.player = reactivePlayer;
  return reactivePlayer;
}
