import { createApp } from 'vue';
import '@/assets/css/global.scss';
import NProgress from 'nprogress';
import '@/assets/css/nprogress.css';
import { migrateLegacyDesktopSettings } from '@/services/legacyDataMigration';
import { isDesktopRuntime } from '@/utils/runtime';
import { purgeLegacyDesktopAuthStorage } from '@/utils/authStorage';

window.resetApp = () => {
  localStorage.clear();
  indexedDB.deleteDatabase('yesplaymusic');
  document.cookie.split(';').forEach(function (c) {
    document.cookie = c
      .replace(/^ +/, '')
      .replace(/=.*/, '=;expires=' + new Date().toUTCString() + ';path=/');
  });
  return '已重置应用，请刷新页面（按Ctrl/Command + R）';
};
console.log(
  '如出现问题，可尝试在本页输入 %cresetApp()%c 然后按回车重置应用。',
  'background: #eaeffd;color:#335eea;padding: 4px 6px;border-radius:3px;',
  'background:unset;color:unset;'
);

NProgress.configure({ showSpinner: false, trickleSpeed: 100 });

async function bootstrap() {
  // store 读取 localStorage 发生在模块求值阶段，所以必须先完成一次性迁移，
  // 再动态加载整个业务依赖图。
  await migrateLegacyDesktopSettings();
  purgeLegacyDesktopAuthStorage(localStorage, isDesktopRuntime);
  const [
    { default: App },
    { default: router },
    { default: store },
    { default: i18n },
    { default: SvgIcons },
    { default: filters },
    { copyText },
    { dailyTask },
  ] = await Promise.all([
    import('./App.vue'),
    import('./router'),
    import('./store'),
    import('@/locale'),
    import('@/assets/icons'),
    import('@/utils/filters'),
    import('@/utils/clipboard'),
    import('@/utils/common'),
  ]);

  dailyTask();
  const app = createApp(App);
  app.config.globalProperties.$filters = filters;
  app.config.globalProperties.$copyText = copyText;
  app.use(i18n);
  app.use(store);
  app.use(router);
  app.use(SvgIcons);
  app.mount('#app');
}

void bootstrap();
