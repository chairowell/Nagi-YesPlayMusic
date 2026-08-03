/* global __static */
import path from 'path';
import { app, nativeImage, Tray, Menu, nativeTheme, net } from 'electron';
import { isLinux, isMac } from '@/utils/platform';

function createMenuTemplate(win) {
  return [
    {
      label: '播放',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/play.png')
      ),
      click: () => {
        win.webContents.send('play');
      },
      id: 'play',
    },
    {
      label: '暂停',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/pause.png')
      ),
      click: () => {
        win.webContents.send('play');
      },
      id: 'pause',
      visible: false,
    },
    {
      label: '上一首',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/left.png')
      ),
      accelerator: 'CmdOrCtrl+Left',
      click: () => {
        win.webContents.send('previous');
      },
    },
    {
      label: '下一首',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/right.png')
      ),
      accelerator: 'CmdOrCtrl+Right',
      click: () => {
        win.webContents.send('next');
      },
    },
    {
      label: '循环播放',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/repeat.png')
      ),
      accelerator: 'Alt+R',
      click: () => {
        win.webContents.send('repeat');
      },
    },
    {
      label: '加入喜欢',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/like.png')
      ),
      accelerator: 'CmdOrCtrl+L',
      click: () => {
        win.webContents.send('like');
      },
      id: 'like',
    },
    {
      label: '取消喜欢',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/unlike.png')
      ),
      accelerator: 'CmdOrCtrl+L',
      click: () => {
        win.webContents.send('like');
      },
      id: 'unlike',
      visible: false,
    },
    {
      label: '退出',
      icon: nativeImage.createFromPath(
        path.join(__static, 'img/icons/exit.png')
      ),
      accelerator: 'CmdOrCtrl+W',
      click: () => {
        app.exit();
      },
    },
  ];
}

// linux下托盘的实现方式比较迷惑
// right-click无法在linux下使用
// click在默认行为下会弹出一个contextMenu，里面的唯一选项才会调用click事件
// setContextMenu应该是目前唯一能在linux下使用托盘菜单api
// 但是无法区分鼠标左右键

// 发现openSUSE KDE环境可以区分鼠标左右键
// 添加左键支持
// 2022.05.17
class YPMTrayLinuxImpl {
  constructor(tray, win, emitter, store) {
    this.tray = tray;
    this.win = win;
    this.emitter = emitter;
    this.store = store;
    this.template = undefined;
    this.initTemplate();
    this.contextMenu = Menu.buildFromTemplate(this.template);

    this.tray.setContextMenu(this.contextMenu);
    this.handleEvents();
  }

  initTemplate() {
    //在linux下，鼠标左右键都会呼出contextMenu
    //所以此处单独为linux添加一个 显示主面板 选项
    this.template = [
      {
        label: '显示主面板',
        click: () => {
          this.win.show();
        },
      },
      {
        type: 'separator',
      },
    ].concat(createMenuTemplate(this.win));
  }

  handleEvents() {
    this.tray.on('click', () => {
      this.win.show();
    });

    this.emitter.on('updateTooltip', title => this.tray.setToolTip(title));
    this.emitter.on('updatePlayState', isPlaying => {
      this.contextMenu.getMenuItemById('play').visible = !isPlaying;
      this.contextMenu.getMenuItemById('pause').visible = isPlaying;
      this.tray.setContextMenu(this.contextMenu);
    });
    this.emitter.on('updateLikeState', isLiked => {
      this.contextMenu.getMenuItemById('like').visible = !isLiked;
      this.contextMenu.getMenuItemById('unlike').visible = isLiked;
      this.tray.setContextMenu(this.contextMenu);
    });
    this.emitter.on('updateIcon', () => {
      this.updateIcon();
    });
  }

  updateIcon() {
    let trayIconSetting = this.store.get('settings.trayIconTheme') || 'auto';
    let iconTheme;
    if (trayIconSetting === 'auto') {
      iconTheme = nativeTheme.shouldUseDarkColors ? 'light' : 'dark';
    } else {
      iconTheme = trayIconSetting;
    }

    let icon = nativeImage
      .createFromPath(path.join(__static, `img/icons/menu-${iconTheme}@88.png`))
      .resize({
        height: 20,
        width: 20,
      });

    this.tray.setImage(icon);
  }
}

class YPMTrayWindowsImpl {
  constructor(tray, win, emitter, store) {
    this.tray = tray;
    this.win = win;
    this.emitter = emitter;
    this.store = store;
    this.template = createMenuTemplate(win);
    this.contextMenu = Menu.buildFromTemplate(this.template);

    this.isPlaying = false;
    this.curDisplayPlaying = false;

    this.isLiked = false;
    this.curDisplayLiked = false;

    this.handleEvents();
  }

  handleEvents() {
    this.tray.on('click', () => {
      this.win.show();
    });

    this.tray.on('right-click', () => {
      if (this.isPlaying !== this.curDisplayPlaying) {
        this.curDisplayPlaying = this.isPlaying;
        this.contextMenu.getMenuItemById('play').visible = !this.isPlaying;
        this.contextMenu.getMenuItemById('pause').visible = this.isPlaying;
      }

      if (this.isLiked !== this.curDisplayLiked) {
        this.curDisplayLiked = this.isLiked;
        this.contextMenu.getMenuItemById('like').visible = !this.isLiked;
        this.contextMenu.getMenuItemById('unlike').visible = this.isLiked;
      }

      this.tray.popUpContextMenu(this.contextMenu);
    });

    this.emitter.on('updateTooltip', title => this.tray.setToolTip(title));
    this.emitter.on(
      'updatePlayState',
      isPlaying => (this.isPlaying = isPlaying)
    );
    this.emitter.on('updateLikeState', isLiked => (this.isLiked = isLiked));
    this.emitter.on('updateIcon', () => {
      this.updateIcon();
    });
  }

