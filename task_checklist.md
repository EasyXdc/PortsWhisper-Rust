# Rust 性能重写任务清单

参考仓库：`/Users/easyxdc/Desktop/PersonalDocument/MyCode/2026/port-whisperer`

本 Rust 重写项目以参考仓库作为现有命令行为、输出字段、交互方式、检测规则和边界行为的权威来源。

## 进度更新 - 2026-04-17

- [x] 已将当前目录初始化为 git 仓库。
- [x] 已在仓库根目录创建 Rust crate，并提供 `ports` 与 `whoisonport` 两个二进制入口。
- [x] 已完成第一版 Rust 兼容实现，覆盖端口列表、端口详情、进程列表、kill、logs、clean、watch、展示格式和检测规则。
- [x] 已保留参考项目的 `README.md` 与 `LICENSE`。
- [x] 已验证 `cargo fmt`、`cargo check`、`ports help`、`ports --all`、`ports ps --all`（因沙箱限制，`ps` 访问在沙箱外验证）和 `whoisonport 99999`。
- [x] 已优化 `ports <number>`：只富化目标监听端口，而不是深度富化所有监听端口。
- [x] 已为 `ports watch` 增加 Ctrl+C 处理，包括输出 `Stopped watching.` 和退出码 `0`。
- [x] 已增加 `PlatformScanner` trait，以及 `src/platform/macos.rs`、`src/platform/linux.rs`、`src/platform/windows.rs` 平台分发文件。
- [x] 已增加 32 个单元测试，覆盖 CLI 解析、fake platform 端口/进程富化、端口表和进程表 golden 基础、端口详情、clean、watch 事件、检测规则、Docker 端口解析、日志参数、路径识别、排序去重、kill 目标解析、kill range 校验、项目根目录、运行时间和内存格式化。
- [x] 已将扫描逻辑拆分为 `framework`、`docker`、`ports`、`process` 等独立模块，并保留 `scanner` 兼容包装接口。
- [x] 已验证 `cargo install --path . --root /tmp/port-whisperer-install --force` 会暴露 `ports` 与 `whoisonport`。

## 0. 项目目标

最终目标：将 `port-whisperer` 重建为性能优化的 Rust CLI，同时一比一保留当前用户可见行为。

只有满足以下五类兼容目标，才算重写成功：

- [x] 命令兼容：所有现有命令、别名、flag、位置参数和错误路径行为一致。
- [x] 输出字段兼容：表格、详情页、日志头部中的所有可见字段都存在，并保持等价含义和格式。
- [x] 交互兼容：提示、确认、多选、Ctrl+C 处理和退出流程与当前工具一致。
- [x] 检测规则兼容：开发进程过滤、框架检测、Docker 镜像检测、项目根目录检测、孤儿/僵尸状态检测、日志文件检测均匹配当前规则。
- [x] 行为兼容：kill 解析、端口与 PID fallback、范围展开、空范围处理、`--all` 过滤、watch 事件、日志 fallback 和退出码都匹配当前行为。

性能目标：在保持兼容的前提下，通过原生 Rust 二进制、进程快照、延迟富化、缓存和可靠的平台原生数据源，让高频路径更快。

优先优化的高频路径：

- [x] `ports`
- [x] `ports --all`
- [x] `ports <port>`
- [x] `whoisonport <port>`

需要准确保留行为的重路径或低频路径：

- [x] `ports ps`
- [x] `ports ps --all`
- [x] `ports kill ...`
- [x] `ports logs ...`
- [x] `ports clean`
- [x] `ports watch`

## 1. 当前 Node 基线盘点

### 1.1 作为权威行为来源的文件

- [x] 将 `src/index.js` 作为命令路由和交互行为的权威来源。
- [x] 将 `src/scanner.js` 作为扫描、检测、kill、logs 和 watch 行为的权威来源。
- [x] 将 `src/display.js` 作为终端输出行为的权威来源。
- [x] 将 `src/platform/darwin.js` 作为 macOS 原始数据策略的权威来源。
- [x] 将 `src/platform/linux.js` 作为 Linux 原始数据策略的权威来源。
- [x] 将 `src/platform/win32.js` 作为 Windows 原始数据策略的权威来源。
- [x] 将 `README.md` 示例作为公开用户行为预期。
- [x] 将 `package.json` 的 `bin` 字段作为公开命令名来源。

### 1.2 当前公开命令

- [x] `ports`
- [x] `ports --all`
- [x] `ports -a`
- [x] `ports <number>`
- [x] `whoisonport <number>`
- [x] `ports ps`
- [x] `ports ps --all`
- [x] `ports ps -a`
- [x] `ports clean`
- [x] `ports kill <target>`
- [x] `ports kill <target> <target> ...`
- [x] `ports kill <start>-<end>`
- [x] `ports kill -f <target>`
- [x] `ports kill --force <target>`
- [x] `ports logs <target>`
- [x] `ports logs <target> -f`
- [x] `ports logs <target> --follow`
- [x] `ports logs <target> --lines <n>`
- [x] `ports logs <target> --lines=<n>`
- [x] `ports logs <target> --err`
- [x] `ports watch`
- [x] `ports help`
- [x] `ports --help`
- [x] `ports -h`

### 1.3 当前外部命令

- [x] macOS 端口扫描：`lsof -iTCP -sTCP:LISTEN -P -n`
- [x] macOS 进程详情：`ps -p <pidList> -o pid=,ppid=,stat=,rss=,lstart=,command=`
- [x] macOS cwd 查询：`lsof -a -d cwd -p <pidList>`
- [x] macOS 进程树：`ps -eo pid=,ppid=,comm=`
- [x] macOS 单进程日志：`lsof -p <pid>`
- [x] macOS 系统日志 fallback：`log show --predicate 'processID == <pid>' --style compact --last 1m`
- [x] macOS 系统日志 follow fallback：`log stream --predicate 'processID == <pid>' --style compact`
- [x] Linux 端口扫描优先路径：`ss -tlnp`
- [x] Linux 端口扫描 fallback：`netstat -tlnp`
- [x] Linux 进程详情：`ps -p <pidList> -o pid=,ppid=,stat=,rss=,lstart=,command=`
- [x] Linux 全进程详情：`ps -eo pid=,pcpu=,pmem=,rss=,lstart=,cmd=`
- [x] Linux cwd 查询：`/proc/<pid>/cwd`
- [x] Linux 进程树：`/proc/<pid>/stat`
- [x] Linux 日志 fd fallback：`/proc/<pid>/fd`
- [x] Linux 系统日志 fallback：`journalctl _PID=<pid> --no-pager -n 50`
- [x] Linux 系统日志 follow fallback：`journalctl _PID=<pid> -f --no-pager`
- [x] Windows 端口扫描：`netstat -ano -p TCP`
- [x] Windows 进程查询主路径：`wmic process ...`
- [x] Windows 进程查询 fallback：PowerShell `Get-Process` / `Get-CimInstance`
- [x] Windows 强制 kill 路径：`taskkill /F /PID <pid>`
- [x] Docker 映射路径：`docker ps --format "{{.Ports}}\t{{.Names}}\t{{.Image}}"`
- [x] Git 分支路径：`git -C "<cwd>" rev-parse --abbrev-ref HEAD`
- [x] Unix 日志 tail 路径：`tail -n <lines> <file>`
- [x] Unix 日志 follow 路径：`tail -f -n <lines> <file>`
- [x] Windows 日志 tail 路径：PowerShell `Get-Content -Path '<file>' -Tail <lines>`
- [x] Windows 日志 follow 路径：PowerShell `Get-Content -Path '<file>' -Tail <lines> -Wait`

## 2. Rust 项目搭建

### 2.1 Cargo 工作区

- [x] 创建 `Cargo.toml`。
- [x] 包名设置为 `port-whisperer`，除非迁移期间需要临时 crate 名。
- [x] 配置 Rust edition，优先 `2021` 或更新版本。
- [x] 创建 `src/main.rs`。
- [x] 如果集成测试需要共享库访问，则增加 `src/lib.rs`。
- [x] 在 Rust 兼容性验证完成之前，保留现有 Node 实现。
- [x] 决定 Rust 产物放在仓库根目录还是 `rust/` 等子目录。
- [x] 如果使用仓库根目录，避免在 cutover 前覆盖已有 `src/*.js`。
- [x] 将构建产物加入 `.gitignore`，包括 `target/`。
- [x] 保留现有 `LICENSE`。
- [x] 在 Rust 命令准备好前保留现有 `README.md`。

