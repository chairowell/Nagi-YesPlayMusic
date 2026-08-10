import { expect, test } from 'bun:test';
import { shouldOpenLibraryOnStartup } from '../src/services/startupNavigation';

test('only the default home route opens the library at startup', () => {
  expect(shouldOpenLibraryOnStartup(true, 'home')).toBe(true);
  expect(shouldOpenLibraryOnStartup(false, 'home')).toBe(false);
  expect(shouldOpenLibraryOnStartup(true, 'settings')).toBe(false);
  expect(shouldOpenLibraryOnStartup(true, undefined)).toBe(false);
});
