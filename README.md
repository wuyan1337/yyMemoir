# yyMemoir

装上就不用管的 VRChat 回顾工具。开机后在托盘里安静记录，每次下线自动推送一张「本次回顾」，日报、周报、月报和年度报告随时可以看、可以存成图片分享。

所有数据只保存在你自己的电脑上。

## 功能

- **自动记录**：读取 VRChat 自己写在本机的日志，记下你去过的世界、房间类型、遇到的人、待了多久、拍的照片。
- **下线推送**：每次关掉 VRChat，弹出一条 Windows 通知，点开就是这次游玩的回顾卡片。
- **日报 / 周报 / 月报**：在程序里随时翻看，可以切换版式（小票 / 格子 / 票根），一键复制或保存为图片。
- **年度报告**：10 页故事页，最后一页是「年度小票」，适合分享。
- **世界封面**：自动从 VRChat 取世界名和封面，卡片颜色跟着封面走。
- **隐私开关**：导出图片前可以隐藏其他玩家的名字。
- **备份**：导出一个 `.yymb` 文件（含记录、照片、封面），换电脑时导入，可选合并或覆盖。

## 安装

1. 在 [Releases](../../releases) 下载 `yyMemoir_x.x.x_x64-setup.exe`。还没有正式发布时，可以在 [Actions](../../actions) 里最新一次成功的构建下载 `yyMemoir-windows`（需要登录 GitHub）。
2. 双击安装。安装包没有数字签名，Windows 可能提示「已保护你的电脑」，点「更多信息」→「仍要运行」。
3. 安装到当前用户目录，不需要管理员权限。第一次打开后会自动开启开机启动，可以在设置里关掉。

## 使用前必看

**把 VRChat 的日志设置成「完整」**：游戏内 设置 → Debug → 日志（Logging）→ 完整（Full）。

如果是「仅错误（Errors Only）」，游戏不会写下你去了哪些世界、和谁在一起，yyMemoir 就什么也记不到（程序检测到时会提醒）。Quest 版新装默认就是「仅错误」。

## 支持范围

- Windows 10 / 11 上的 VRChat PC 版（Steam 版、Meta 商店 PC 版，桌面模式和 VR 模式都可以）。
- 不支持 Quest 一体机和手机版：它们的日志不在电脑上。
- 照片只认 VRChat 相机拍的（日志里有记录的那些）。「多层相机」模式拍的照片不会写日志，认不到。
- 只能记录装上之后的游玩：VRChat 会自动删除超过 24 小时的旧日志。

## 隐私

- 记录全部保存在本机 `%APPDATA%\com.yymemoir.app\`，没有账号、没有服务器、不上传任何东西。
- 唯一的联网是按世界 ID 向 VRChat 官方接口查询世界名和封面（不需要登录），查过的会缓存，不重复请求。
- 只读取日志和截图，不修改游戏、不注入、不抓包。

## 从源码构建

需要 Rust（stable）、Node.js 22，Windows 上还需要 Visual Studio 生成工具（「使用 C++ 的桌面开发」）。

```bash
git clone https://github.com/wuyan1337/yyMemoir
cd yyMemoir/app
npm ci
npx tauri dev      # 开发运行
npx tauri build    # 打包，安装包在 target/release/bundle/nsis/
```

核心库的测试可以在任何平台跑：

```bash
cargo test
```

## 目录

| 路径 | 内容 |
| --- | --- |
| `crates/core` | 核心：日志解析、SQLite 存储、增量读取、统计、世界封面、备份 |
| `crates/cli` | 命令行工具（`yymemoir-cli`）：读日志、出报告、导出导入备份，用来调试和校对 |
| `app/src` | 界面（React + TypeScript）：报告卡片、年度故事页、主窗口 |
| `app/src-tauri` | Windows 程序（Tauri 2）：托盘、后台监听、通知、开机启动 |

## 致谢

日志格式的识别参考了这些开源项目：[VRCX](https://github.com/vrcx-team/VRCX)、[VRCTimeline](https://github.com/tsugu233/VRCTimeline)、[vrchat-log-rs](https://github.com/sksat/vrchat-log-rs)。年度故事页的页序参考了 [github-wrapped](https://github.com/PhumudzoSly/github-wrapped)。

yyMemoir 是非官方工具，和 VRChat Inc. 没有关联。
