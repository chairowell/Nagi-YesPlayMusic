import clc from 'cli-color';
// 这个模块没有导出，靠导入时的副作用创建 anonymous_token 文件
import '../utils/checkAuthToken';
import server from '@neteaseapireborn/api/server';
import apiCache from '@neteaseapireborn/api/util/apicache';
// 必须静态导入全部路由，让 Electron 和 Bun 单文件 sidecar 都能收集依赖。
import ncmModDef from '../ncmModDef';
import { configureSafeNeteaseApiCache } from '../utils/audioCacheIntegrity';
import { waitForServer } from '../utils/serverLifecycle';

configureSafeNeteaseApiCache(apiCache);

export async function startNeteaseMusicApi({
  port = 10754,
  host = '127.0.0.1',
} = {}) {
  console.log(`${clc.redBright('[NetEase API]')} initiating NCM API`);

  const apiApp = await server.serveNcmApi({
    port,
    host,
    moduleDefs: ncmModDef,
  });
  // 上游在调用 listen() 后立即 resolve；必须等到 listening，
  // 否则端口冲突时 Tauri 会把启动失败误判成 ready。
  await waitForServer(apiApp.server);
  return apiApp;
}
