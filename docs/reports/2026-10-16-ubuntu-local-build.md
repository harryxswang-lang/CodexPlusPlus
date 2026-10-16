# Ubuntu 首版本地构建记录

目标环境：Ubuntu 24.04 amd64。已安装的桌面宿主为 `/usr/lib/chatgpt/ChatGPT`，包版本为 `26.1007.71326`。

## 改动

- Linux 使用 XDG 应用菜单入口，支持修复用户入口并识别系统安装入口；Exec 正确转义带空格、百分号和引号的路径。
- 从 `resources/linux-package-metadata.json` 读取桌面版本，损坏时回退已有探测方式。
- 使用 `/proc` 识别当前用户的 Electron 桌面主进程，排除 CLI 和 renderer；按调试端口匹配停止目标，并复查可执行文件和进程启动标识。
- 已运行的桌面宿主需要先手动退出，再由 Codex++ 带调试端口启动。未支持的自动接管平台保守跳过。
- Linux 使用 `xdg-open`；本地版通过 .deb 手动更新，不使用上游 Windows/macOS 自动更新。
- 重写 .deb 脚本：不安装依赖、不提权、不清理历史产物；前端嵌入二进制，运行依赖由 dpkg-shlibdeps 计算。
- 第三方供应商、模型配置和协议转换沿用项目实现，增加供应商切换时模型、地址、凭据和 catalog 同步更新的临时目录回归测试。

## 构建与安装包

以下检查已通过：

- `npm run check`
- `npm run vite:build`
- 管理器及其测试的 Rust 编译检查。
- `cargo build --locked --offline --release -p codex-plus-manager --bin codex-plus-plus`
- .deb 元数据与文件列表检查、Desktop Entry 校验、脚本语法检查、`git diff --check`。
- `apt-get --simulate install ./dist/linux/codex-plus-plus_1.9.0+ubuntu.2_amd64.deb`：只需安装本项目一个包，运行依赖已满足，不移除或升级现有软件。

交付候选包：`dist/linux/codex-plus-plus_1.9.0+ubuntu.2_amd64.deb`，30,788,772 字节。

SHA-256：`784e8485e4ac7c3b95dfe9b751c2590da54cd1bfd54e8886829f92006e466a55`。

## 测试结果与限制

| 检查 | 结果 |
| --- | --- |
| relay_config 集成测试 | 187 通过，含新增供应商/模型切换回归 |
| installers 集成测试 | 14 通过 |
| Linux 入口与进程单元测试 | 3 通过 |
| 不支持平台的自动接管保护测试 | 1 通过 |
| 管理器更新相关测试 | 5 通过 |
| 协议转换的 chat_response 测试 | 4 通过 |
| launcher 集成测试 | 97 通过、5 失败；5 项均因沙箱禁止监听本机端口而失败 |
| 核心库完整单元测试（新增自动接管保护测试前） | 774 通过、111 失败、2 忽略；97 项直接报告沙箱禁止端口操作，另有依赖端口的断言、由此前失败引起的锁中毒，以及插件市场锁测试失败 |
| 插件市场锁测试单独复核 | 通过；未修改插件市场代码 |

未将完整测试套件标记为通过。需要在可监听本机端口的普通桌面终端复核网络相关测试，例如：

```bash
cargo test --locked --offline -p codex-plus-core --lib --test launcher --test installers --test relay_config --test protocol_proxy --no-fail-fast
```

## 启动验证

已用独立的 `CODEX_PLUS_STATE_DIR` 和 `CODEX_HOME` 尝试启动 release 二进制，避免改动现有 Codex 配置。日志记录到了 `manager.start` 和单实例锁回退，随后因沙箱内 GTK 初始化失败退出。用户随后在本机桌面终端运行同一个 release 二进制，并确认能看到管理界面。

用户随后执行 apt 安装。已通过 `dpkg-query` 确认 `codex-plus-plus` 的 `1.9.0+ubuntu.2` 状态为 `install ok installed`；`/usr/bin/codex-plus-plus` 与已验证的 release 二进制 SHA-256 一致，系统菜单入口已安装并通过 Desktop Entry 校验，运行所需共享库均可解析。apt 输出的 `_apt` 无法访问本地仓库路径提示未影响安装。

仍需用用户自己的第三方服务配置验证实际回复、工具调用和重启切换。真实 API Key 只在应用内填写。本次自动测试均使用临时目录与虚构凭据。

操作步骤见 `scripts/installer/linux/README.md`。开机启动、自动接管以及其他平台专属增强不在首版范围。尝试按仓库约定暂存源码时，`.git/index.lock` 写入被只读文件系统拒绝，因此尚未生成提交；代码与安装包均未推送到远端。
