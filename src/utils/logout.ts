interface SessionStateWriter {
  clearUserSession: () => void;
}

interface LogoutDependencies {
  isTauri: boolean;
  clearDesktopSession: () => Promise<unknown>;
  requestWebLogout: () => void;
  removeWebCookie: (key: string) => void;
  reportError: (error: unknown) => void;
}

export async function performLogout(
  stateWriter: SessionStateWriter,
  dependencies: LogoutDependencies
): Promise<boolean> {
  if (dependencies.isTauri) {
    try {
      await dependencies.clearDesktopSession();
    } catch (error: unknown) {
      // Keep the session when HttpOnly cookie removal fails.
      dependencies.reportError(error);
      return false;
    }
  } else {
    dependencies.requestWebLogout();
    dependencies.removeWebCookie('MUSIC_U');
    dependencies.removeWebCookie('__csrf');
  }

  stateWriter.clearUserSession();
  return true;
}
