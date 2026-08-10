import { createI18n } from 'vue-i18n';
import { getAppStore } from '@/stores/accessor';

import en from './lang/en';
import zhCN from './lang/zh-CN';
import zhTW from './lang/zh-TW';
import tr from './lang/tr';

const i18n = createI18n({
  legacy: true,
  locale: getAppStore().settings.lang ?? 'en',
  fallbackLocale: 'en',
  messages: {
    en,
    'zh-CN': zhCN,
    'zh-TW': zhTW,
    tr,
  },
  missingWarn: false,
  fallbackWarn: false,
});

// Keep vue-i18n mode details behind a typed adapter.
const locale = Object.assign(i18n, {
  t(key: string): string {
    return String(i18n.global.t(key));
  },
});

export default locale;
