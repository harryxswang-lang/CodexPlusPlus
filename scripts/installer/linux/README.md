# Ubuntu 本地版

首版目标是 Ubuntu 24.04 amd64：安装 Codex++ 管理界面，配置第三方供应商和模型，从界面启动已安装的 Codex / ChatGPT 桌面应用。桌面应用单独安装，不包含在本项目的 .deb 中。

## 准备构建环境

需要已有 Node.js、npm、Rust/Cargo 和桌面会话。Rust 使用仓库现有工具链，不由脚本安装或升级。

系统依赖请在终端手动安装：

```bash
sudo apt-get install --no-install-recommends build-essential pkg-config libssl-dev libgtk-3-dev libayatana-appindicator3-dev libwebkit2gtk-4.1-dev
```

在仓库根目录准备项目已有依赖：

```bash
(cd apps/codex-plus-manager && npm ci --ignore-scripts)
cargo fetch --locked
bash scripts/installer/linux/build-deb.sh --check
```

若下载失败，先检查本机代理和网络。脚本不会自动安装依赖、执行 sudo、更新工具链或清理旧产物。

本地语音依赖 sherpa-onnx 会在首次 Rust 编译时另外下载原生库，`cargo fetch` 不会下载这个文件。受限网络环境可在本机终端预先缓存（Ubuntu amd64）：

```bash
mkdir -p target/sherpa-onnx-prebuilt
curl -fL --retry 3 'https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-linux-x64-static-lib.tar.bz2' -o target/sherpa-onnx-prebuilt/sherpa-onnx-v1.13.8-linux-x64-static-lib.tar.bz2
```

该版本与当前 Cargo.lock 中的 sherpa-onnx 版本对应；更新该依赖时需要同步检查缓存文件名。

## 构建和安装

```bash
bash scripts/installer/linux/build-deb.sh
```

默认版本来自前端 package.json。产物位于 `dist/linux/codex-plus-plus_<版本>_<架构>.deb`。可传入 `1.9.0+ubuntu.1` 等版本区分本地构建；已有同名包时脚本拒绝覆盖。

编译后的前端嵌入二进制；打包脚本通过 dpkg-shlibdeps 生成实际共享库依赖，并添加托盘与系统链接处理的运行依赖。只有打包可用 `--package-only`，此选项要求已完成当前代码的前端和 release 构建。

把构建输出中的实际安装包路径交给 apt，例如：

```bash
sudo apt install ./dist/linux/codex-plus-plus_1.9.0_amd64.deb
```

随后在应用菜单打开 **Codex++**，或执行 `codex-plus-plus`。更新时手动安装新 .deb。Ubuntu 本地版不从上游 Windows/macOS 发行渠道自动更新。

## 使用第三方模型

1. 先从桌面版菜单退出已有 Codex / ChatGPT，然后打开 Codex++。
2. 确认「Codex 应用路径」。标准 deb 安装会自动检测 `/usr/lib/chatgpt`；也可手动选择应用目录。
3. 在供应商设置中添加「纯 API」供应商，填写服务商提供的 Base URL、API Key、模型名和协议。不要把桌面版替换成 PATH 中的 Codex CLI。
4. 选中供应商，保存并应用配置，再从 Codex++ 界面启动 Codex。
5. 新建对话，验证文本回复与工具调用。切换供应商后按界面提示重启桌面版。模型上下文窗口可沿用本项目的 `model-name[128K]` 等语法。

Responses 与 Chat Completions 的配置和协议转换沿用项目已有实现。模型必须匹配服务商实际名称；完整编程能力还取决于工具调用、流式输出等兼容性。不要把“配置成功”当作真实 API 调用成功。

配置应用会写入当前 Codex 配置；验证测试使用临时目录，不使用真实凭据。日常运行沿用现有配置备份机制。

## 首版范围与验证

- Linux 应用菜单入口和修复入口；识别系统 .deb 安装的入口。
- 从 Linux 包元数据读取桌面版本；Linux 下使用 xdg-open 打开链接。
- 识别当前用户的桌面主进程，排除 CLI 和 Electron 子进程；按调试端口停止指定实例。
- 已经运行的桌面版需要先手动退出，再由 Codex++ 带调试参数启动。
- 开机启动、自动接管、Claude 增强和其他平台专属能力不在首版范围内。

构建前的相关回归检查：

```bash
cargo test --locked -p codex-plus-core --lib
cargo test --locked -p codex-plus-core --test launcher --test installers --test relay_config --test protocol_proxy
```

构建后应在桌面会话启动二进制，确认窗口、进程和 `manager.start` 日志。最终还需使用你自己的供应商在本机验证真实对话和工具调用；API Key 只在应用内填写。
