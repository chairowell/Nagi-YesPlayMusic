import { app, dialog, globalShortcut, ipcMain, screen, shell } from 'electron';
import { registerGlobalShortcut } from '@/electron/globalShortcut';
import { createUnblockMusicService } from '@/services/unblockMusic';
import cloneDeep from 'lodash/cloneDeep';
import shortcuts from '@/utils/shortcuts';
import { createMenu } from './menu';
import { isCreateTray, isMac } from '@/utils/platform';
import { hasReachableWindowArea } from '@/utils/windowGeometry';

const clc = require('cli-color');
const log = text => {
  console.log(`${clc.blueBright('[ipcMain.js]')} ${text}`);
};

const exitAsk = (e, win) => {
  e.preventDefault(); //阻止默认行为
  dialog
    .showMessageBox({
      type: 'info',
      title: 'Information',
      cancelId: 2,
      defaultId: 0,
      message: '确定要关闭吗？',
      buttons: ['最小化', '直接退出'],
    })
    .then(result => {
      if (result.response == 0) {
        e.preventDefault(); //阻止默认行为
        win.minimize(); //调用 最小化实例方法
      } else if (result.response == 1) {
        win = null;
        //app.quit();
        app.exit(); //exit()直接关闭客户端，不会执行quit();
      }
    })
    .catch(err => {
      log(err);
    });
};

const exitAskWithoutMac = (e, win) => {
  e.preventDefault(); //阻止默认行为
  dialog
    .showMessageBox({
      type: 'info',
      title: 'Information',
      cancelId: 2,
      defaultId: 0,
      message: '确定要关闭吗？',
      buttons: ['最小化到托盘', '直接退出'],
      checkboxLabel: '记住我的选择',
    })
    .then(result => {
      if (result.checkboxChecked && result.response !== 2) {
        win.webContents.send(
          'rememberCloseAppOption',
          result.response === 0 ? 'minimizeToTray' : 'exit'
        );
      }

      if (result.response === 0) {
        e.preventDefault(); //阻止默认行为
        win.hide(); //调用 最小化实例方法
      } else if (result.response === 1) {
        win = null;
        //app.quit();
        app.exit(); //exit()直接关闭客户端，不会执行quit();
      }
    })
    .catch(err => {
      log(err);
    });
};

const client = require('discord-rich-presence')('818936529484906596');

