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

典型使用场景通常是这样：

- `ports`：开发环境很乱，先快速扫一眼当前都有哪些服务在占端口
- `ports 3000`：项目启动失败，先确认 3000 到底被谁占了
- `ports check 3000 5173 8080`：启动多项本地服务前先检查端口是否空闲
- `ports logs 3000 --grep error`：日志太吵，只想看错误相关输出
- `ports open 3000`：服务已经启动，直接在浏览器里打开
- `ports kill --signal SIGINT 3000`：想优雅停止进程，而不是直接强杀

它提供两个命令：

- `ports`
- `whoisonport`

其中 `whoisonport <port>` 本质上就是 `ports <port>` 的别名。

## 🚀 安装

### npm

```bash
npm i -g ports-rs
```

安装后会暴露：

```bash
ports
whoisonport
```

如果未来需要体验预发布版本，再显式使用 `next` dist-tag。

### Cargo

如果你已经安装了 Rust，并且希望从源码安装：

```bash
git clone https://github.com/LarsenCundric/port-whisperer-rust.git
cd port-whisperer-rust
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

### 启动前先检查端口是否空闲

```bash
ports check 3000
ports check 3000 5173 8080
ports --json check 3000 5173 8080
```

这个命令很适合放在你启动前端、后端、代理服务或本地数据库之前先跑一遍。

退出码规则：

- `0`：所有端口都空闲
- `1`：至少有一个端口已被占用

### 直接在浏览器里打开本地服务

```bash
ports open 3000
```

它会直接打开 `http://localhost:3000`。

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
ports kill --signal SIGINT 3000
```

规则：

- `1..=65535` 范围内的数字优先按端口解析
- 如果没有监听进程，再回退按 PID 解析
- 大于 `65535` 的数字只按 PID 解析
- 端口范围会自动展开
- 范围内的空端口只计入 summary，不作为硬错误
- `--signal <name>` 可以显式指定信号，例如 `SIGINT`、`SIGTERM`、`SIGKILL`
- `--force` 仍然等价于 `SIGKILL`

### 查看日志

```bash
ports logs 3000
ports logs 3000 -f
ports logs 3000 --follow
ports logs 3000 --lines 10
ports logs 3000 --lines=10
ports logs 3000 --err
ports logs 3000 --grep error
ports logs 3000 --since 10m
ports logs 3000 -f --grep error
```

Port Whisperer 会尝试发现：

- stdout / stderr 重定向文件
- `.log` / `logs/` / `nohup.out` 等路径
- macOS、Linux、Windows 的系统日志 fallback

常见用法：

- `--grep <pattern>`：只保留匹配的日志行
- `--since <value>`：缩小系统日志 fallback 的查询时间范围
- `--err`：优先看 stderr（如果它被单独重定向了）

### 清理孤儿或僵尸进程

```bash
ports clean
```

### 监听端口变化

```bash
ports watch
```

按 `Ctrl+C` 停止。

## 🧭 全局参数

这些参数会在支持的命令上全局生效：

- `--json`：给脚本、CI、插件使用的结构化输出
- `--quiet`：减少装饰性输出，让终端或重定向结果更干净
- `--ascii`：强制使用 ASCII 安全字符
- `--all` / `-a`：显示全部监听端口或进程，而不只看开发相关项

示例：

```bash
ports --json
ports ps --json --all
ports --quiet
ports --ascii
TERM=dumb ports
```

## 🧱 技术栈

| 层级 | 技术 |
| --- | --- |
| 核心 CLI | Rust |
| 打包 | npm + Node.js 安装脚本 |
| macOS 进程发现 | `lsof` + `ps` + `log` |
| Linux 进程发现 | `/proc` + `ss` / `netstat` + `ps` + `journalctl` |
| Windows 进程发现 | PowerShell + `Get-NetTCPConnection` / `Get-Process` + `taskkill` |

## 📊 性能表现

当前 release 模式下的本地实测数据（`hyperfine --warmup 3`）：

| 命令 | 平均耗时 |
| --- | ---: |
| `./target/release/ports` | `99.8 ms` |
| `./target/release/ports ps` | `96.2 ms` |
| `./target/release/ports 3000` | `43.9 ms` |

和 Phase 1 基线相比：

- `ports`：`116.1 ms` -> `99.8 ms`
- `ports ps`：`96.8 ms` -> `96.2 ms`
- `ports 3000`：`54.4 ms` -> `43.9 ms`

这些数字是本机样本，不代表所有平台和所有环境下的固定表现；Docker 状态、后台负载、系统差异都会影响结果。

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

### 为什么 `ports logs` 读不到我另一个终端里正在打印的内容？

如果一个进程只是把日志直接打印到某个正在运行的终端，而没有重定向到文件或系统日志，Port Whisperer 不一定能在之后重新“抓回”这些内容。更稳定的做法是：

- 启动时把 stdout / stderr 重定向到文件
- 或者使用本身就会写日志文件 / 系统日志的服务

### 什么时候该用 `ports check`，什么时候该用 `ports 3000`？

用法区别是：

- `ports 3000`：你想看“是谁占用了这个端口”以及详情
- `ports check 3000`：你只关心“这个端口现在能不能用”

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
