export interface UpdaterBuildPlan {
  targetTriple: string;
  args: string[];
  afterBuild: string[][];
}

export const UPDATER_BUILD_PLANS: Readonly<Record<string, UpdaterBuildPlan>>;

export function createUpdaterBuildConfig(
  publicKey: string
): Promise<Record<string, unknown>>;

export function buildTauriUpdater(
  target: string,
  options?: { developerId?: boolean }
): Promise<UpdaterBuildPlan>;
