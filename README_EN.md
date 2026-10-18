<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="mimi">
  <h1>Mimi</h1>
  <p>Live translated subtitles from computer or microphone audio.</p>
  <p>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/github/v/release/yuxino/mimi?style=flat&amp;logo=github&amp;logoColor=white" alt="Latest release"></a>
    <a href="https://github.com/yuxino/mimi/releases"><img src="https://img.shields.io/github/downloads/yuxino/mimi/total?style=flat&amp;labelColor=a85f82&amp;color=e889b5" alt="Total downloads"></a>
    <a href="https://github.com/yuxino/mimi/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://img.shields.io/github/actions/workflow/status/yuxino/mimi/ci.yml?style=flat&amp;logo=githubactions&amp;logoColor=white&amp;branch=main&amp;event=push&amp;label=CI" alt="CI status on main"></a>
    <a href="LICENSE"><img src="https://img.shields.io/github/license/yuxino/mimi?style=flat&amp;logo=opensourceinitiative&amp;logoColor=white" alt="MIT license"></a>
  </p>
  <p>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/badge/macOS-13%2B-555?style=flat&amp;logo=apple&amp;logoColor=white" alt="macOS 13+"></a>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/badge/Windows-x64-0078D4?style=flat&amp;logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0id2hpdGUiIGQ9Ik0wIDBoMTF2MTFIMHptMTMgMGgxMXYxMUgxM3pNMCAxM2gxMXYxMUgwem0xMyAwaDExdjExSDEzeiIvPjwvc3ZnPg%3D%3D&amp;logoColor=white" alt="Windows x64"></a>
    <a href="https://github.com/yuxino/mimi/releases/latest"><img src="https://img.shields.io/badge/Linux-x86__64-FCC624?style=flat&amp;logo=linux&amp;logoColor=white" alt="Linux"></a>
    <a href="android/README.md"><img src="https://img.shields.io/badge/Android-app-3DDC84?style=flat&amp;logo=android&amp;logoColor=white" alt="Android"></a>
  </p>
  <p>
    <a href="README_EN.md"><strong>English</strong></a> · <a href="docs/readme/zh-CN.md">简体中文</a> · <a href="docs/readme/zh-TW.md">繁體中文</a> · <a href="docs/readme/ja.md">日本語</a> · <a href="docs/readme/th.md">ภาษาไทย</a> · <a href="docs/readme/ko.md">한국어</a> · <a href="docs/readme/fr.md">Français</a> · <a href="docs/readme/de.md">Deutsch</a>
  </p>
</div>

> [!NOTE]
> **This is a personal fork of Mimi** maintained by [@wangmian0](https://github.com/wangmian0), based on upstream v1.6.3. It adds an "AI Copilot" for speakerphone calls: Mimi tells you and the other speaker apart with an on-device voiceprint, and after each sentence from the other speaker it suggests one English reply with a Chinese gloss. It requires Apple silicon and macOS 26 and must be built from source. See [FORK.md](FORK.md) (Simplified Chinese) for the changes and setup. The download and feedback links below point to the official [yuxino/Mimi](https://github.com/yuxino/Mimi), which does not include these changes.

![Mimi bilingual subtitle illustration with fictional dialogue](docs/assets/readme-preview.png)

Interface illustration with sample dialogue. [Watch subtitles and Immersive Mode (older UI)](docs/demos/README.md).

## Live subtitles and translation

- **Floating subtitles** for films, streams, and classes. Show both languages, translations only, or originals only.
- **Choose your audio**: computer audio, microphone, or both. Select an app on macOS / Windows 11, or an individual playback stream on Linux.
- **Adjust the display**: change position, font size, and color. Immersive Mode shows only text; click-through lets you use the app underneath.
- **Save subtitles and audio**: on desktop, save subtitles as TXT or record selected audio as WAV. Both options are off until you enable them.

## Get started

[Download for desktop](https://github.com/yuxino/mimi/releases/latest) · [Android setup](android/README.md#start)

Desktop supports macOS 13+ (Apple silicon and Intel), Windows x64, and Linux x86_64.

1. **Set up a service** in Settings → Models & Services. The [setup guide](docs/provider-setup.md#choose) explains how to choose a service and get its credentials.
2. **Choose audio and languages** in Audio & Languages: select your audio source, spoken language, and translation language.
3. **Start subtitles** on the Subtitles page. Turn on Live Subtitles, grant the requested permissions, and play your content.

Mimi is free; cloud services may charge. Cloud recognition or translation sends audio or recognized text to the selected service. See the [setup guide](docs/provider-setup.md#apple-speech) for Apple local service requirements.

## Need help?

The [usage guide](docs/usage.md) covers installation, permissions, subtitle display, and saved sessions. For missing subtitles or connection failures, start with [recognition errors](docs/usage.md#errors). If that does not help, [report an issue](https://github.com/yuxino/mimi/issues).

## Contributors

Want to help improve Mimi? Start with the [contribution guide](.github/CONTRIBUTING.md).

Thanks to everyone who writes code, reports issues, tries Mimi, or shares it (๑•̀ㅂ•́)و✧

Special thanks to [@yebuwudong](https://github.com/yebuwudong) for the [Android app](https://github.com/yuxino/mimi/pull/37), and [@LLLin000](https://github.com/LLLin000) for [subtitle animation](https://github.com/yuxino/mimi/pull/67) and [Windows audio improvements](https://github.com/yuxino/mimi/pull/89).

Thanks also to [@Chtholly000](https://github.com/Chtholly000) for improving [Gemini's continuous captions and planned connection rotation](https://github.com/yuxino/Mimi/pull/176).

Thanks to [@EmmetZ](https://github.com/EmmetZ) for fixing [Wayland overlay controls](https://github.com/yuxino/Mimi/pull/242) and adding [individual playback stream capture on Linux](https://github.com/yuxino/Mimi/pull/275).

Thanks to [@wangmian0](https://github.com/wangmian0) for [disabling thinking mode in DeepSeek live subtitle translation](https://github.com/yuxino/Mimi/pull/270).

<p>
  <a href="https://github.com/yuxino"><img src="docs/assets/contributors/yuxino.svg" width="64" height="64" alt="@yuxino"></a>
  <a href="https://github.com/LLLin000"><img src="docs/assets/contributors/LLLin000.svg" width="64" height="64" alt="@LLLin000"></a>
  <a href="https://github.com/yebuwudong"><img src="docs/assets/contributors/yebuwudong.svg" width="64" height="64" alt="@yebuwudong"></a>
  <a href="https://github.com/Chtholly000"><img src="docs/assets/contributors/Chtholly000.svg" width="64" height="64" alt="@Chtholly000"></a>
  <a href="https://github.com/inhome"><img src="docs/assets/contributors/inhome.svg" width="64" height="64" alt="@inhome"></a>
  <a href="https://github.com/EmmetZ"><img src="docs/assets/contributors/EmmetZ.svg" width="64" height="64" alt="@EmmetZ"></a>
  <a href="https://github.com/wangmian0"><img src="docs/assets/contributors/wangmian0.svg" width="64" height="64" alt="@wangmian0"></a>
</p>

[All contributors](https://github.com/yuxino/mimi/graphs/contributors)

## Community

Thanks to the people in [V2EX](https://www.v2ex.com/), [LINUX DO](https://linux.do/), [Appinn](https://meta.appinn.net/), [NodeLoc](https://www.nodeloc.com/), [Solo](https://solo.xin/), [Xinquji](https://xinquji.com/posts/859305), and [Eleduck](https://eleduck.com/) for trying Mimi, sharing feedback, and spreading the word.

[MIT](LICENSE) © 2026 yuxino
