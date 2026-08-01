import clc from 'cli-color';
import checkAuthToken from '../utils/checkAuthToken';
import server from '@neteaseapireborn/api/server';
// 必须是静态 import：运行时 require('../ncmModDef') 打包后会相对 out/main 解析而找不到
import ncmModDef from '../ncmModDef';

export async function startNeteaseMusicApi() {
  // Let user know that the service is starting
  console.log(`${clc.redBright('[NetEase API]')} initiating NCM API`);

  // Load the NCM API.
  await server.serveNcmApi({
    port: 10754,
    moduleDefs: ncmModDef,
  });
}