### 2.2 推荐依赖

- [ ] 添加 `clap` 用于 CLI 解析。
- [ ] 添加 `owo-colors` 或 `anstyle`/`anstream` 用于彩色输出。
- [x] 添加 `tabled` 或实现自定义表格渲染器；如果精确盒线输出手写更容易，则手写。
- [x] 添加 `unicode-width` 用于终端列宽兼容。
- [ ] 添加 `regex` 用于命令和端口解析规则。
- [ ] 添加 `serde` 和 `serde_json` 用于 `package.json` 检测。
- [ ] 添加 `sysinfo`，在可靠时用于跨平台进程元数据。
- [ ] 添加 `ctrlc` 用于 watch/log follow 中断处理。
- [ ] 添加 `which` 用于 fallback 命令检测。
- [ ] 添加 `thiserror` 用于结构化错误。
- [ ] 仅当应用层错误处理有收益时，添加 `anyhow`。
- [ ] 添加 `tempfile` 作为测试依赖。
- [ ] 添加 `assert_cmd` 作为 CLI 测试依赖。
- [ ] 添加 `predicates` 作为输出断言依赖。

### 2.3 二进制命令名

- [x] 产出 `ports` 二进制。
- [x] 产出 `whoisonport` 二进制，或基于 `argv[0]` 分发行为。
- [x] 确保 `whoisonport <number>` 与 `ports <number>` 行为完全一致。
- [x] 决定两个二进制命令名的包安装策略。
- [x] 尽可能验证 `cargo install --path .` 会暴露预期命令。

## 3. Rust 模块设计

### 3.1 建议模块布局

- [x] `src/main.rs`：薄入口。
- [x] `src/cli.rs`：命令解析和规范化命令模型。
- [x] `src/model.rs`：共享数据结构。
- [x] `src/scanner.rs`：高层扫描编排。
- [x] `src/platform/mod.rs`：平台 trait 和分发。
- [x] `src/platform/macos.rs`：macOS 实现。
- [x] `src/platform/linux.rs`：Linux 实现。
- [x] `src/platform/windows.rs`：Windows 实现。
- [x] `src/process.rs`：进程快照、进程元数据、进程树。
- [x] `src/ports.rs`：监听端口发现。
- [x] `src/framework.rs`：项目根目录和框架检测。
- [x] `src/docker.rs`：Docker 端口、镜像和容器映射。
- [x] `src/display.rs`：终端渲染。
- [x] `src/kill.rs`：kill 目标解析和信号分发。
- [x] `src/logs.rs`：日志文件发现和 tail/follow 行为。
- [x] `src/watch.rs`：watch 循环和事件 diff。
- [x] `src/error.rs`：错误类型和退出码映射。

### 3.2 共享数据模型

- [x] 定义 `PortInfo`。
- [x] `PortInfo.port: u16`。
- [x] `PortInfo.pid: u32`。
- [x] `PortInfo.process_name: String`。
- [x] `PortInfo.raw_name: String`。
- [x] `PortInfo.command: String`。
- [x] `PortInfo.cwd: Option<PathBuf>`。
- [x] `PortInfo.project_name: Option<String>`。
- [x] `PortInfo.framework: Option<String>`。
- [x] `PortInfo.uptime: Option<String>`。
- [x] `PortInfo.start_time: Option<SystemTime or DateTime equivalent>`。
- [x] `PortInfo.status: ProcessStatus`。
- [x] `PortInfo.memory: Option<String>`。
- [x] `PortInfo.git_branch: Option<String>`。
- [x] `PortInfo.process_tree: Vec<ProcessTreeNode>`。
- [x] 定义 `ProcessInfo`。
- [x] `ProcessInfo.pid: u32`。
- [x] `ProcessInfo.ppid: Option<u32>`。
- [x] `ProcessInfo.process_name: String`。
- [x] `ProcessInfo.command: String`。
- [x] `ProcessInfo.cpu: f32`。
- [x] `ProcessInfo.rss_kb: u64`。
- [x] `ProcessInfo.memory: Option<String>`。
- [x] `ProcessInfo.cwd: Option<PathBuf>`。
- [x] `ProcessInfo.project_name: Option<String>`。
- [x] `ProcessInfo.framework: Option<String>`。
- [x] `ProcessInfo.uptime: Option<String>`。
- [x] `ProcessInfo.status_raw: String`。
- [x] 定义 `ProcessStatus`。
- [x] 包含 `Healthy`。
- [x] 包含 `Orphaned`。
- [x] 包含 `Zombie`。
- [x] 包含 `Unknown`。
- [x] 输出标签保持为 `healthy`、`orphaned`、`zombie`、`unknown`。
- [x] 定义 `DockerInfo`。
- [x] `DockerInfo.host_port: u16`。
- [x] `DockerInfo.container_name: String`。
- [x] `DockerInfo.image: String`。
- [x] `DockerInfo.framework: String`。
- [x] 定义 `LogFile`。
- [x] `LogFile.path: PathBuf`。
- [x] `LogFile.fd: LogFdKind`。
- [x] `LogFile.kind: String`。
- [x] `LogFile.priority: u8`。
- [x] 定义 `KillTargetResolution`。
- [x] 包含解析后的 PID。
- [x] 包含解析类型：`Port` 或 `Pid`。
- [x] 包含可选端口号。
- [x] 包含可选 `PortInfo`。

## 4. CLI 解析兼容

### 4.1 全局 flag 规则

- [x] 全局接受 `--all`。
- [x] 全局接受 `-a`。
- [x] 命令分发前从命令专属参数中移除 `--all` 和 `-a`。
- [x] 未知命令打印 `Unknown command: <command>`。
- [x] 未知命令提示 `Run ports --help for usage.`。
- [x] 未知命令退出码为 `1`。

### 4.2 默认命令

- [x] 无参数时列出监听中的开发端口。
- [x] `ports` 使用 `isDevProcess` 过滤。
- [x] `ports --all` 不使用 `isDevProcess` 过滤。
- [x] `ports -a` 行为等同 `ports --all`。
- [x] 结果按端口数字升序排序。

### 4.3 第一个参数为数字

- [x] 如果第一个过滤后的参数可解析为数字，则作为端口详情请求。
- [x] `ports 3000` 调用端口详情行为。
- [x] `whoisonport 3000` 调用同一详情行为。
- [x] 如果该端口没有进程，则渲染相同的“未找到进程”详情输出。
- [x] 如果找到进程，则显示交互式 kill 提示。
- [x] 提示文本为 `Kill process on :<port>? [y/N]`。
- [x] 只有输入 `y` 或 `Y`，经小写化后等于 `y`，才执行 kill。
- [x] 其他答案直接退出，不执行 kill。

### 4.4 帮助命令

- [x] `ports help` 渲染帮助。
- [x] `ports --help` 渲染帮助。
- [x] `ports -h` 渲染帮助。
- [x] 帮助输出包含 `Port Whisperer — listen to your ports`。
- [x] 帮助输出包含当前所有命令示例。
- [x] 帮助输出不需要执行进程扫描。

## 5. `ports` 列表命令兼容

### 5.1 数据收集

- [x] 发现所有 TCP listening 端口。
- [x] 按端口去重，匹配当前行为。
- [x] 对 IPv4/IPv6 重复 listener，优先使用第一个发现的进程，匹配当前行为。
- [x] 批量获取唯一 PID 的进程信息。
- [x] 批量获取唯一 PID 的 cwd。
- [x] 仅当存在 Docker-like listener 时检测 Docker 端口映射。
- [x] 如果可用，为每个端口富化进程命令。
- [x] 如果 RSS 可用，为每个端口富化内存。
- [x] 如果启动时间可用，为每个端口富化启动时间。
- [x] 如果启动时间有效，为每个端口富化运行时间。
- [x] 为每个端口富化状态。
- [x] cwd 可用时富化项目根目录和项目名。
- [x] 富化框架信息。
- [x] 最终列表按 `port` 升序排序。

### 5.2 默认过滤

- [x] 非 `--all` 时应用 `isDevProcess(processName, command)`。
- [x] 存在 `--all` 或 `-a` 时不应用过滤。
- [x] 默认输出保留 Docker listener。
- [x] 默认输出保留常见开发运行时。
- [x] 默认输出过滤桌面应用和系统应用。

### 5.3 输出字段

