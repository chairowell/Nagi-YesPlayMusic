export function waitForServer(server) {
  if (!server) return Promise.reject(new Error('服务没有返回监听实例'));

  return new Promise((resolve, reject) => {
    const cleanup = () => {
      server.off('listening', onListening);
      server.off('error', onError);
    };
    const onListening = () => {
      cleanup();
      resolve(server);
    };
    const onError = error => {
      cleanup();
      reject(error);
    };

    server.once('listening', onListening);
    server.once('error', onError);
  });
}
