export function resolveRuntime(environment = {}) {
  const isElectron = environment.IS_ELECTRON === true;
  const isTauri = environment.IS_TAURI === true;
  return {
    isElectron,
    isTauri,
    isDesktop: isElectron || isTauri || environment.IS_DESKTOP === true,
  };
}

const runtime = resolveRuntime(process.env);

export const isElectronRuntime = runtime.isElectron;
export const isTauriRuntime = runtime.isTauri;
export const isDesktopRuntime = runtime.isDesktop;
