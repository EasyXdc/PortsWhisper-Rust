# Port Whisperer

<p align="center">
  <img src="./assets/hero.svg" alt="Port Whisperer Hero" width="100%" />
</p>

<p align="center">
  <strong>🔎 用一个命令行工具快速找出谁占用了端口、查看进程细节、追踪日志，并监听本机端口变化。</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/CLI-Rust-0f172a?style=for-the-badge&logo=rust" alt="Rust CLI" />
  <img src="https://img.shields.io/badge/npm-ports--rs-CB3837?style=for-the-badge&logo=npm" alt="npm package" />
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-2563eb?style=for-the-badge" alt="Platform support" />
</p>

<p align="center">
  <a href="./README.md">🌍 English Documentation: README.md</a>
</p>

## ✨ 它能做什么

Port Whisperer 主要解决这些问题：

- 这个端口到底被哪个进程占用了？
- 它是前端开发服务、数据库、Docker 容器，还是孤儿进程？
- 它属于哪个项目、用的是什么框架？
- 我能不能直接看日志、或者直接终止它？

它提供两个命令：

- `ports`
- `whoisonport`

其中 `whoisonport <port>` 本质上就是 `ports <port>` 的别名。

## 🚀 安装

### npm

通过 npm 安装 beta 通道：

```bash
npm i -g ports-rs@next
```

安装后会暴露：

```bash
ports
whoisonport
```

### Cargo

如果你已经安装了 Rust：

```bash
cargo install --path .
```

### GitHub Releases

你也可以直接下载预构建二进制。

当前 release 资产命名约定为：

- `ports-rs-darwin-arm64.tar.gz`
- `ports-rs-darwin-x64.tar.gz`
- `ports-rs-linux-x64.tar.gz`
- `ports-rs-windows-x64.zip`

每个压缩包内包含：

- `ports`
- `whoisonport`

Windows 下对应为：

- `ports.exe`
- `whoisonport.exe`

## ⚡ 快速开始

### 查看开发相关端口

```bash
ports
```

默认只聚焦开发相关进程，例如 Node.js、Python、Java、Rust、Docker、前端 dev server 和常见本地服务。

### 查看所有监听端口

```bash
ports --all
ports -a
```

### 查看某个端口的详情

```bash
ports 3000
whoisonport 3000
```

详情页会尽量展示：

- 端口
- 进程名
- PID
- 健康状态
- 框架
- 内存占用
- 运行时间
- 启动时间
- 工作目录
- 项目名
- Git 分支
- 进程树

如果该端口存在监听进程，还会提示你是否直接终止它：

```text
Kill process on :3000? [y/N]
```

只有输入 `y` 或 `Y` 才会确认执行。

## 🧰 命令说明

### 进程列表

```bash
ports ps
ports ps --all
ports ps -a
```

`ports ps` 提供一个偏开发者视角的进程表，主要包含：

- PID
- 进程名
- CPU%
- 内存
- 项目
- 框架
- 运行时间
- 摘要命令

### 终止端口或进程

```bash
ports kill 3000
ports kill 3000 5173 8080
ports kill 3000-3010
ports kill 42872
ports kill --force 3000
```

规则：

- `1..=65535` 范围内的数字优先按端口解析
- 如果没有监听进程，再回退按 PID 解析
- 大于 `65535` 的数字只按 PID 解析
- 端口范围会自动展开
- 范围内的空端口只计入 summary，不作为硬错误

### 查看日志

```bash
ports logs 3000
ports logs 3000 -f
ports logs 3000 --follow
ports logs 3000 --lines 10
ports logs 3000 --lines=10
ports logs 3000 --err
```

Port Whisperer 会尝试发现：

- stdout / stderr 重定向文件
- `.log` / `logs/` / `nohup.out` 等路径
- macOS、Linux、Windows 的系统日志 fallback

### 清理孤儿或僵尸进程

```bash
ports clean
```

### 监听端口变化

```bash
ports watch
```

按 `Ctrl+C` 停止。

## 🧱 技术栈

| 层级 | 技术 |
| --- | --- |
| 核心 CLI | Rust |
| 打包 | npm + Node.js 安装脚本 |
| macOS 进程发现 | `lsof` + `ps` + `log` |
| Linux 进程发现 | `/proc` + `ss` / `netstat` + `ps` + `journalctl` |
| Windows 进程发现 | `netstat` + `wmic` / PowerShell + `taskkill` |

## 📊 性能表现

当前记录的本地实测数据：

| 命令 | Node 平均耗时 | Rust 平均耗时 |
| --- | ---: | ---: |
| `ports` | `0.46s` | `0.19s` |
| `ports --all` | `0.45s` | `0.18s` |
| `ports ps` | `0.15s` | `0.10s` |
| `ports <port>` | `7.20s` | `0.09s` |

这些数据来自当前开发机的 fresh 本地测试。实际结果会受到系统、硬件、后台负载和 Docker 状态影响。

## 🖥️ 平台支持

当前目标发布平台：

- macOS arm64
- macOS x64
- Linux x64
- Windows x64

## 🔗 参考项目

这个 Rust 重构版本基于原项目：

- [LarsenCundric/port-whisperer](https://github.com/LarsenCundric/port-whisperer)

当前仓库的目标是在保留核心 CLI 使用习惯和主要公开行为的前提下，用 Rust 重写整套实现。

## ❓ 常见问题

### `whoisonport` 是什么？

它本质上只是：

```bash
ports <port>
```

### npm 安装失败怎么办？

你可以：

1. 去 GitHub Releases 手动下载对应平台二进制
2. 直接从源码安装：

```bash
cargo install --path .
```

### 为什么有些字段是空的？

目录、项目、框架、Git 分支等信息依赖系统可见性、权限、cwd 探测和项目根目录识别。某些系统进程或受限进程无法完整暴露这些元数据。

### 为什么默认视图看不到所有端口？

默认 `ports` 有意聚焦开发相关进程。使用：

```bash
ports --all
```

可以看到所有监听端口。

## 🔧 开发

```bash
cargo build
cargo test
```

本地运行：

```bash
cargo run --bin ports
cargo run --bin whoisonport -- 3000
```

## 📄 License

[MIT](LICENSE)
