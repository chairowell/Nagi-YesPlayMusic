declare module '@neteaseapireborn/api/server' {
  import type { Express } from 'express';
  import type { Server } from 'node:http';

  export interface NcmModuleDefinition {
    identifier?: string;
    route: string;
    module: unknown;
  }

  export interface NcmApiOptions {
    port?: number;
    host?: string;
    checkVersion?: boolean;
    moduleDefs?: NcmModuleDefinition[];
  }

  export type NcmApiApp = Express & { server: Server };

  const server: {
    serveNcmApi(options: NcmApiOptions): Promise<NcmApiApp>;
  };

  export default server;
}

declare module '@neteaseapireborn/api/util/apicache' {
  const apiCache: {
    options(options: { enabled: boolean }): unknown;
  };

  export default apiCache;
}
