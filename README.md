# port-whisperer-rust

`port-whisperer-rust` 是 `port-whisperer` 的 Rust 重写版本。目标是在保持原有 CLI 可见行为兼容的前提下，用原生二进制提升常用路径的启动速度和扫描性能。

它用于快速查看本机哪些进程正在监听端口，并尽量补全进程、项目、框架、Docker 容器、运行时间、内存和状态信息。

## 当前状态

这是迁移中的 Rust 实现，已具备第一版可运行 CLI：

- `ports`
- `whoisonport`
- `ports --all`
- `ports <number>`
- `ports ps`
- `ports ps --all`
- `ports kill ...`
- `ports logs ...`
- `ports clean`
- `ports watch`
- `ports help`

迁移任务和兼容进度记录在 [task_checklist.md](task_checklist.md)。

## 效果示例

```text
$ ports

 ┌─────────────────────────────────────┐
 │  🔊 Port Whisperer                   │
 │  listening to your ports...         │
 └─────────────────────────────────────┘

┌───────┬─────────┬───────┬──────────┬───────────┬────────┬───────────┐
│ PORT  │ PROCESS │ PID   │ PROJECT  │ FRAMEWORK │ UPTIME │ STATUS    │
├───────┼─────────┼───────┼──────────┼───────────┼────────┼───────────┤
│ :3000 │ node    │ 42872 │ frontend │ Next.js   │ 1d 9h  │ ● healthy │
│ :5173 │ node    │ 95380 │ web      │ Vite      │ 2h 40m │ ● healthy │
│ :5432 │ docker  │ 58351 │ postgres │ PostgreSQL│ 10d 3h │ ● healthy │
└───────┴─────────┴───────┴──────────┴───────────┴────────┴───────────┘

  3 ports active  ·  Run ports <number> for details  ·  --all to show everything
```

状态颜色含义：

- 绿色：`healthy`
- 黄色：`orphaned`
- 红色：`zombie`
- 灰色：`unknown`

## 构建

需要 Rust 工具链。当前项目使用 Cargo 构建：

```bash
cargo build
```

构建完成后可直接运行：

```bash
cargo run --bin ports
cargo run --bin whoisonport -- 3000
```

## 安装到本机

从当前目录安装：

```bash
cargo install --path .
```

安装后会暴露两个命令：

```bash
ports
whoisonport
```

如需先验证安装产物而不写入默认 Cargo bin 目录，可以安装到临时目录：

```bash
cargo install --path . --root /tmp/port-whisperer-install --force
```

## 命令用法

### 查看开发端口

```bash
ports
```

默认只显示开发相关进程，例如 Node.js、Python、Java、Rust、Docker、常见前端框架、本地数据库等。桌面应用和系统服务会被过滤。

### 查看所有监听端口

```bash
ports --all
ports -a
```

显示所有 TCP listening 端口，包括桌面应用、系统服务和后台进程。

### 查看指定端口详情

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

如果该端口有监听进程，详情页后会提示是否终止进程：

```text
Kill process on :3000? [y/N]
```

只有输入 `y` 或 `Y` 才会执行终止。

### 终止进程

```bash
ports kill 3000
ports kill 3000 5173 8080
ports kill 3000-3010
ports kill 42872
ports kill -f 3000
ports kill --force 3000
```

解析规则：

- `1..=65535` 的数字会先按端口解析。
- 如果端口没有监听进程，再尝试按 PID 解析。
- 大于 `65535` 的数字只按 PID 解析。
- `-f` 或 `--force` 会使用强制终止。
- 端口范围会展开为多个端口，范围内没有监听的端口只计入 summary，不逐个报错。

示例：

```text
$ ports kill 3000-3005

  Killing :3000 — node (PID 42872)
  ✓ Sent SIGTERM to :3000 — node (PID 42872)
  Killing :3001 — node (PID 95380)
  ✓ Sent SIGTERM to :3001 — node (PID 95380)
  Range summary: 2 killed, 4 empty
```

### 查看日志

