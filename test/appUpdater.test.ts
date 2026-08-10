import { afterEach, describe, expect, test } from 'bun:test';
import {
  checkForAppUpdate,
  checkForAppUpdateInBackground,
  clearPendingAppUpdate,
  installPendingAppUpdate,
} from '../src/services/appUpdater';
import type { AppUpdaterBindings } from '../src/services/appUpdater';

afterEach(async () => {
  await clearPendingAppUpdate();
});

describe('Tauri updater flow', () => {
  test('reports an unconfigured development build without checking the endpoint', async () => {
    let checked = false;
    const bindings: AppUpdaterBindings = {
      isConfigured: async () => false,
      check: async () => {
        checked = true;
        return null;
      },
      relaunch: async () => {},
    };

    expect(await checkForAppUpdate(bindings)).toEqual({
      status: 'unconfigured',
    });
    expect(checked).toBe(false);
  });

  test('checks, downloads, installs, and relaunches a signed update', async () => {
    let relaunched = false;
    let closed = false;
    const bindings: AppUpdaterBindings = {
      isConfigured: async () => true,
      check: async () => ({
        version: '0.7.0',
        body: 'Release notes',
        date: '2026-08-10T00:00:00Z',
        async downloadAndInstall(onEvent) {
          onEvent?.({ event: 'Started', data: { contentLength: 10 } });
          onEvent?.({ event: 'Progress', data: { chunkLength: 4 } });
          onEvent?.({ event: 'Progress', data: { chunkLength: 6 } });
          onEvent?.({ event: 'Finished' });
        },
        async close() {
          closed = true;
        },
      }),
      relaunch: async () => {
        relaunched = true;
      },
    };

    expect(await checkForAppUpdate(bindings)).toEqual({
      status: 'available',
      version: '0.7.0',
      notes: 'Release notes',
      date: '2026-08-10T00:00:00Z',
    });

    const progress: Array<number | null> = [];
    await installPendingAppUpdate(
      state => progress.push(state.percent),
      bindings
    );
    expect(progress).toEqual([0, 40, 100, 100]);
    expect(relaunched).toBe(true);

    await clearPendingAppUpdate();
    expect(closed).toBe(true);
  });

  test('background checks suppress network errors', async () => {
    const errors: unknown[] = [];
    const result = await checkForAppUpdateInBackground(
      {
        isConfigured: async () => true,
        check: async () => {
          throw new Error('offline');
        },
        relaunch: async () => {},
      },
      error => errors.push(error)
    );
    expect(result).toBeNull();
    expect(errors).toHaveLength(1);
  });

  test('startup and manual checks share one updater request', async () => {
    let checks = 0;
    let finishCheck: () => void = () => {};
    const waitForCheck = new Promise<void>(resolve => {
      finishCheck = resolve;
    });
    const bindings: AppUpdaterBindings = {
      isConfigured: async () => true,
      check: async () => {
        checks += 1;
        await waitForCheck;
        return null;
      },
      relaunch: async () => {},
    };

    const startup = checkForAppUpdateInBackground(bindings);
    const manual = checkForAppUpdate(bindings);
    finishCheck();

    expect(await Promise.all([startup, manual])).toEqual([
      { status: 'up-to-date' },
      { status: 'up-to-date' },
    ]);
    expect(checks).toBe(1);
  });
});
