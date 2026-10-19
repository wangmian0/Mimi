<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="mimi">
  <h1>Mimi</h1>
  <p>把电脑或麦克风声音转成实时翻译字幕。</p>
  <p>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/github/v/release/yuxino/mimi?style=flat&amp;logo=github&amp;logoColor=white" alt="最新版本"></a>
    <a href="https://github.com/yuxino/mimi/releases"><img src="https://img.shields.io/github/downloads/yuxino/mimi/total?style=flat&amp;labelColor=a85f82&amp;color=e889b5" alt="总下载量"></a>
    <a href="https://github.com/yuxino/mimi/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://img.shields.io/github/actions/workflow/status/yuxino/mimi/ci.yml?style=flat&amp;logo=githubactions&amp;logoColor=white&amp;branch=main&amp;event=push&amp;label=CI" alt="main 分支 CI 状态"></a>
    <a href="LICENSE"><img src="https://img.shields.io/github/license/yuxino/mimi?style=flat&amp;logo=opensourceinitiative&amp;logoColor=white" alt="MIT 许可证"></a>
  </p>
  <p>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/badge/macOS-13%2B-555?style=flat&amp;logo=apple&amp;logoColor=white" alt="macOS 13+"></a>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/badge/Windows-x64-0078D4?style=flat&amp;logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0id2hpdGUiIGQ9Ik0wIDBoMTF2MTFIMHptMTMgMGgxMXYxMUgxM3pNMCAxM2gxMXYxMUgwem0xMyAwaDExdjExSDEzeiIvPjwvc3ZnPg%3D%3D&amp;logoColor=white" alt="Windows x64"></a>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/badge/Linux-x86__64-FCC624?style=flat&amp;logo=linux&amp;logoColor=white" alt="Linux"></a>
    <a href="android/README.zh-CN.md"><img src="https://img.shields.io/badge/Android-app-3DDC84?style=flat&amp;logo=android&amp;logoColor=white" alt="Android"></a>
  </p>
  <p>
    <a href="README_EN.md"><strong>English</strong></a> · <a href="docs/readme/zh-CN.md">简体中文</a> · <a href="docs/readme/zh-TW.md">繁體中文</a> · <a href="docs/readme/ja.md">日本語</a> · <a href="docs/readme/th.md">ภาษาไทย</a> · <a href="docs/readme/ko.md">한국어</a> · <a href="docs/readme/fr.md">Français</a> · <a href="docs/readme/de.md">Deutsch</a>
  </p>
</div>

