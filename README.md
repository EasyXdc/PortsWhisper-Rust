# Port Whisperer

`Port Whisperer` 是一个命令行工具，用来查看本机哪些进程正在监听端口，并尽量补全它们的进程、项目、框架、Docker、运行时间和状态信息。

安装后会提供两个命令：

- `ports`
- `whoisonport`

其中：

- `ports` 是主命令
- `whoisonport <port>` 是 `ports <port>` 的等价别名

## 安装

### 方式一：npm

首发 npm 包名为：

```bash
ports-rs
```

Beta 版本安装：

```bash
npm i -g ports-rs@next
```

安装完成后会暴露：

```bash
ports
whoisonport
```

### 方式二：Cargo

如果你已经安装 Rust 工具链，也可以直接从仓库安装：

```bash
cargo install --path .
```

### 方式三：GitHub Releases

也可以直接下载对应平台的预构建二进制，并将其放进你的 `PATH`。

当前 release 资产命名约定为：

- `ports-rs-darwin-arm64.tar.gz`
- `ports-rs-darwin-x64.tar.gz`
- `ports-rs-linux-x64.tar.gz`
- `ports-rs-windows-x64.zip`

压缩包内包含：

- `ports`
- `whoisonport`

Windows 下对应为：

- `ports.exe`
- `whoisonport.exe`

## 快速开始

### 查看开发相关端口

```bash
ports
```

默认只显示开发相关进程，例如 Node.js、Python、Java、Rust、Docker、前端开发服务器和常见本地服务。桌面应用和系统服务会被过滤。

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

详情页会显示：

- 端口
- 进程名
- PID
- 状态
- 框架
- 内存
- 运行时间
- 启动时间
- 项目目录
- 项目名
- Git 分支
- 进程树

如果该端口存在监听进程，详情页末尾会提示你是否直接终止它：

```text
Kill process on :3000? [y/N]
```

只有输入 `y` 或 `Y` 才会执行终止。

### 查看进程列表

```bash
ports ps
ports ps --all
ports ps -a
```

`ports ps` 提供一个偏开发者视角的进程列表，显示：

- PID
- 进程名
- CPU%
- 内存
- 项目
- 框架
- 运行时间
- 简短命令描述

默认只显示开发相关进程。`--all` 或 `-a` 会显示所有进程。

## 终止进程

```bash
ports kill 3000
ports kill 3000 5173 8080
ports kill 3000-3010
ports kill 42872
ports kill -f 3000
ports kill --force 3000
```

解析规则：

- `1..=65535` 的数字先按端口解析
- 如果端口没有监听进程，再尝试按 PID 解析
- 大于 `65535` 的数字只按 PID 解析
- `-f` 或 `--force` 会使用强制终止
- 端口范围会展开为多个端口，范围内没有监听的端口只计入 summary，不逐个报错

示例：

```text
$ ports kill 3000-3005

  Killing :3000 — node (PID 42872)
  ✓ Sent SIGTERM to :3000 — node (PID 42872)
  Killing :3001 — node (PID 95380)
  ✓ Sent SIGTERM to :3001 — node (PID 95380)
  Range summary: 2 killed, 4 empty
```

## 查看日志

```bash
ports logs 3000
ports logs 3000 -f
ports logs 3000 --follow
ports logs 3000 --lines 10
ports logs 3000 --lines=10
ports logs 3000 --err
```

日志发现逻辑：

- macOS / Linux 优先用 `lsof -p <pid>` 检查打开的文件描述符
- Linux 在必要时回退到 `/proc/<pid>/fd`
- 自动识别 stdout / stderr 重定向文件
- 自动识别 `.log`、`/log/`、`/logs/`、`nohup.out`、`stdout`、`stderr` 等路径
- 如果找不到日志文件，会回退到系统日志

系统日志回退：

- macOS：`log show` / `log stream`
- Linux：`journalctl`
- Windows：PowerShell 事件日志命令

## 清理孤儿或僵尸进程

```bash
ports clean
```

会扫描监听端口，找出状态为 `orphaned` 或 `zombie` 的开发进程，并在确认后尝试终止。

## 监听端口变化

```bash
ports watch
```

会持续监听端口新增和关闭事件。按 `Ctrl+C` 停止，退出时会打印：

```text
Stopped watching.
```

## 平台支持

当前首发目标平台：

- macOS arm64
- macOS x64
- Linux x64
- Windows x64

当前平台实现策略：

- macOS：`lsof` + `ps` + `log`
- Linux：`/proc` + `ss` / `netstat` + `ps` + `journalctl`
- Windows：`netstat` + `wmic` / PowerShell + `taskkill`

## 兼容性说明

这个 Rust 版本保持了与原有 `port-whisperer` CLI 尽量一致的公开行为，包括：

- 命令名
- 参数形式
- 常见输出字段
- 交互提示
- `whoisonport <port>` 兼容入口

## 常见问题

### npm 安装失败怎么办？

如果 npm 安装时无法下载预构建二进制，可以：

1. 去 GitHub Releases 手动下载对应平台二进制
2. 在仓库中执行：

```bash
cargo install --path .
```

### 为什么有些进程没有目录、项目或 Git 分支？

这些字段依赖系统权限、进程可见性以及 cwd / 项目根目录探测。某些系统进程、沙箱进程或权限受限进程可能无法补全这些信息。

### 为什么默认看不到某些端口？

默认 `ports` 只显示开发相关进程。使用：

```bash
ports --all
```

可以显示所有监听端口。

## 从源码构建

```bash
cargo build
```

运行：

```bash
cargo run --bin ports
cargo run --bin whoisonport -- 3000
```

测试：

```bash
cargo test
```

## 许可证

[MIT](LICENSE)