  updateIcon() {
    let trayIconSetting = this.store.get('settings.trayIconTheme') || 'auto';
    let iconTheme;
    if (trayIconSetting === 'auto') {
      iconTheme = nativeTheme.shouldUseDarkColors ? 'light' : 'dark';
    } else {
      iconTheme = trayIconSetting;
    }

    let icon = nativeImage
      .createFromPath(path.join(__static, `img/icons/menu-${iconTheme}@88.png`))
      .resize({
        height: 20,
        width: 20,
      });

    this.tray.setImage(icon);
  }
}

// 全角字符（中日韩、全角标点）算 2 个字宽，其余算 1 个
function charWidth(ch) {
  return /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹯＀-｠￠-￦]/.test(
    ch
  )
    ? 2
    : 1;
}

function truncateByWidth(text, maxWidth) {
  let width = 0;
  for (let i = 0; i < text.length; i++) {
    width += charWidth(text[i]);
    if (width > maxWidth) return `${text.slice(0, i)}…`;
  }
  return text;
}

// macOS 菜单栏：图标位置放专辑封面，右边跟一行歌名或歌词
class YPMTrayMacImpl {
  constructor(tray, win, emitter, store) {
    this.tray = tray;
    this.win = win;
    this.emitter = emitter;
    this.store = store;
    this.template = createMenuTemplate(win);
    this.contextMenu = Menu.buildFromTemplate(this.template);

    this.isPlaying = false;
    this.curDisplayPlaying = false;
    this.isLiked = false;
    this.curDisplayLiked = false;

    this.lastCoverUrl = null; // 同一张封面不重复下载

    this.handleEvents();
  }

  handleEvents() {
    this.tray.on('click', () => {
      this.win.isVisible() && this.win.isFocused()
        ? this.win.hide()
        : this.win.show();
    });

    this.tray.on('right-click', () => {
      if (this.isPlaying !== this.curDisplayPlaying) {
        this.curDisplayPlaying = this.isPlaying;
        this.contextMenu.getMenuItemById('play').visible = !this.isPlaying;
        this.contextMenu.getMenuItemById('pause').visible = this.isPlaying;
      }
      if (this.isLiked !== this.curDisplayLiked) {
        this.curDisplayLiked = this.isLiked;
        this.contextMenu.getMenuItemById('like').visible = !this.isLiked;
        this.contextMenu.getMenuItemById('unlike').visible = this.isLiked;
      }
      this.tray.popUpContextMenu(this.contextMenu);
    });

    this.emitter.on('updateTooltip', title => this.tray.setToolTip(title));
    this.emitter.on(
      'updatePlayState',
      isPlaying => (this.isPlaying = isPlaying)
    );
    this.emitter.on('updateLikeState', isLiked => (this.isLiked = isLiked));
    this.emitter.on('updateIcon', () => {});
    this.emitter.on('updateNowPlaying', payload => {
      this.updateNowPlaying(payload);
    });

    // 窗口显示/隐藏都要重算菜单栏该不该显示文字
    this.win.on('show', () => this.renderTitle());
    this.win.on('hide', () => this.renderTitle());
  }

  // 播放窗口可见时菜单栏只留封面，避免同一首歌在上下两处重复显示；
  // 窗口一旦被隐藏（比如点菜单栏图标收起），文字要自动补回来。
  renderTitle() {
    const { title = '' } = this.last || {};
    const windowVisible = this.win.isVisible();
    // 菜单栏空间有限，但不能按字符数一刀切：
    // 中日韩和全角标点占两个字宽，英文只占一个，
    // 按字符截会让英文歌词白白少显示一半。这里按显示宽度算。
    const maxWidth = this.store.get('settings.trayTitleMaxWidth') || 44;
    this.tray.setTitle(
      windowVisible ? '' : truncateByWidth(title.trim(), maxWidth)
    );
  }

  async updateNowPlaying(payload = {}) {
    this.last = payload;
    this.renderTitle();
    const { coverUrl } = payload;

    if (!coverUrl || coverUrl === this.lastCoverUrl) return;
    this.lastCoverUrl = coverUrl;
    try {
      const res = await net.fetch(coverUrl);
      const buf = Buffer.from(await res.arrayBuffer());
      const icon = nativeImage
        .createFromBuffer(buf)
        .resize({ width: 18, height: 18 });
      this.tray.setImage(icon);
    } catch (e) {
      // 封面拉不下来就保持原图标，不影响文字
      console.error('[tray] failed to load cover:', e.message);
    }
  }
}

export function createTray(win, eventEmitter, store) {
  let trayIconSetting = store.get('settings.trayIconTheme') || 'auto';
  let iconTheme;
  if (trayIconSetting === 'auto') {
    iconTheme = nativeTheme.shouldUseDarkColors ? 'light' : 'dark';
  } else {
    iconTheme = trayIconSetting;
  }

  let icon = nativeImage
    .createFromPath(path.join(__static, `img/icons/menu-${iconTheme}@88.png`))
    .resize({
      height: 20,
      width: 20,
    });

  let tray = new Tray(icon);
  tray.setToolTip('YesPlayMusic');

  if (isLinux) return new YPMTrayLinuxImpl(tray, win, eventEmitter, store);
  if (isMac) return new YPMTrayMacImpl(tray, win, eventEmitter, store);
  return new YPMTrayWindowsImpl(tray, win, eventEmitter, store);
}
