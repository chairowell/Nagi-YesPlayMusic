import { defineConfig, externalizeDepsPlugin } from 'electron-vite';
import vue2 from '@vitejs/plugin-vue2';
import { createSvgIconsPlugin } from 'vite-plugin-svg-icons';
import { loadEnv } from 'vite';
import path from 'node:path';

const root = import.meta.dirname;
const src = path.resolve(root, 'src');

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, root, '');
  const isDev = mode === 'development';

  // 原来 vue-cli-plugin-electron-builder 注入的全局变量 __static：
  // 开发时指向 public/，打包后 public/ 的内容在 out/renderer/。
  // define 只接受字面量，所以用 banner 在产物顶部声明成全局变量。
  const staticBanner = isDev
    ? `globalThis.__static = ${JSON.stringify(path.resolve(root, 'public'))};`
    : `globalThis.__static = require('path').join(__dirname, '../renderer');`;

  return {
    main: {
      // 原生模块和 node 依赖不打进 bundle，由 electron 直接 require
      plugins: [externalizeDepsPlugin()],
      resolve: {
        alias: [{ find: /^@\//, replacement: src + '/' }],
        extensions: ['.mjs', '.js', '.json'],
      },
      define: {
        'process.env.IS_ELECTRON': JSON.stringify(true),
      },
      build: {
        // ncmModDef.js 是 CJS 写法（module.exports），默认只有 node_modules
        // 会走 commonjs 插件，src 下的 .js 会被当成 ESM 解析导致取不到 default
        commonjsOptions: { include: [/ncmModDef\.js$/, /node_modules/] },
        rollupOptions: {
          input: path.resolve(src, 'background.js'),
          output: { banner: staticBanner },
        },
      },
    },

    renderer: {
      root,
      resolve: {
        alias: [
          { find: /^~@\//, replacement: src + '/' },
          { find: /^@\//, replacement: src + '/' },
        ],
        extensions: ['.mjs', '.js', '.json', '.vue'],
      },
      plugins: [
        vue2(),
        createSvgIconsPlugin({
          iconDirs: [path.resolve(src, 'assets/icons')],
          symbolId: 'icon-[name]',
        }),
      ],
      // 生产模式下渲染进程和 API 同源（都在 27232，Express 转发 /api）。
      // dev 必须照做，否则 5173 -> 10754 属于跨站，登录 cookie 会被
      // Chromium 的 SameSite 策略丢掉，表现为头像不刷新、library 空。
      server: {
        // 锁死端口。让 Vite 自己漂到 5174 的话 origin 就变了，
        // IndexedDB 按 origin 隔离，等于凭空多出一份空的歌曲缓存。
        port: 5173,
        strictPort: true,
        proxy: {
          '^/api': {
            target: 'http://127.0.0.1:10754',
            changeOrigin: true,
            rewrite: p => p.replace(/^\/api/, ''),
          },
        },
      },
      define: {
        'process.env': {
          NODE_ENV: isDev ? 'development' : 'production',
          BASE_URL: '/',
          IS_ELECTRON: true,
          VUE_APP_NETEASE_API_URL: env.VUE_APP_NETEASE_API_URL,
          VUE_APP_ELECTRON_API_URL: env.VUE_APP_ELECTRON_API_URL,
          // 不用 .env 里的 http://127.0.0.1:10754，改走上面的同源代理
          VUE_APP_ELECTRON_API_URL_DEV: '/api',
          VUE_APP_LASTFM_API_KEY: env.VUE_APP_LASTFM_API_KEY,
          VUE_APP_LASTFM_API_SHARED_SECRET:
            env.VUE_APP_LASTFM_API_SHARED_SECRET,
        },
        IS_ELECTRON: JSON.stringify(true),
      },
      build: {
        sourcemap: false,
        rollupOptions: {
          input: path.resolve(root, 'index.html'),
        },
      },
    },
  };
});
