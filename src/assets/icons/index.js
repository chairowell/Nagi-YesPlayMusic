import SvgIcon from '@/components/SvgIcon';
// vite-plugin-svg-icons 会把 src/assets/icons 下的 svg 打成一张 sprite，
// 导入这个虚拟模块即可注册，等价于原来的 require.context。
import 'virtual:svg-icons-register';

export default {
  install(app) {
    app.component('svg-icon', SvgIcon);
  },
};
