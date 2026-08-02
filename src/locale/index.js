import { createI18n } from 'vue-i18n';
import store from '@/store';

import en from './lang/en.js';
import zhCN from './lang/zh-CN.js';
import zhTW from './lang/zh-TW.js';
import tr from './lang/tr.js';

const i18n = createI18n({
  legacy: true,
  locale: store.state.settings.lang,
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

// 业务模块原先直接调用 locale.t；保留这个薄适配层，避免把国际化实现
// 泄漏到所有 API 和 Vuex 文件里。
i18n.t = (...args) => i18n.global.t(...args);

export default i18n;