export function initIpcMain(win, store, trayEventEmitter) {
  // WIP: Do not enable logging as it has some issues in non-blocking I/O environment.
  // UNM.enableLogging(UNM.LoggingType.ConsoleEnv);
  const unblockMusic = createUnblockMusicService({ log });
  ipcMain.handle(
    'unblock-music',
    /**
     *
     * @param {*} _
     * @param {string | null} sourceListString
     * @param {Record<string, any>} ncmTrack
     * @param {UNM.Context} context
     */
    (_, sourceListString, ncmTrack, context) =>
      unblockMusic(sourceListString, ncmTrack, context)
  );

  ipcMain.handle('openExternalUrl', (_, value) => {
    let url;
    try {
      url = new URL(value);
    } catch {
      throw new Error('外链地址无效');
    }
    // 渲染进程会处理用户输入，主进程仍要再做一层协议校验，
    // 避免被注入的 IPC 调用借 shell.openExternal 打开本地文件或其他协议。
    if (!['http:', 'https:'].includes(url.protocol)) {
      throw new Error('只允许打开 HTTP(S) 外链');
    }
    return shell.openExternal(url.href);
  });

  ipcMain.on('close', e => {
    if (isMac) {
      win.hide();
      exitAsk(e, win);
    } else {
      let closeOpt = store.get('settings.closeAppOption');
      if (closeOpt === 'exit') {
        win = null;
        //app.quit();
        app.exit(); //exit()直接关闭客户端，不会执行quit();
      } else if (closeOpt === 'minimizeToTray') {
        e.preventDefault();
        win.hide();
      } else {
        exitAskWithoutMac(e, win);
      }
    }
  });

  ipcMain.on('minimize', () => {
    win.minimize();
  });

  ipcMain.on('maximizeOrUnmaximize', () => {
    win.isMaximized() ? win.unmaximize() : win.maximize();
  });

  // 迷你播放器的「钉在最上层」开关
  ipcMain.handle('toggleAlwaysOnTop', () => {
    const next = !win.isAlwaysOnTop();
    // 'floating' 这一层能盖住普通窗口，但不会挡住系统菜单栏
    win.setAlwaysOnTop(next, 'floating');
    // 切到别的桌面/全屏空间时也跟着走
    win.setVisibleOnAllWorkspaces(next, { visibleOnFullScreen: true });
    store.set('window.alwaysOnTop', next); // 记住，下次启动还原
    return next;
  });

  ipcMain.handle('isAlwaysOnTop', () => win.isAlwaysOnTop());

  const getCompactWindowSnapshot = () => {
    const [width, height] = win.getContentSize();
    const { x, y } = win.getBounds();
    return {
      frame: { x, y, width, height },
      maximized: win.isMaximized(),
      fullscreen: win.isFullScreen(),
    };
  };

  const waitForFullScreenExit = () =>
    new Promise(resolve => {
      if (!win.isFullScreen()) {
        resolve();
        return;
      }
      let timer;
      const finish = () => {
        clearTimeout(timer);
        win.removeListener('leave-full-screen', finish);
        resolve();
      };
      win.once('leave-full-screen', finish);
      timer = setTimeout(finish, 1500);
      win.setFullScreen(false);
    });

  const applyCompactWindowFrame = async frame => {
    const width = Math.round(Number(frame?.width));
    const height = Math.round(Number(frame?.height));
    if (
      !Number.isFinite(width) ||
      !Number.isFinite(height) ||
      width < 300 ||
      height < 48 ||
      width > 8192 ||
      height > 8192
    ) {
      return false;
    }
    await waitForFullScreenExit();
    if (win.isMaximized()) win.unmaximize();
    win.setContentSize(width, height, true);
    const bounds = win.getBounds();
    const x = frame?.x == null ? null : Number(frame.x);
    const y = frame?.y == null ? null : Number(frame.y);
    const target = { ...bounds, x, y };
    if (
      Number.isFinite(x) &&
      Number.isFinite(y) &&
      hasReachableWindowArea(target, screen.getAllDisplays())
    ) {
      win.setPosition(Math.round(x), Math.round(y), true);
    } else {
      win.center();
    }
    return true;
  };

  ipcMain.handle('getCompactWindowSnapshot', getCompactWindowSnapshot);

  ipcMain.handle('restoreRememberedCompactWindowFrame', (_, target) =>
    applyCompactWindowFrame(target)
  );

  ipcMain.handle('expandCompactWindow', (_, target) => {
    const [currentWidth, currentHeight] = win.getContentSize();
    if (
      !win.isMaximized() &&
      !win.isFullScreen() &&
      currentWidth >= 620 &&
      currentHeight >= 340
    ) {
      return false;
    }
    return applyCompactWindowFrame(target);
  });

  ipcMain.handle('restoreCompactWindow', (_, target) => {
    const [currentWidth, currentHeight] = win.getContentSize();
    if (
      !win.isMaximized() &&
      !win.isFullScreen() &&
      (currentWidth < 620 || currentHeight < 340)
    ) {
      return false;
    }
    // 目标来自持久化的 Bar 记忆，不能依赖“本次会话里曾展开过”；否则重启后无法收回。
    return applyCompactWindowFrame(target);
  });

  // 迷你模式下默认藏起红绿灯，鼠标悬浮时再显示（仅 macOS 支持）
  ipcMain.on('setWindowButtonVisibility', (e, visible) => {
    if (process.platform !== 'darwin') return;
    win.setWindowButtonVisibility(visible);
  });

  ipcMain.on('settings', (event, options) => {
    store.set('settings', options);
    if (options.enableGlobalShortcut) {
      registerGlobalShortcut(win, store);
    } else {
      log('unregister global shortcut');
      globalShortcut.unregisterAll();
    }
  });

  ipcMain.on('playDiscordPresence', (event, track) => {
    client.updatePresence({
      details: track.name + ' - ' + track.ar.map(ar => ar.name).join(','),
      state: track.al.name,
      endTimestamp: Date.now() + track.dt,
      largeImageKey: track.al.picUrl,
      largeImageText: 'Listening ' + track.name,
      smallImageKey: 'play',
      smallImageText: 'Playing',
      instance: true,
    });
  });

  ipcMain.on('pauseDiscordPresence', (event, track) => {
    client.updatePresence({
      details: track.name + ' - ' + track.ar.map(ar => ar.name).join(','),
      state: track.al.name,
      largeImageKey: track.al.picUrl,
      largeImageText: 'YesPlayMusic',
      smallImageKey: 'pause',
      smallImageText: 'Pause',
      instance: true,
    });
  });

  ipcMain.on('setProxy', (event, config) => {
    const proxyRules = `${config.protocol}://${config.server}:${config.port}`;
    store.set('proxy', proxyRules);
    win.webContents.session.setProxy(
      {
        proxyRules,
      },
      () => {
        log('finished setProxy');
      }
    );
  });

  ipcMain.on('removeProxy', (event, arg) => {
    log('removeProxy');
    win.webContents.session.setProxy({});
    store.set('proxy', '');
  });

  ipcMain.on('switchGlobalShortcutStatusTemporary', (e, status) => {
    log('switchGlobalShortcutStatusTemporary');
    if (status === 'disable') {
      globalShortcut.unregisterAll();
    } else {
      registerGlobalShortcut(win, store);
    }
  });

  ipcMain.on('updateShortcut', (e, { id, type, shortcut }) => {
    log('updateShortcut');
    let shortcuts = store.get('settings.shortcuts');
    let newShortcut = shortcuts.find(s => s.id === id);
    newShortcut[type] = shortcut;
    store.set('settings.shortcuts', shortcuts);

    createMenu(win, store);
    globalShortcut.unregisterAll();
    registerGlobalShortcut(win, store);
  });

  ipcMain.on('restoreDefaultShortcuts', () => {
    log('restoreDefaultShortcuts');
    store.set('settings.shortcuts', cloneDeep(shortcuts));

    createMenu(win, store);
    globalShortcut.unregisterAll();
    registerGlobalShortcut(win, store);
  });

  if (isCreateTray) {
    ipcMain.on('updateTrayTooltip', (_, title) => {
      trayEventEmitter.emit('updateTooltip', title);
    });
    ipcMain.on('updateTrayPlayState', (_, isPlaying) => {
      trayEventEmitter.emit('updatePlayState', isPlaying);
    });
    ipcMain.on('updateTrayLikeState', (_, isLiked) => {
      trayEventEmitter.emit('updateLikeState', isLiked);
    });
    ipcMain.on('updateTrayIcon', () => {
      trayEventEmitter.emit('updateIcon');
    });
    // macOS 菜单栏：封面 + 歌名/歌词
    ipcMain.on('updateTrayNowPlaying', (_, payload) => {
      trayEventEmitter.emit('updateNowPlaying', payload);
    });
  }
}
