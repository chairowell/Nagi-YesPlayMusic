interface ServerLike {
  once(event: 'listening', listener: () => void): void;
  once(event: 'error', listener: (error: unknown) => void): void;
  off(event: 'listening', listener: () => void): void;
  off(event: 'error', listener: (error: unknown) => void): void;
}

export function waitForServer<T extends ServerLike>(
  server: T | null | undefined
): Promise<T> {
  if (!server) return Promise.reject(new Error('服务没有返回监听实例'));

  return new Promise<T>((resolve, reject) => {
    const cleanup = () => {
      server.off('listening', onListening);
      server.off('error', onError);
    };
    const onListening = () => {
      cleanup();
      resolve(server);
    };
    const onError = (error: unknown) => {
      cleanup();
      reject(error);
    };

    server.once('listening', onListening);
    server.once('error', onError);
  });
}
