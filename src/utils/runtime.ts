interface RuntimeEnvironment {
  IS_TAURI?: unknown;
}

export function resolveRuntime(environment: RuntimeEnvironment = {}) {
  const isTauri =
    environment.IS_TAURI === true || environment.IS_TAURI === 'true';
  return {
    isTauri,
    isDesktop: isTauri,
  };
}

const runtime = resolveRuntime({ IS_TAURI: import.meta.env['IS_TAURI'] });

export const isTauriRuntime = runtime.isTauri;
export const isDesktopRuntime = runtime.isDesktop;
