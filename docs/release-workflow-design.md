# Release Workflow Design

## Goal

为 `port-whisperer-rust` 增加一套面向 GitHub Releases 的自动化发布流程：当推送 `v*` tag 时，GitHub Actions 自动构建多平台 release 二进制、打包成约定好的资产名，并上传到 GitHub Release。npm 发布仍保持手动执行，不放进首版 workflow。

## Scope

本设计覆盖：

- GitHub Actions release workflow
- Release 资产命名与上传规则
- 多平台构建矩阵
- Release checklist 文档结构

本设计不覆盖：

- 自动 npm 发布
- 签名、公证、checksum 发布自动化
- Linux arm64 / Windows arm64 扩展

## Workflow Strategy

首版采用单一 tag 驱动 workflow：

- 文件路径：`.github/workflows/release.yml`
- 触发条件：`push.tags: ['v*']`

不引入 `workflow_dispatch` 作为首版入口，先保持发布链路最小可用。后续如果需要手动补包或重试能力，再扩展为双入口 workflow。

## Build Matrix

首发构建矩阵：

- `macos-latest`：产出 `darwin-arm64`
- `macos-13`：产出 `darwin-x64`
- `ubuntu-latest`：产出 `linux-x64`
- `windows-latest`：产出 `windows-x64`

每个平台 job 统一执行：

1. Checkout 仓库
2. 安装 Rust stable toolchain
3. `cargo build --release`
4. 调用 `node scripts/package-release.js <target>`
5. 上传生成的压缩包为 workflow artifact

## Release Asset Names

GitHub Release 最终资产名固定为：

- `ports-rs-darwin-arm64.tar.gz`
- `ports-rs-darwin-x64.tar.gz`
- `ports-rs-linux-x64.tar.gz`
- `ports-rs-windows-x64.zip`

压缩包内容要求：

- Unix 平台：`ports`、`whoisonport`
- Windows 平台：`ports.exe`、`whoisonport.exe`

资产命名必须和 npm `postinstall` 的平台映射完全一致，避免安装器和 Release 输出脱节。

## Release Publication Flow

在 build matrix 之后增加一个汇总 job：

1. 下载所有 workflow artifacts
2. 创建或更新对应 tag 的 GitHub Release
3. 上传四个平台资产

发布动作只负责 GitHub Release，不负责 npm。

## Release Checklist Document

建议新增文档：`docs/release-checklist.md`

文档分成 4 个区块：

### 1. Preflight

- `cargo test`
- `npm pack`
- `PORTS_RS_SKIP_DOWNLOAD=1 node scripts/postinstall.js`
- 确认 README 与 README.zh-CN.md 已更新
- 确认 `scripts/package-release.js` 本地可运行

### 2. Tag & Release

- 创建 `v0.1.0-beta.1` 或 `v0.1.0`
- 推送 tag
- 等待 GitHub Actions 完成
- 检查 Release 页面是否包含 4 个资产
- 检查资产文件名是否符合约定

### 3. Smoke Checks

- 下载任一平台资产进行解压验证
- 确认压缩包只包含 `ports` 和 `whoisonport`
- Windows 包确认包含 `.exe`
- 至少做一次本机实际运行验证

### 4. npm Follow-up

由于首版 workflow 不自动发 npm，这部分保留为手动步骤：

- `npm publish --tag next`
- 安装验证：`npm i -g ports-rs@next`
- 命令验证：`ports`、`whoisonport 3000`

## Error Handling

首版 workflow 的失败策略以“明确失败”为主：

- 任一平台构建失败，则整个 release workflow 失败
- 不上传部分成功、部分缺失的 release 资产
- npm 发布仍交由人工执行，因此不会因为 npm token 或 registry 状态阻塞 release 资产构建

## Rationale

之所以先只自动化 GitHub Release，而不自动发 npm，是因为当前 npm 层仍依赖下载 release 资产。先把上游资产构建链路自动化，能最大程度降低整体发布复杂度，同时保留手动控制 npm 发布节奏的空间。
