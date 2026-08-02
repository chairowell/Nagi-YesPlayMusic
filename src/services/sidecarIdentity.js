export const SIDECAR_HEALTH_PATH = '/__yesplaymusic/health';
export const SIDECAR_HEALTH_BODY = JSON.stringify({
  service: 'yesplaymusic-sidecar',
  protocol: 1,
});

export function installSidecarHealthRoute(app) {
  app.get(SIDECAR_HEALTH_PATH, (_request, response) => {
    // Bun 单文件编译后不会可靠保留 Express response 的便捷方法，使用 Node 原生接口。
    response.statusCode = 200;
    response.setHeader('Content-Type', 'application/json; charset=utf-8');
    response.setHeader('Cache-Control', 'no-store');
    response.end(SIDECAR_HEALTH_BODY);
  });

  // 网易云 API 在返回 app 前已装好全部路由；健康检查必须排在业务路由前面。
  const healthLayer = app._router.stack.pop();
  app._router.stack.unshift(healthLayer);
}