```bash
ports logs 3000
ports logs 3000 -f
ports logs 3000 --follow
ports logs 3000 --lines 10
ports logs 3000 --lines=10
ports logs 3000 --err
```

日志发现逻辑：

- macOS/Linux 使用 `lsof -p <pid>` 检查打开的文件描述符。
- Linux 在必要时回退到 `/proc/<pid>/fd`。
- 自动识别 stdout/stderr 重定向文件。
- 自动识别 `.log`、`/log/`、`/logs/`、`nohup.out`、`stdout`、`stderr` 等日志路径。
- 如果找不到日志文件，会回退到系统日志：
  - macOS：`log show` / `log stream`
  - Linux：`journalctl`
  - Windows：PowerShell 事件日志命令

### 查看开发进程

```bash
ports ps
ports ps --all
ports ps -a
```

`ports ps` 类似面向开发者的 `ps` 视图，会显示：

- PID
- 进程名
- CPU%
- 内存
- 项目
- 框架
- 运行时间
- 简短命令描述

默认只显示开发相关进程。`--all` 或 `-a` 会显示所有进程。

### 清理孤儿或僵尸进程

```bash
ports clean
```

扫描监听端口，查找状态为 `orphaned` 或 `zombie` 的开发进程，并在确认后尝试终止。

### 监听端口变化

```bash
ports watch
```

实时观察端口新增和关闭事件。按 `Ctrl+C` 停止，退出时会打印：

```text
Stopped watching.
```

## 检测规则

### 开发进程

当前会识别常见开发运行时和工具，包括：

- Node.js：`node`、`npm`、`npx`、`yarn`、`pnpm`
- Python：`python`、`python3`、`uvicorn`、`gunicorn`、`flask`、`pytest`
- Ruby：`ruby`、`rails`
- Java/JVM：`java`、`gradle`、`mvn`
- Rust：`cargo`、`rustc`
- Go：`go`
- 其他：`deno`、`bun`、`php`、`dotnet`、`elixir`、`mix`
- 前端工具：`vite`、`next`、`nuxt`、`webpack`、`remix`、`astro`、`gulp`
- Docker：`com.docke*`、`docker`、`docker-sandbox`

### 框架识别

项目框架会从命令行、进程名、项目文件和 `package.json` 中推断。已覆盖：

- Next.js
- Vite
- React
- Vue
- Angular
- Svelte / SvelteKit
- Nuxt
- Remix
- Astro
- Express
- Fastify
- NestJS
- Hono
- Koa
- Django
- Flask
- FastAPI
- Rails
- Gatsby
- Webpack
- esbuild
- Parcel
- Rust
- Go
- Python
- Ruby
- Java

Docker 镜像会识别：

- PostgreSQL
- Redis
- MySQL / MariaDB
- MongoDB
- nginx
- LocalStack
- RabbitMQ
- Kafka
- Elasticsearch / OpenSearch
- MinIO

## 平台支持

| 平台 | 当前策略 |
| --- | --- |
| macOS | `lsof` + `ps` + `log` |
| Linux | `ss` / `netstat` + `/proc` + `ps` + `journalctl` |
| Windows | `netstat` + PowerShell / `taskkill` |

当前优先目标是行为兼容。后续会在保持输出一致的前提下，逐步替换为更快的原生数据源。

## 与原 Node 版本的关系

参考实现位于相邻目录：

```text
/Users/easyxdc/Desktop/PersonalDocument/MyCode/2026/port-whisperer
```

Rust 版本以该 Node 项目为兼容基准，包括命令、参数、输出字段、提示文案、检测规则和退出码。

## 开发与验证

格式化：

```bash
cargo fmt
```

编译检查：

```bash
cargo check
```

运行测试：

```bash
cargo test
```

常用烟雾测试：

```bash
cargo run --bin ports -- help
cargo run --bin ports -- --all
cargo run --bin ports -- ps --all
cargo run --bin whoisonport -- 99999
```

注意：某些系统进程信息读取在沙箱环境中可能被限制。如果 `ps` 或端口富化结果为空，需要在普通终端环境中复测。

## 许可证

[MIT](LICENSE)