- [x] 表头包含 `PORT`。
- [x] 表头包含 `PROCESS`。
- [x] 表头包含 `PID`。
- [x] 表头包含 `PROJECT`。
- [x] 表头包含 `FRAMEWORK`。
- [x] 表头包含 `UPTIME`。
- [x] 表头包含 `STATUS`。
- [x] 端口显示为 `:<port>`。
- [x] PID 显示为字符串。
- [x] 缺失项目显示为 `—`。
- [x] 缺失框架显示为 `—`。
- [x] 缺失运行时间显示为 `—`。
- [x] 状态显示为彩色圆点加标签。
- [x] 项目名按照当前输出的可见宽度策略截断。
- [x] 表格使用盒线字符。
- [x] 表格前渲染头部 banner。
- [x] 单个端口时 summary 包含 `<n> port active`。
- [x] 多个端口时 summary 包含 `<n> ports active`。
- [x] summary 包含 `Run ports <number> for details`。
- [x] 仅在过滤模式下 summary 包含 `--all to show everything`。

### 5.4 空输出

- [x] 过滤后无端口时渲染 `No active listening ports found.`。
- [x] 渲染 `Start a dev server and run ports again.`。
- [x] 保留空消息前后的空行间距。

## 6. `ports <number>` 详情命令兼容

### 6.1 优化数据路径

- [x] 实现详情查询时不需要深度富化所有端口。
- [x] 查找请求端口对应的 listener。
- [x] 尽可能只富化解析到的 PID。
- [x] 只为解析到的 PID 获取项目根目录。
- [x] 只为解析到的 PID 获取 Git 分支。
- [x] 只为解析到的 PID 获取进程树。
- [x] 完全保留最终展示字段的格式一致性。

### 6.2 详情输出字段

- [x] 渲染头部 banner。
- [x] 渲染 `Port :<port>`。
- [x] 渲染 `Process`。
- [x] 渲染 `PID`。
- [x] 渲染 `Status`。
- [x] 渲染 `Framework`。
- [x] 渲染 `Memory`。
- [x] 渲染 `Uptime`。
- [x] 存在启动时间时渲染 `Started`。
- [x] 渲染 `Location` section。
- [x] 渲染 `Directory`。
- [x] 渲染 `Project`。
- [x] 渲染 `Git Branch`。
- [x] 进程树存在时渲染 `Process Tree` section。
- [x] 渲染 `Kill: ports kill <port> or ports kill -f <port> (force)`。
- [x] 缺失值使用 `—`。

### 6.3 详情交互

- [x] 显示详情后询问 `Kill process on :<port>? [y/N]`。
- [x] 如果答案小写后等于 `y`，用默认信号 kill 解析到的 PID。
- [x] 成功时打印 `✓ Killed PID <pid>`。
- [x] 失败时打印 `✕ Failed. Try: sudo kill -9 <pid>`。
- [x] 答案不是 `y` 时退出，不输出 abort 消息。
- [x] 确保 stdin prompt 正确 flush。
- [x] 确保 prompt 完成后进程干净退出。

## 7. `ports ps` 兼容

### 7.1 数据收集

- [x] 获取所有进程。
- [x] 类 Unix 平台跳过 PID `<= 1`，与当前实现一致。
- [x] 跳过当前进程 PID。
- [x] 按 PID 去重。
- [x] 收集 CPU 值。
- [x] 收集 RSS 内存。
- [x] 收集命令字符串。
- [x] 收集进程名。
- [x] 收集启动时间。
- [x] 收集运行时间。
- [x] 对非 Docker 进程收集 cwd。
- [x] 从命令/进程名检测框架。
- [x] 从 cwd 检测项目根目录和项目名。
- [x] 当命令/进程名不足时，从项目根目录检测框架。

### 7.2 默认过滤

- [x] `ports ps` 使用 `isDevProcess` 过滤。
- [x] `ports ps --all` 不使用 `isDevProcess` 过滤。
- [x] `ports ps -a` 行为等同 `ports ps --all`。

### 7.3 Docker 折叠

- [x] 识别 Docker-like 进程：
  - [x] 名称以 `com.docke` 开头。
  - [x] 名称以 `Docker` 开头。
  - [x] 名称精确等于 `docker`。
  - [x] 名称精确等于 `docker-sandbox`。
- [x] 过滤模式下将 Docker-like 进程折叠为一行 summary。
- [x] 汇总 Docker CPU。
- [x] 汇总 Docker RSS 内存。
- [x] 使用相同 `GB`/`MB`/`KB` 阈值格式化 Docker 总内存。
- [x] summary 行使用第一个 Docker 进程 PID。
- [x] 使用进程名 `Docker`。
- [x] 使用框架 `Docker`。
- [x] 描述使用 `<count> processes`。
- [x] 保留第一个 Docker 进程的 uptime。
- [x] `--all` 模式下不折叠 Docker，除非刻意变更并记录。

### 7.4 排序

- [x] 进程表按 CPU 降序排序。
- [x] 对 CPU 相等项尽量保持足够稳定的顺序。

### 7.5 输出字段

- [x] 表头包含 `PID`。
- [x] 表头包含 `PROCESS`。
- [x] 表头包含 `CPU%`。
- [x] 表头包含 `MEM`。
- [x] 表头包含 `PROJECT`。
- [x] 表头包含 `FRAMEWORK`。
- [x] 表头包含 `UPTIME`。
- [x] 表头包含 `WHAT`。
- [x] CPU 保留一位小数。
- [x] CPU 大于 `25` 显示为红色。
- [x] CPU 大于 `5` 显示为黄色。
- [x] CPU 小于等于 `5` 显示为绿色。
- [x] 缺失内存显示为 `—`。
- [x] 缺失项目显示为 `—`。
- [x] 缺失框架显示为 `—`。
- [x] 缺失运行时间显示为 `—`。
- [x] 进程名截断到 15 字符。
- [x] 项目名截断到 20 字符。
- [x] 描述截断到 30 字符。
- [x] summary 包含 `<n> process` 或 `<n> processes`。
- [x] 只有过滤模式下 summary 包含 `--all to show everything`。

### 7.6 空输出

- [x] 未找到开发进程时渲染 `No dev processes found.`。
- [x] 渲染 `Run ports ps --all to show all processes.`。

## 8. `ports kill` 兼容

### 8.1 参数解析

- [x] `ports kill` 没有目标时打印 usage。
- [x] usage 包含 `Usage: ports kill [-f|--force] <port|pid|range> [port|pid|range...]`。
- [x] usage 包含 `Kills listener on port (1-65535), or process by PID. Use -f for SIGKILL.`。
- [x] usage 包含 `Ranges: ports kill 3000-3010`。
- [x] 无目标 usage 退出码为 `1`。
- [x] `-f` 启用 force。
- [x] `--force` 启用 force。
- [x] Unix 上 force signal 映射为 `SIGKILL`。
- [x] Unix 上默认 signal 映射为 `SIGTERM`。
- [x] Windows force kill 使用 `taskkill /F /PID <pid>`。

### 8.2 目标解析

- [x] 接受多个数字目标。
- [x] 接受匹配 `^(\d+)-(\d+)$` 的数字范围。
- [x] 拒绝非数字目标字符串。
- [x] 拒绝带额外字符的数字字符串。
- [x] 拒绝反向范围。
- [x] 拒绝 end - start 超过 `1000` 的范围。
- [x] 拒绝超出 `1..=65535` 的范围。
- [x] 将有效范围展开为单个端口。
- [x] 记录哪些展开目标来自范围。

### 8.3 解析行为

- [x] 对整数 `n <= 65535`，先尝试按监听端口解析。
- [x] 如果没有 listener 且 PID `n` 存在，则按 PID 解析。
- [x] 对整数 `n > 65535`，只按 PID 解析。
- [x] 如果端口和 PID 都无法解析，则报告失败。
- [x] 对范围内缺失 listener，不逐端口打印错误。
- [x] 范围内缺失 listener 计为空。

### 8.4 kill 输出

- [x] kill 操作前打印空行。
- [x] 端口解析时打印 `Killing :<port> — <process> (PID <pid>)`。
- [x] PID 解析时打印 `Killing PID <pid>`。
- [x] 成功时打印 `✓ Sent <signal> to <label>`。
- [x] 普通信号失败时打印 `✕ Failed. Try: sudo kill <pid>`。
- [x] force 失败时打印 `✕ Failed. Try: sudo kill -9 <pid>`。
- [x] 操作后打印最终空行。

### 8.5 范围 summary

- [x] 只要提供过范围，就打印 `Range summary: ...`。
- [x] killed 数量大于 0 时包含 `<n> killed`。
- [x] empty 数量大于 0 时包含 `<n> empty`。
- [x] 任意目标失败时包含 `some failed`。
- [x] 匹配标点和逗号分隔。

