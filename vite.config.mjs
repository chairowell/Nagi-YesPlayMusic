import { defineConfig, loadEnv } from 'vite';
import vue from '@vitejs/plugin-vue';
import { createSvgIconsPlugin } from 'vite-plugin-svg-icons';
import path from 'node:path';

// 原来的 vue.config.js 把 VUE_APP_* 通过 webpack DefinePlugin 注入，
// 代码里有 43 处 process.env.* 引用。这里整体替换 process.env，
// 免得改动那 43 处业务代码。
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const isElectron = process.env.IS_ELECTRON === 'true';

  return {
    resolve: {
      // 用数组保证匹配顺序：'~@' 要排在 '@' 前面，否则会被 '@' 先吃掉。
      // '~' 是 webpack 的写法，SCSS 里引 Barlow 字体用到了。
      alias: [
        {
          find: /^~@\//,
          replacement: path.resolve(import.meta.dirname, 'src') + '/',
        },
        {
          find: /^@\//,
          replacement: path.resolve(import.meta.dirname, 'src') + '/',
        },
      ],
      // 原来 webpack 允许 import 省略 .vue 后缀，代码里大量这么写
      extensions: ['.mjs', '.js', '.json', '.vue'],
    },
    plugins: [
      vue(),
      createSvgIconsPlugin({
        iconDirs: [path.resolve(import.meta.dirname,'src/assets/icons')],
        symbolId: 'icon-[name]',
      }),
    ],
    define: {
      'process.env': {
        NODE_ENV: mode === 'production' ? 'production' : 'development',
        BASE_URL: '/',
        IS_ELECTRON: isElectron,
        VUE_APP_NETEASE_API_URL: env.VUE_APP_NETEASE_API_URL,
        VUE_APP_ELECTRON_API_URL: env.VUE_APP_ELECTRON_API_URL,
        VUE_APP_ELECTRON_API_URL_DEV: env.VUE_APP_ELECTRON_API_URL_DEV,
        VUE_APP_LASTFM_API_KEY: env.VUE_APP_LASTFM_API_KEY,
        VUE_APP_LASTFM_API_SHARED_SECRET: env.VUE_APP_LASTFM_API_SHARED_SECRET,
      },
      IS_ELECTRON: JSON.stringify(isElectron),
    },
    server: {
      port: Number(env.DEV_SERVER_PORT) || 8080,
      proxy: {
        '^/api': {
          target: 'http://localhost:3000',
          changeOrigin: true,
          rewrite: p => p.replace(/^\/api/, ''),
        },
      },
    },
    build: {
      sourcemap: false,
      outDir: 'dist',
      rollupOptions: {
        output: {
          manualChunks: {
            'audio-vendor': ['howler', 'vue-slider-component'],
            'data-vendor': ['axios', 'dexie'],
            'vue-vendor': ['vue', 'vue-i18n', 'vue-router', 'vuex'],
          },
        },
      },
    },
  };
});