> [!NOTE]
> **这是 Mimi 的个人分支**，由 [@wangmian0](https://github.com/wangmian0) 维护，基于上游 v1.6.3，新增「AI 提词」：用免提打电话时，Mimi 在本机靠声纹分出你和对方，对方说完一句，字幕窗就给出一句可以接着说的英文和中文意思。这个功能仅支持 Apple 芯片和 macOS 26，可以在 [Releases](https://github.com/wangmian0/Mimi/releases/latest) 下载安装包「Mimi Copilot」，它能和官方版同时安装。改动和用法见 [FORK.md](FORK.md)。下文的下载和反馈链接都指向官方 [yuxino/Mimi](https://github.com/yuxino/Mimi)，官方版本不包含这些改动。
>
> **[→ AI 提词分支：改动与使用说明](https://github.com/wangmian0/Mimi/blob/fork-main/FORK.md)**

![Mimi 双语字幕示意：原创插画与示例对白](docs/assets/readme-preview.png)

字幕显示示意，使用示例对白。[看字幕与沉浸模式演示（旧版界面）](docs/demos/README.md)。

## 实时字幕与翻译

- **悬浮字幕**：为电影、直播、网课实时显示字幕，支持双语、仅译文或仅原文。
- **选择音源**：采集电脑声音、麦克风，或同时采集两路。macOS / Windows 11 支持指定应用，Linux 支持选择单独的播放流。
- **调整显示**：自由调整字幕位置、字号和颜色；沉浸模式只显示文字，点击穿透可让你继续操作下方应用。
- **保存字幕和录音**：桌面端可将字幕保存为 TXT，也可将所选音源录制为 WAV。两项功能均需手动开启。

## 开始使用

[下载桌面版](https://github.com/yuxino/mimi/releases/latest) · [Android 安装与使用](android/README.zh-CN.md#start)

桌面版支持 macOS 13+（Apple 芯片和 Intel）、Windows x64、Linux x86_64。

1. **配置服务**：在「设置 → 模型与服务」中添加语音服务。服务选择和密钥获取方法见[配置指南](docs/provider-setup.zh-CN.md#choose)。
2. **选择声音和语言**：在「声音与语言」中选择要采集的音源、识别语言和翻译语言。
3. **开启字幕**：在「字幕」页打开「实时字幕」，按提示授予所需权限，再播放想看的内容。

Mimi 本身免费，云服务可能收费。使用云端识别或翻译时，音频或识别文字会发送给相应服务。Apple 本地服务的适用条件见[配置指南](docs/provider-setup.zh-CN.md#apple-speech)。

## 使用中遇到问题？

[使用指南](docs/usage.zh-CN.md)包含安装、权限、字幕显示和保存记录。没有字幕或连接失败，先看[识别错误](docs/usage.zh-CN.md#errors)；仍未解决可[反馈问题](https://github.com/yuxino/mimi/issues)。

## 贡献者

想帮 Mimi 改进？从[贡献指南（中英）](.github/CONTRIBUTING.md)开始。

感谢每一位写代码、提问题、试用和分享的朋友 (๑•̀ㅂ•́)و✧

特别感谢 [@yebuwudong](https://github.com/yebuwudong) 贡献 [Android 版](https://github.com/yuxino/mimi/pull/37)，以及 [@LLLin000](https://github.com/LLLin000) 贡献[字幕动效](https://github.com/yuxino/mimi/pull/67)和 [Windows 音源改进](https://github.com/yuxino/mimi/pull/89)。

也感谢 [@Chtholly000](https://github.com/Chtholly000) 改进 [Gemini 连续字幕和连接轮换](https://github.com/yuxino/Mimi/pull/176)。

感谢 [@EmmetZ](https://github.com/EmmetZ) 修复 [Wayland 浮窗与控制组件不同步的问题](https://github.com/yuxino/Mimi/pull/242)，并新增 [Linux 单独播放流采集](https://github.com/yuxino/Mimi/pull/275)。

感谢 [@wangmian0](https://github.com/wangmian0) 为 [DeepSeek 实时字幕翻译关闭思考模式](https://github.com/yuxino/Mimi/pull/270)。

<p>
  <a href="https://github.com/yuxino"><img src="docs/assets/contributors/yuxino.svg" width="64" height="64" alt="@yuxino"></a>
  <a href="https://github.com/LLLin000"><img src="docs/assets/contributors/LLLin000.svg" width="64" height="64" alt="@LLLin000"></a>
  <a href="https://github.com/yebuwudong"><img src="docs/assets/contributors/yebuwudong.svg" width="64" height="64" alt="@yebuwudong"></a>
  <a href="https://github.com/Chtholly000"><img src="docs/assets/contributors/Chtholly000.svg" width="64" height="64" alt="@Chtholly000"></a>
  <a href="https://github.com/inhome"><img src="docs/assets/contributors/inhome.svg" width="64" height="64" alt="@inhome"></a>
  <a href="https://github.com/EmmetZ"><img src="docs/assets/contributors/EmmetZ.svg" width="64" height="64" alt="@EmmetZ"></a>
  <a href="https://github.com/wangmian0"><img src="docs/assets/contributors/wangmian0.svg" width="64" height="64" alt="@wangmian0"></a>
</p>

[查看所有贡献者](https://github.com/yuxino/mimi/graphs/contributors)

## 社区致谢

也感谢 [V2EX](https://www.v2ex.com/)、[LINUX DO](https://linux.do/)、[小众软件](https://meta.appinn.net/)、[NodeLoc](https://www.nodeloc.com/)、[Solo](https://solo.xin/)、[新趣集](https://xinquji.com/posts/859305)和[电鸭](https://eleduck.com/)社区朋友的试用、反馈与分享。

[MIT](LICENSE) © 2026 yuxino