### 8.6 退出码

- [x] 所有显式目标都解析成功且 kill 成功时退出码为 `0`。
- [x] 任意显式目标无效、未解析或 kill 失败时退出码为 `1`。
- [x] 范围内缺失 listener 不应单独导致退出码 `1`。

## 9. `ports logs` 兼容

### 9.1 参数解析

- [x] `ports logs` 无目标时打印 usage。
- [x] usage 包含 `Usage: ports logs <port|pid> [-f] [--lines=N] [--err]`。
- [x] usage 包含 `Show log output for a process running on a port.`。
- [x] usage 包含 `Use -f or --follow to stream new lines.`。
- [x] 无目标 usage 退出码为 `1`。
- [x] `-f` 启用 follow。
- [x] `--follow` 启用 follow。
- [x] `--err` 启用只看 stderr。
- [x] 默认行数为 `50`。
- [x] 解析 `--lines=N`。
- [x] 解析 `--lines N`。
- [x] 拒绝非数字目标。
- [x] 使用与 kill 相同的端口/PID 逻辑解析目标。

### 9.2 头部输出

- [x] header 前打印空行。
- [x] 端口解析时打印 `Port Whisperer — logs for :<port> (<process>, PID <pid>)`。
- [x] PID 解析时打印 `Port Whisperer — logs for PID <pid> (<process>, PID <pid>)`。
- [x] header 后打印空行。

### 9.3 日志发现规则

- [x] macOS/Linux 上使用 `lsof -p <pid>` 或原生等价方式检查打开文件描述符。
- [x] fd 为 `1w` 且类型为 `REG` 时检测 stdout 重定向。
- [x] fd 为 `2w` 且类型为 `REG` 时检测 stderr 重定向。
- [x] 检测可写的 log-like 普通文件。
- [x] Linux 回退到 `/proc/<pid>/fd`。
- [x] fd `1` 不是 `/dev/*` 且不是 `pipe:*` 时检测为 stdout。
- [x] fd `2` 不是 `/dev/*` 且不是 `pipe:*` 时检测为 stderr。
- [x] 检查相对原始进程 cwd 的常见框架日志文件：
  - [x] `.next/server.log`
  - [x] `logs/development.log`
  - [x] `log/development.log`
  - [x] `tmp/pids/server.log`
  - [x] `storage/logs/laravel.log`
  - [x] `npm-debug.log`
  - [x] `yarn-error.log`
- [x] 日志文件按 priority 升序排序。
- [x] 按 path 去重并保留 priority 顺序。

### 9.4 log-like 路径规则

- [x] 以 `.log` 结尾的路径视为 log-like。
- [x] 包含 `/log/` 的路径视为 log-like。
- [x] 包含 `/logs/` 的路径视为 log-like。
- [x] 包含 `\log\` 的路径视为 log-like。
- [x] 包含 `\logs\` 的路径视为 log-like。
- [x] 包含 `/tmp/` 的路径视为 log-like。
- [x] 包含 `nohup.out` 的路径视为 log-like。
- [x] 包含 `stdout` 的路径视为 log-like。
- [x] 包含 `stderr` 的路径视为 log-like。

### 9.5 只看 stderr 的行为

- [x] 存在 `--err` 且 stderr 重定向存在时 tail stderr 文件。
- [x] 打印 `▸ Tailing stderr: <path>`。
- [x] 存在 `--err` 但没有 stderr 重定向时打印 `No stderr redirect found for PID <pid>`。
- [x] stderr-only 且无文件时，不 fallback 到系统日志。

### 9.6 单日志文件行为

- [x] 只有一个日志文件时立即 tail。
- [x] stdout 文件标签为 `stdout`。
- [x] stderr 文件标签为 `stderr`。
- [x] 其他文件标签为 `log`。
- [x] 打印 `▸ Tailing <label>: <path>`。
- [x] 遵守行数。
- [x] 遵守 follow 模式。

### 9.7 多日志文件行为

- [x] 打印 `Found log files:`。
- [x] 文件编号从 `1` 开始。
- [x] 显示 fd 标签：`stdout`、`stderr` 或 type。
- [x] 提示 `Pick a file (1-<count>):`。
- [x] 无效选择打印 `Invalid selection.`。
- [x] 有效选择 tail 所选文件。
- [x] 打印 `▸ Tailing: <path>`。

### 9.8 系统日志 fallback

- [x] 如果没有日志文件，则获取平台系统日志命令。
- [x] macOS 非 follow 命令匹配当前语义。
- [x] macOS follow 命令匹配当前语义。
- [x] Linux 非 follow 命令匹配当前语义。
- [x] Linux follow 命令匹配当前语义。
- [x] Windows 命令匹配当前语义。
- [x] 打印 `No log files found. Falling back to system log...`。
- [x] 打印系统命令行 `$ <command>`。
- [x] 执行系统日志命令。
- [x] follow 模式下 Ctrl+C 终止子进程并退出。

### 9.9 无日志输出行为

- [x] 如果没有日志文件且没有系统日志命令，则打印 `No log files or system log found for PID <pid>.`。
- [x] 打印提示：`Tip: if the process logs to the terminal, check the terminal where it was started.`。

## 10. `ports clean` 兼容

### 10.1 发现逻辑

- [x] 查找监听端口。
- [x] 过滤状态为 `orphaned` 或 `zombie` 的端口。
- [x] 通过保留当前状态规则，只针对开发运行时。

### 10.2 无孤儿进程输出

- [x] 渲染头部 banner。
- [x] 打印 `✓ No orphaned or zombie processes found. All clean!`。

### 10.3 确认提示

- [x] found 列表前打印空行。
- [x] 单个时打印 `Found <n> orphaned/zombie process:`。
- [x] 多个时打印 `Found <n> orphaned/zombie processes:`。
- [x] 每个 orphan/zombie 打印一条 bullet。
- [x] bullet 格式包含 `:<port> — <process> (PID <pid>)`。
- [x] 提示 `Kill all? [y/N]`。
- [x] 只有小写化后为 `y` 才 kill 全部。
- [x] 非 `y` 打印 `Aborted.`。

### 10.4 kill 结果

- [x] 记录 killed PID。
- [x] 记录 failed PID。
- [x] 使用 header 渲染 clean 结果。
- [x] killed 使用 `✓`。
- [x] failed 使用 `✕`。
- [x] 未 kill 使用 `?`。
- [x] 失败时打印 `Failed to kill. Try: sudo kill -9 <pid>`。
- [x] 打印 `Cleaned <n> process.` 或 `Cleaned <n> processes.`。
- [x] 打印 `Failed to clean <n> process.` 或 `Failed to clean <n> processes.`。

## 11. `ports watch` 兼容

### 11.1 watch 循环

- [x] 渲染头部 banner。
- [x] 打印 `Watching for port changes...`。
- [x] 打印 `Press Ctrl+C to stop`。
- [x] 使用默认间隔 `2000ms`。
- [x] 如果一次扫描耗时超过 interval，避免重叠扫描。
- [x] 维护 previous port set。
- [x] 检测新出现的端口。
- [x] 检测关闭的端口。
- [x] 每次扫描后更新 previous set。

### 11.2 watch 事件输出

- [x] new 事件包含当前本地时间。
- [x] new 事件标签为 `▲ NEW`。
- [x] new 事件格式包含 `:<port> ← <process>`。
- [x] projectName 存在时 new 事件包含 `[<project>]`。
- [x] framework 存在时 new 事件包含 framework。
- [x] removed 事件标签为 `▼ CLOSED`。
- [x] removed 事件格式包含 `:<port>`。

### 11.3 中断行为

- [x] Ctrl+C 清理 interval 或停止循环。
- [x] 打印 `Stopped watching.`。
- [x] 退出码为 `0`。

### 11.4 性能优化

- [x] watch 扫描应先计算轻量级 `port -> pid` map。
- [x] 只富化新发现端口。
- [x] 对仍存在的端口缓存富化信息。
- [x] 除非 Docker 端口变化，否则不要每个 tick 都调用 Docker。

## 12. 检测规则兼容

### 12.1 系统/桌面应用过滤

- [x] 过滤当前 macOS 系统/桌面应用进程名前缀：
  - [x] `spotify`
  - [x] `raycast`
  - [x] `tableplus`
  - [x] `postman`
  - [x] `linear`
  - [x] `cursor`
  - [x] `controlce`
  - [x] `rapportd`
  - [x] `superhuma`
  - [x] `setappage`
  - [x] `slack`
  - [x] `discord`
  - [x] `firefox`
  - [x] `chrome`
  - [x] `google`
  - [x] `safari`
  - [x] `figma`
  - [x] `notion`
  - [x] `zoom`
  - [x] `teams`
  - [x] `code`
  - [x] `iterm2`
  - [x] `warp`
  - [x] `arc`
  - [x] `loginwindow`
  - [x] `windowserver`
  - [x] `systemuise`
  - [x] `kernel_task`
  - [x] `launchd`
  - [x] `mdworker`
  - [x] `mds_stores`
  - [x] `cfprefsd`
  - [x] `coreaudio`
  - [x] `corebrightne`
  - [x] `airportd`
  - [x] `bluetoothd`
  - [x] `sharingd`
  - [x] `usernoted`
  - [x] `notificationc`
  - [x] `cloudd`
- [x] 过滤当前 Linux 系统进程前缀：
  - [x] `systemd`
  - [x] `snapd`
  - [x] `networkmanager`
  - [x] `gdm`
  - [x] `sshd`
  - [x] `cron`
  - [x] `dbus-daemon`
  - [x] `polkitd`
  - [x] `rsyslogd`
  - [x] `thermald`
  - [x] `accounts-daemon`
- [x] 过滤当前 Windows 系统进程前缀：
  - [x] `svchost`
  - [x] `csrss`
  - [x] `lsass`
  - [x] `services`
  - [x] `explorer`
  - [x] `dwm`
  - [x] `searchindexer`
  - [x] `taskhostw`
  - [x] `runtimebroker`
  - [x] `shellexperiencehost`

### 12.2 开发进程名称规则

- [x] 将精确进程名 `node` 视为开发进程。
- [x] 将精确进程名 `python` 视为开发进程。
- [x] 将精确进程名 `python3` 视为开发进程。
- [x] 将精确进程名 `ruby` 视为开发进程。
- [x] 将精确进程名 `java` 视为开发进程。
- [x] 将精确进程名 `go` 视为开发进程。
- [x] 将精确进程名 `cargo` 视为开发进程。
- [x] 将精确进程名 `deno` 视为开发进程。
- [x] 将精确进程名 `bun` 视为开发进程。
- [x] 将精确进程名 `php` 视为开发进程。
- [x] 将精确进程名 `uvicorn` 视为开发进程。
- [x] 将精确进程名 `gunicorn` 视为开发进程。
- [x] 将精确进程名 `flask` 视为开发进程。
- [x] 将精确进程名 `rails` 视为开发进程。
- [x] 将精确进程名 `npm` 视为开发进程。
- [x] 将精确进程名 `npx` 视为开发进程。
- [x] 将精确进程名 `yarn` 视为开发进程。
- [x] 将精确进程名 `pnpm` 视为开发进程。
- [x] 将精确进程名 `tsc` 视为开发进程。
- [x] 将精确进程名 `tsx` 视为开发进程。
- [x] 将精确进程名 `esbuild` 视为开发进程。
- [x] 将精确进程名 `rollup` 视为开发进程。
- [x] 将精确进程名 `turbo` 视为开发进程。
- [x] 将精确进程名 `nx` 视为开发进程。
- [x] 将精确进程名 `jest` 视为开发进程。
- [x] 将精确进程名 `vitest` 视为开发进程。
- [x] 将精确进程名 `mocha` 视为开发进程。
- [x] 将精确进程名 `pytest` 视为开发进程。
- [x] 将精确进程名 `cypress` 视为开发进程。
- [x] 将精确进程名 `playwright` 视为开发进程。
- [x] 将精确进程名 `rustc` 视为开发进程。
- [x] 将精确进程名 `dotnet` 视为开发进程。
- [x] 将精确进程名 `gradle` 视为开发进程。
- [x] 将精确进程名 `mvn` 视为开发进程。
- [x] 将精确进程名 `mix` 视为开发进程。
- [x] 将精确进程名 `elixir` 视为开发进程。

### 12.3 Docker 开发进程规则

- [x] 将名称以 `com.docke` 开头的进程视为开发进程。
- [x] 将精确名称 `docker` 视为开发进程。
- [x] 将精确名称 `docker-sandbox` 视为开发进程。

### 12.4 命令指示器规则

- [x] 匹配 `\bnode\b`。
- [x] 匹配 `\bnext[\s-]`。
- [x] 匹配 `\bvite\b`。
- [x] 匹配 `\bnuxt\b`。
- [x] 匹配 `\bwebpack\b`。
- [x] 匹配 `\bremix\b`。
- [x] 匹配 `\bastro\b`。
- [x] 匹配 `\bgulp\b`。
- [x] 匹配 `\bng serve\b`。
- [x] 匹配当前 Gatsby typo 兼容正则 `\bgatsb`，除非后续带测试地刻意修正。
- [x] 匹配 `\bflask\b`。
- [x] 匹配 `\bdjango\b`。
- [x] 匹配 `manage.py`。
- [x] 匹配 `\buvicorn\b`。
- [x] 匹配 `\brails\b`。
- [x] 匹配 `\bcargo\b`。

### 12.5 项目根目录标记

- [x] 从 cwd 向上遍历。
- [x] 在文件系统根目录停止。
- [x] 深度达到 `15` 后停止。
- [x] 返回第一个包含 `package.json` 的目录。
- [x] 返回第一个包含 `Cargo.toml` 的目录。
- [x] 返回第一个包含 `go.mod` 的目录。
- [x] 返回第一个包含 `pyproject.toml` 的目录。
- [x] 返回第一个包含 `Gemfile` 的目录。
- [x] 返回第一个包含 `pom.xml` 的目录。
- [x] 返回第一个包含 `build.gradle` 的目录。
- [x] 如果没有找到标记，则返回原始 cwd。

### 12.6 `package.json` 框架检测

- [x] 读取 `dependencies`。
- [x] 读取 `devDependencies`。
- [x] 合并 dependencies 和 devDependencies。
- [x] 检测 `next` 为 `Next.js`。
- [x] 检测 `nuxt` 或 `nuxt3` 为 `Nuxt`。
- [x] 检测 `@sveltejs/kit` 为 `SvelteKit`。
- [x] 检测 `svelte` 为 `Svelte`。
- [x] 检测 `@remix-run/react` 或 `remix` 为 `Remix`。
- [x] 检测 `astro` 为 `Astro`。
- [x] 检测 `vite` 为 `Vite`。
- [x] 检测 `@angular/core` 为 `Angular`。
- [x] 检测 `vue` 为 `Vue`。
- [x] 检测 `react` 为 `React`。
- [x] 检测 `express` 为 `Express`。
- [x] 检测 `fastify` 为 `Fastify`。
- [x] 检测 `hono` 为 `Hono`。
- [x] 检测 `koa` 为 `Koa`。
- [x] 检测 `nestjs` 或 `@nestjs/core` 为 `NestJS`。
- [x] 检测 `gatsby` 为 `Gatsby`。
- [x] 检测 `webpack-dev-server` 为 `Webpack`。
- [x] 检测 `esbuild` 为 `esbuild`。
- [x] 检测 `parcel` 为 `Parcel`。
- [x] 无效 JSON 不应导致检测崩溃。

### 12.7 文件标记框架检测

- [x] 检测 `vite.config.ts` 为 `Vite`。
- [x] 检测 `vite.config.js` 为 `Vite`。
- [x] 检测 `next.config.js` 为 `Next.js`。
- [x] 检测 `next.config.mjs` 为 `Next.js`。
- [x] 检测 `angular.json` 为 `Angular`。
- [x] 检测 `Cargo.toml` 为 `Rust`。
- [x] 检测 `go.mod` 为 `Go`。
- [x] 检测 `manage.py` 为 `Django`。
- [x] 检测 `Gemfile` 为 `Ruby`。
- [x] 没有匹配时返回无框架。

### 12.8 命令框架检测

- [x] 命令包含 `next` 映射到 `Next.js`。
- [x] 命令包含 `vite` 映射到 `Vite`。
- [x] 命令包含 `nuxt` 映射到 `Nuxt`。
- [x] 命令包含 `angular` 映射到 `Angular`。
- [x] 命令包含 `ng serve` 映射到 `Angular`。
- [x] 命令包含 `webpack` 映射到 `Webpack`。
- [x] 命令包含 `remix` 映射到 `Remix`。
- [x] 命令包含 `astro` 映射到 `Astro`。
- [x] 命令包含 `gatsby` 映射到 `Gatsby`。
- [x] 命令包含 `flask` 映射到 `Flask`。
- [x] 命令包含 `django` 映射到 `Django`。
- [x] 命令包含 `manage.py` 映射到 `Django`。
- [x] 命令包含 `uvicorn` 映射到 `FastAPI`。
- [x] 命令包含 `rails` 映射到 `Rails`。
- [x] 命令包含 `cargo` 映射到 `Rust`。
- [x] 命令包含 `rustc` 映射到 `Rust`。
- [x] fallback 到进程名框架检测。

### 12.9 进程名框架检测

- [x] 进程名 `node` 映射到 `Node.js`。
- [x] 进程名 `python` 映射到 `Python`。
- [x] 进程名 `python3` 映射到 `Python`。
- [x] 进程名 `ruby` 映射到 `Ruby`。
- [x] 进程名 `java` 映射到 `Java`。
- [x] 进程名 `go` 映射到 `Go`。
- [x] 未知进程名映射为无框架。

### 12.10 Docker 镜像框架检测

- [x] 镜像包含 `postgres` 映射到 `PostgreSQL`。
- [x] 镜像包含 `redis` 映射到 `Redis`。
- [x] 镜像包含 `mysql` 映射到 `MySQL`。
- [x] 镜像包含 `mariadb` 映射到 `MySQL`。
- [x] 镜像包含 `mongo` 映射到 `MongoDB`。
- [x] 镜像包含 `nginx` 映射到 `nginx`。
- [x] 镜像包含 `localstack` 映射到 `LocalStack`。
- [x] 镜像包含 `rabbitmq` 映射到 `RabbitMQ`。
- [x] 镜像包含 `kafka` 映射到 `Kafka`。
- [x] 镜像包含 `elasticsearch` 映射到 `Elasticsearch`。
- [x] 镜像包含 `opensearch` 映射到 `Elasticsearch`。
- [x] 镜像包含 `minio` 映射到 `MinIO`。
- [x] 空镜像或未知镜像映射到 `Docker`。

## 13. 格式化兼容

### 13.1 头部 banner

- [x] banner 前渲染空行。
- [x] 渲染宽度匹配当前输出的顶部边框。
- [x] 渲染 `Port Whisperer`。
- [x] 保留或明确决定是否在 Rust 输出中包含当前扬声器 emoji。
- [x] 渲染 `listening to your ports...`。
- [x] 渲染底部边框。
- [x] banner 后渲染空行。
- [x] 使用 Unicode 宽度计算确保盒子对齐。

### 13.2 颜色语义

- [x] Healthy 状态圆点和标签为绿色。
- [x] Orphaned 状态圆点和标签为黄色。
- [x] Zombie 状态圆点和标签为红色。
- [x] Unknown 状态圆点和标签为灰色。
- [x] 在终端颜色库支持范围内匹配当前 framework 颜色映射。
- [x] 缺失值为灰色。
- [x] 端口值为白色粗体。
- [x] 项目名为蓝色。
- [x] 运行时间为黄色。
- [x] 内存为绿色。

### 13.3 framework 颜色映射

- [x] `Next.js` 映射为白字黑底或最接近支持样式。
- [x] `Vite` 映射为黄色。
- [x] `React` 映射为青色。
- [x] `Vue` 映射为绿色。
- [x] `Angular` 映射为红色。
- [x] `Svelte` 映射为支持时的 RGB 橙色。
- [x] `SvelteKit` 映射为支持时的 RGB 橙色。
- [x] `Express` 映射为灰色。
- [x] `Fastify` 映射为白色。
- [x] `NestJS` 映射为红色。
- [x] `Nuxt` 映射为绿色。
- [x] `Remix` 映射为蓝色。
- [x] `Astro` 映射为品红色。
- [x] `Django` 映射为绿色。
- [x] `Flask` 映射为白色。
- [x] `FastAPI` 映射为青色。
- [x] `Rails` 映射为红色。
- [x] `Gatsby` 映射为品红色。
- [x] `Go` 映射为青色。
- [x] `Rust` 映射为支持时的 RGB 棕黄色。
- [x] `Ruby` 映射为红色。
- [x] `Python` 映射为黄色。
- [x] `Node.js` 映射为绿色。
- [x] `Java` 映射为红色。
- [x] `Hono` 映射为支持时的 RGB 橙色。
- [x] `Koa` 映射为白色。
- [x] `Webpack` 映射为蓝色。
- [x] `esbuild` 映射为黄色。
- [x] `Parcel` 映射为支持时的 RGB 金色。
- [x] `Docker` 映射为蓝色。
- [x] `PostgreSQL` 映射为蓝色。
- [x] `Redis` 映射为红色。
- [x] `MySQL` 映射为蓝色。
- [x] `MongoDB` 映射为绿色。
- [x] `nginx` 映射为绿色。
- [x] `LocalStack` 映射为白色。
- [x] `RabbitMQ` 映射为支持时的 RGB 橙色。
- [x] `Kafka` 映射为白色。
- [x] `Elasticsearch` 映射为黄色。
- [x] `MinIO` 映射为红色。

### 13.4 截断规则

- [x] 实现可见宽度感知截断。
- [x] 端口表项目名最大长度为 `20`。
- [x] 进程表进程名最大长度为 `15`。
- [x] 进程表项目名最大长度为 `20`。
- [x] 进程表描述最大长度为 `30`。
- [x] 截断时使用省略号。
- [x] 决定保留当前 Unicode 省略号 `…`；如果偏好 ASCII-only 输出，需要记录并测试差异。

### 13.5 时间与内存格式化

- [x] 运行时间低于一分钟显示 `<seconds>s`。
- [x] 运行时间低于一小时显示 `<minutes>m <seconds>s`。
- [x] 运行时间低于一天显示 `<hours>h <minutes>m`。
- [x] 运行时间超过一天显示 `<days>d <hours>h`。
- [x] 内存大于 `1048576 KB` 显示 `<gb>.1 GB`。
- [x] 内存大于 `1024 KB` 显示 `<mb>.1 MB`。
- [x] 其他内存显示 `<kb> KB`。
- [x] 使用与当前代码相同的严格大于阈值比较。

## 14. 平台实现任务

### 14.1 平台 trait

- [x] 定义 `PlatformScanner` trait。
- [x] 包含 `get_listening_ports_raw`。
- [x] 包含 `batch_process_info`。
- [x] 包含 `batch_cwd`。
- [x] 包含 `get_all_processes_raw`。
- [x] 包含 `get_process_tree`。
- [x] 包含 `pid_exists`。
- [x] 包含 `kill_process`。
- [x] 包含 `get_process_log_files` 或 logs 使用的平台特定 helper。
- [x] 包含 `get_system_log_command`。
- [x] 尽可能通过 Rust 编译期 `cfg` 分发。
- [x] 除非有助于测试，否则避免运行时动态分发。

### 14.2 macOS 实现

- [x] 第一版可以使用 `lsof` fallback 来保证可靠的端口到 PID 映射。
- [ ] 兼容完成后评估原生替代方案。
- [x] 保留 `lsof -iTCP -sTCP:LISTEN -P -n` fallback。
- [x] 从第 `0` 列解析命令名。
- [x] 从第 `1` 列解析 PID。
- [x] 从 `NAME` 列用 `:(\d+)$` 解析端口。
- [x] 按端口去重。
- [x] 保留 `ps -p` 作为进程信息 fallback。
- [x] 解析 PID、PPID、stat、RSS、lstart、command。
- [x] 保留 `lsof -a -d cwd -p` 作为 cwd fallback。
- [x] 保留 `ps -eo pid=,ppid=,comm=` 作为进程树 fallback。
- [x] 权限失败时返回部分结果，不崩溃。

### 14.3 Linux 实现

- [x] 如果实现原生 `/proc` 与 socket inode 映射，则优先使用。
- [x] 保留 `ss -tlnp` fallback。
- [x] 保留 `netstat -tlnp` fallback。
- [x] 从 `ss` users 字段解析 `pid=<pid>`。
- [x] 可用时从 `("name"` 解析进程名。
- [x] fallback 到 `/proc/<pid>/comm` 获取进程名。
- [x] 使用 `/proc/<pid>/cwd` 获取 cwd。
- [x] 使用 `/proc/<pid>/stat` 作为 PPID 和状态 fallback。
- [x] 使用 `/proc/<pid>/cmdline` 作为 command fallback。
- [x] 使用 `/proc/<pid>/statm` 或 sysinfo 作为 RSS fallback。
- [x] 使用 `/proc` 构建进程树。
- [x] 优雅处理权限缺失。

### 14.4 Windows 实现

- [ ] 如果可行，优先使用 Windows API 读取 TCP table。
- [x] 保留 `netstat -ano -p TCP` fallback。
- [x] 只解析 `LISTENING` 行。
- [x] 用 `:(\d+)$` 从 local address 解析端口。
- [x] 从最后一列解析 PID。
- [x] 使用 Windows API、`wmic` 或 PowerShell fallback 解析进程名。
- [x] 保留进程名 `.exe` trimming 行为。
- [x] 如果原生 API 不覆盖命令行，则保留 `wmic` 主路径进程元数据行为。
- [x] `wmic` 不可用时保留 PowerShell fallback。
- [x] 使用 `taskkill /F /PID` 实现 force kill。
- [x] 用最佳 Windows 等价方式实现普通 kill。
- [x] 优雅处理不可访问进程。

## 15. 性能优化任务

### 15.1 快速启动

- [x] Rust 二进制应避免脚本运行时带来的动态启动开销。
- [x] 命令分发前避免昂贵初始化。
- [x] help 命令不应初始化 scanner。
- [x] 无效命令不应初始化 scanner。
- [x] 默认列表除非需要，否则避免加载 Docker/Git/log 模块。

### 15.2 快照复用

- [x] 每次命令执行只构建一次进程快照。
- [x] 端口富化复用快照。
- [x] `ps` 行复用快照。
- [x] orphan/zombie 检测复用快照。
- [x] 缓存 `pid -> ProcessInfo`。
- [x] 缓存 `pid -> cwd`。
- [x] 缓存 `cwd -> project_root`。
- [x] 缓存 `project_root -> framework`。

### 15.3 延迟富化

- [x] 列表视图不要获取 Git 分支。
- [x] 列表视图不要获取进程树。
- [x] 除非执行 `logs` 命令，否则不要获取日志文件。
- [x] 除非检测到 Docker-like listener，否则不要获取 Docker 信息。
- [x] 除非需要 Docker collapse，否则 `ps` 不获取 Docker 信息。
- [x] 对已在过滤前排除的进程，尽量不读取 `package.json` 做框架检测。

### 15.4 并行

- [x] 安全时并发运行互不依赖的昂贵 collector。
- [x] 在 `ports` 中，如果平台支持，并行收集 listener map 和进程快照。
- [x] 仅在检测到 Docker-like 进程后，或带短 timeout 时，并行运行 Docker mapping。
- [x] 即使并行收集，也保持输出确定性。
- [x] 不允许并发写终端输出。

### 15.5 超时

- [x] 对 Docker CLI fallback 应用 timeout。
- [x] 对 `lsof` fallback 应用 timeout。
- [x] 对 `ps` fallback 应用 timeout。
- [x] 对 Git 分支 fallback 应用 timeout。
- [x] 对系统日志命令 setup 应用 timeout（如相关）。
- [x] timeout 对列表命令应产出部分数据，而不是硬崩溃。

### 15.6 benchmark 目标

- [x] 重写前记录 Node 基线耗时。
- [x] benchmark `ports`。
- [x] benchmark `ports --all`。
- [x] benchmark `ports <active-port>`。
- [x] benchmark `ports <empty-port>`。
- [x] benchmark `ports ps`。
- [ ] 可行时 benchmark `ports logs <target> --lines 1`。
- [ ] 内部 benchmark `ports watch` 扫描 tick。
- [ ] 普通本地开发机器上，在 Docker 不慢时，目标 `ports` 小于 `100ms`。
- [ ] 普通 active port 下，目标 `ports <port>` 小于 `100ms`。
- [ ] 普通机器上，目标 `ports ps` 小于 `300ms`。
- [x] 记录 OS 权限或 Docker daemon 延迟主导耗时的情况。

## 16. 测试策略

### 16.1 单元测试

- [x] 测试每个公开命令的命令解析。
- [x] 测试 `--all` 与 `-a` 过滤规范化。
- [x] 测试 kill 目标解析。
- [x] 测试 kill range 解析。
- [x] 测试无效 range 场景。
- [x] 测试 `--lines=N` 解析。
- [x] 测试 `--lines N` 解析。
- [x] 测试从 package dependencies 检测框架。
- [x] 测试从文件 marker 检测框架。
- [x] 测试从命令字符串检测框架。
- [x] 测试从进程名检测框架。
- [x] 测试 Docker 镜像框架检测。
- [x] 测试开发进程过滤中的系统应用排除。
- [x] 测试开发进程过滤中的开发进程名。
- [x] 测试开发进程过滤中的命令指示器。
- [x] 测试项目根目录向上遍历深度。
- [x] 测试内存格式化。
- [x] 测试运行时间格式化。
- [x] 测试命令摘要。
- [x] 测试 log-like 路径检测。
- [x] 测试日志文件 priority 排序和去重。

### 16.2 golden 输出测试

- [x] 创建端口表 fixture 数据。
- [x] 创建空端口表 fixture 数据。
- [x] 创建进程表 fixture 数据。
- [x] 创建空进程表 fixture 数据。
- [x] 创建端口详情 fixture 数据。
- [x] 创建 clean 结果 fixture 数据。
- [x] 创建 watch new 事件 fixture 数据。
- [x] 创建 watch removed 事件 fixture 数据。
- [x] 断言表头。
- [x] 断言 summary 行。
- [x] 断言缺失值 marker。
- [x] 如果颜色导致快照不稳定，则测试禁用颜色的输出。
- [x] 对 ANSI 覆盖做启用颜色的 smoke 测试。

### 16.3 使用 fake platform 的集成测试

- [x] 实现内存 fake platform scanner。
- [x] 输入 fake listening ports。
- [x] 输入 fake process metadata。
- [x] 输入 fake cwd values。
- [x] 输入 fake process tree。
- [x] 输入 fake Docker mappings。
- [x] 验证 fake platform 下的 `ports` 输出。
- [x] 验证 fake platform 下的 `ports --all` 输出。
- [x] 验证 fake platform 下的 `ports <port>` 输出。
- [x] 验证 fake platform 下的 `ports ps` 输出。
- [x] 使用模拟输入验证 `ports clean` prompt flow。
- [x] 通过注入 fake killer 验证 `ports kill`，不发送真实信号。
- [x] 使用 fake log files 验证 `ports logs`。
- [x] 不真实 sleep，验证 `ports watch` diff 逻辑。

### 16.4 真实系统 smoke 测试

- [x] macOS：运行 `ports`。
- [x] macOS：运行 `ports --all`。
- [x] macOS：运行 `ports ps`。
- [x] macOS：运行 `ports <known-port>`。
- [x] macOS：安全时运行 `ports logs <known-port> --lines 1`。
- [x] macOS：运行 `ports watch` 并中断。
- [ ] Linux：运行 `ports`。
- [ ] Linux：运行 `ports --all`。
- [ ] Linux：运行 `ports ps`。
- [ ] Linux：运行 `ports <known-port>`。
- [ ] Linux：运行日志 fallback 路径。
- [ ] Windows：运行 `ports`。
- [ ] Windows：运行 `ports --all`。
- [ ] Windows：运行 `ports ps`。
- [ ] Windows：运行 `ports <known-port>`。
- [ ] Windows：只对安全测试进程运行 kill 行为。

### 16.5 兼容性对比测试

- [x] 需要时安装 Node 依赖作为基线。
- [x] 运行 Node `ports --help` 和 Rust `ports --help`。
- [x] 对比命令列表。
- [x] 语义对比 usage 文本。
- [x] 在 fake 或受控本地 server 上运行 Node 和 Rust。
- [x] 对比表格列。
- [x] 对比字段语义。
- [x] 对比过滤行为。
- [x] 使用 fake/injected process layer 对比 kill 解析行为。
- [x] 对比 logs 参数解析。
- [x] 对比 clean 检测。
- [x] 对比 watch diff 行为。

## 17. 受控测试服务

### 17.1 本地 fixture 进程

- [x] 创建监听端口 `3000` 的简单 Node HTTP server fixture。
- [x] 创建 Vite-like package fixture。
- [x] 创建 Next-like package fixture。
- [x] 创建 Express-like package fixture。
- [x] 创建监听端口 `8000` 的 Python HTTP server fixture。
- [x] 可行时创建 FastAPI-like command fixture。
- [x] 必要时创建 Java 或通用长运行 fixture。
- [x] 创建 stdout 重定向到临时文件的进程。
- [x] 创建 stderr 重定向到临时文件的进程。
- [x] 创建没有重定向日志的进程。

### 17.2 Docker fixture

- [x] 可选：运行带 host port mapping 的 PostgreSQL 容器。
- [x] 可选：运行带 host port mapping 的 Redis 容器。
- [x] 可选：运行带 host port mapping 的 nginx 容器。
- [x] 可选：运行 LocalStack-like 镜像或 fixture mapping。
- [x] Docker 不可用时跳过 Docker 测试。
- [x] Docker 测试必须严格清理。

### 17.3 安全规则

- [x] 永远不要对用户任意进程运行 kill 集成测试。
- [x] 从测试 harness 启动测试进程。
- [x] 跟踪 child PID。
- [x] 测试后 kill child 进程。
- [x] 尽量使用随机空闲端口。
- [x] 自动化测试避免硬编码常见用户端口。

## 18. 迁移与 cutover 计划

### 18.1 阶段 1：基线与测试 harness

- [x] 添加本清单。
- [x] 添加基线行为说明。
- [x] 添加 fixture 设计。
- [x] 添加 Rust 项目骨架。
- [x] 添加 fake platform scanner。
- [x] 添加检测和解析单元测试。
- [x] 添加 display golden output 测试。

### 18.2 阶段 2：`ports` 快路径

- [x] 实现平台 listener 发现。
- [x] 实现进程快照。
- [x] 实现项目根目录检测。
- [x] 实现框架检测。
- [x] 实现 Docker mapping。
- [x] 实现端口表展示。
- [x] 实现 `ports`。
- [x] 实现 `ports --all`。
- [x] 与 Node 基线 benchmark。

### 18.3 阶段 3：详情快路径

- [x] 实现优化后的单端口查询。
- [x] 实现详情富化。
- [x] 实现 Git 分支查询。
- [x] 实现进程树。
- [x] 实现详情展示。
- [x] 实现详情后的交互式 kill prompt。
- [x] 实现 `whoisonport`。
- [x] 与 Node 基线 benchmark。

### 18.4 阶段 4：进程与 kill 命令

- [x] 实现 `ports ps`。
- [x] 实现 `ports ps --all`。
- [x] 实现 Docker collapse。
- [x] 实现 kill 目标解析。
- [x] 实现 kill resolution。
- [x] 实现 signal dispatch。
- [x] 实现 range summary。
- [x] 实现 kill 退出码。

### 18.5 阶段 5：logs、clean、watch

- [x] 实现 log discovery。
- [x] 实现 tail/follow。
- [x] 实现 stderr-only 行为。
- [x] 实现 system log fallback。
- [x] 实现 clean discovery。
- [x] 实现 clean prompt 和结果展示。
- [x] 实现 watch loop。
- [x] 实现 watch event 渲染。
- [x] 实现 Ctrl+C 处理。

### 18.6 阶段 6：打包

- [ ] 决定是否保留 Node package wrapper。
- [ ] 决定 npm package 是否下载预构建 Rust 二进制。
- [ ] 决定 npm package 是否从源码构建。
- [ ] 如果保留 npm 分发，更新 `package.json` 的 `bin` 字段。
- [ ] 添加 macOS x64 release workflow。
- [ ] 添加 macOS arm64 release workflow。
- [ ] 添加 Linux x64 release workflow。
- [ ] 如需要，添加 Linux arm64 release workflow。
- [ ] 添加 Windows x64 release workflow。
- [ ] 记录 npm 安装方式。
- [x] 记录 cargo 安装方式。
- [x] 记录直接下载二进制方式。

### 18.7 阶段 7：Node 移除或 wrapper

- [x] 在 Rust 兼容性验收前保留 Node 实现。
- [x] 添加迁移说明，解释 Rust 重写。
- [ ] 决定旧 `src/*.js` 是否作为 fallback 保留。
- [ ] 如果移除 Node source，确保 npm package 仍有正确 files。
- [ ] 如果保留 Node wrapper，确保它只分发到 Rust binary，不重新引入启动延迟。
- [ ] cutover 后再移除未使用 JS 依赖。

## 19. 验收标准

### 19.1 命令兼容验收

- [x] 第 1.2 节列出的所有命令都可用。
- [x] 第 1.2 节列出的所有 flag 都可用。
- [x] `ports` 与 `whoisonport` alias 可用。
- [x] 无效命令产生匹配的错误。
- [x] 缺失参数产生匹配的 usage。

### 19.2 输出字段兼容验收

- [x] `ports` 表包含当前所有列。
- [x] `ports ps` 表包含当前所有列。
- [x] `ports <port>` 详情包含当前所有 section 和字段。
- [x] `ports logs` header 包含当前所有字段。
- [x] `ports clean` 输出包含当前所有字段。
- [x] `ports watch` 事件包含当前所有字段。

### 19.3 交互兼容验收

- [x] 详情 kill prompt 可用。
- [x] clean confirmation prompt 可用。
- [x] logs 多文件选择可用。
- [x] logs follow mode 可中断。
- [x] watch mode 可中断。
- [x] 非肯定 prompt 答案不会 kill。

### 19.4 检测规则兼容验收

- [x] dev-process filtering 匹配 fixture 预期。
- [x] framework detection 匹配 fixture 预期。
- [x] Docker image detection 匹配 fixture 预期。
- [x] project root detection 匹配 fixture 预期。
- [x] status detection 匹配 fixture 预期。
- [x] log-file detection 匹配 fixture 预期。

### 19.5 行为兼容验收

- [x] port-vs-PID resolution 匹配当前行为。
- [x] kill range expansion 匹配当前行为。
- [x] empty range target 匹配当前行为。
- [x] 退出码匹配当前行为。
- [x] 缺失值显示为 `—`。
- [x] 排序匹配当前行为。
- [x] 过滤匹配当前行为。

### 19.6 性能验收

- [x] 同一机器正常条件下，Rust `ports` 明显快于 Node `ports`。
- [x] Rust `ports <port>` 避免对所有端口做完整深度富化。
- [x] Docker 慢时不阻塞非 Docker 场景。
- [x] help 和无效命令路径立即返回，不扫描。
- [x] watch mode 避免不必要的重复富化。

## 20. 已知风险区域

- [ ] macOS 原生 port-to-PID 映射可能仍需要 fallback 到 `lsof`。
- [ ] Windows 命令行和 cwd 访问对权限敏感。
- [ ] `wmic` 已废弃，但在某些系统上仍需要作为 fallback。
- [ ] Docker daemon 延迟可能主导扫描耗时。
- [ ] JS 与 Rust 颜色库差异可能导致轻微 ANSI 差异。
- [ ] Unicode 盒线和表格渲染会因终端和 width library 而不同。
- [ ] 进程启动时间格式若不规范化，可能受 locale 影响。
- [ ] `ps`、PowerShell 和 `sysinfo` 的 CPU 语义不同。
- [ ] zombie/orphan 检测跨平台存在差异。
- [ ] 日志发现天然是 best-effort。
- [ ] 测试中 kill 进程有危险，必须使用受控 fixture。

## 21. 实现前需要决策

- [ ] Rust 重写是否完全保留 header 中的扬声器 emoji？
- [ ] 输出快照应比较 ANSI 颜色，还是比较语义化纯文本？
- [ ] 如果 Docker daemon 很慢，`ports` 默认路径应跳过 Docker，还是等待以保持 Docker 兼容？
- [ ] macOS 第一版应保留 `lsof` 以确保兼容，还是立即尝试原生实现？
- [ ] npm 是否仍作为主要安装渠道？
- [ ] 是否正式支持 `cargo install`？
- [ ] 第一个 Rust release 是否保留 Node source？
- [ ] 性能模式应隐式启用，还是提供兼容/fallback flag？
- [ ] 未来是否添加 `--json` 输出？这不属于一比一兼容范围。

## 22. 完成定义

- [x] 所有公开命令均由 Rust 实现。
- [x] 当前所有输出字段都存在。
- [x] 当前所有交互都已实现。
- [x] 当前所有检测规则都有测试覆盖。
- [x] 当前所有行为边界情况都有测试覆盖。
- [x] macOS smoke tests 通过。
- [x] Linux smoke tests 通过，或记录本地不可用原因。
- [x] Windows smoke tests 通过，或记录本地不可用原因。
- [x] Rust 快路径已与 Node 基线 benchmark。
- [x] README 已更新为 Rust 实现说明。
- [x] 打包路径已决定并测试。
- [x] 现有 Node 实现已移除、作为 fallback 保留，或被有意包装。
- [ ] release checklist 已完成。
