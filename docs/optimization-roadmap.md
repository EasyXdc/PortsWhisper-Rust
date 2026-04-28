# Optimization Roadmap

## Goal

在 `v0.1.0` 稳定版基础上，系统性地提升 `port-whisperer-rust` 在**功能稳定性、终端输出美观性、功能完备性**三个维度的质量，明确下一阶段的优化优先级与落地顺序，避免散点式改动。

## Scope

本文档覆盖：

- 现状三维度代码级评估（带 `file:line` 定位）
- 阶段化优先级路线图与验收标准
- 显式声明的 Non-Goals，避免 scope 膨胀
- 需要后续讨论再决策的开放项

本文档不覆盖：

- 具体实现 PR 的代码 diff
- release 流程调整（见 `release-checklist.md`）
- 商业化 / 品牌 / 网站相关工作

## Repo Snapshot

- 核心 CLI：`src/` 约 6400 行 Rust，两个二进制 `ports` / `whoisonport`
- 跨平台：`src/platform/{macos,linux,windows}.rs` + `src/platform/mod.rs`（871 行为最大单模块之一）
- 打包侧：Node.js `scripts/*.js` + `bin/*.js`，npm 包 `ports-rs`
- 发布：GitHub Actions `release.yml` 由 `v*` tag 触发多平台构建
- CI：`.github/workflows/ci.yml` 执行 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`npm test`、`npm pack` 与 postinstall skip 验证
- 外部依赖：仅 `unicode-width`，几乎零运行时依赖

## Assessment

### 功能稳定性

| # | 问题 | 位置 | 影响 | 建议方向 |
|---|---|---|---|---|
| S1 | `run_output` 的 `timeout_hint` 参数被显式丢弃 | `src/util.rs:13` | `lsof` / `netstat` / `ps` 阻塞时整个 CLI 卡死 | 引入真超时：子线程 + `Duration::try_recv` 或 `wait-timeout` crate |
| S2 | SIGINT 用裸 `signal(2)` syscall | `src/watch.rs:202-216` | 多线程下行为未定义；Windows 分支为空，`Ctrl+C` 不响应 | 替换为 `ctrlc` crate，统一跨平台 |
| S3 | 错误类型仅 `Success` / `Failure` 两态 | `src/error.rs:1-14` | 权限不足、命令缺失、解析失败都被 `Option::None` 吞掉，用户只能看到"—" | 引入 `thiserror` 分层错误；`--verbose` 模式下打印原始错误 |
| S4 | `ps lstart` 解析写死英文月份 | `src/util.rs:78-91` | 中/德/日 locale 系统 `ps` 输出全部 fallback 到空 uptime | 调用时强制 `LANG=C ps ...`，或 Linux 直读 `/proc/[pid]/stat` |
| S5 | Linux 日志发现默认走 `lsof` | `src/logs.rs:191-200` | 容器 / 精简镜像里常缺 `lsof`，功能直接不可用 | 优先读 `/proc/{pid}/fd/*` 软链，`lsof` 作为 fallback |
| S6 | 参数解析为手工字符串切片 | `src/cli.rs:104-125` | `CliCommand::Logs(filtered_args)` 把子命令名 `"logs"` 也塞进去，`logs.rs:170` 用 `.skip(1)` 硬修 | 引入 `clap`（derive API），统一子命令参数；向后兼容现有 CLI 语法 |
| S7 | CI 未跑 `cargo clippy` / `cargo fmt --check` | `.github/workflows/ci.yml:26-27` | 代码中已有两处 `unsafe`，lint 红线缺失 | CI 先补 `cargo clippy -- -D warnings` 与 `cargo fmt --check`，把格式和明显 lint 问题前置到合并前 |
| S8 | Windows 端依赖 `wmic` | `src/platform/windows.rs` + `platform/mod.rs` 调用点 | Windows 11 起 `wmic` 已弃用/默认不装，未来版本将缺失 | 迁移到 `Get-NetTCPConnection` / `Get-Process` / `Get-CimInstance` |

**亮点**：`util.rs:124-154` 对 CJK 宽字符的 `truncate_visible` / `visible_width` 实现正确，并在 `display.rs:706-715` 有"表表表A" width 7 的测试覆盖，这部分质量高，后续不动。

### 终端输出美观性

| # | 问题 | 位置 | 影响 | 建议方向 |
|---|---|---|---|---|
| V1 | `BOX_INNER_WIDTH = 37` 硬编码 | `src/display.rs:5` | 非响应式，窄终端溢出、宽终端浪费 | `terminal_size` crate 动态探测，设上下限 |
| V2 | header 在每条命令执行时都会打印 | `src/display.rs:26-47` + 各 `display_*` 入口 | `ports 3000` / `ports kill` 这种单次命令也打完整 banner，噪音大 | 新增 `--quiet` 或按命令类型按需省略；默认在非 TTY 时不打印装饰 |
| V3 | Framework 颜色映射用长 `match` 硬编码 | `src/style.rs:97-112` | 新增/修改框架需同时改 `framework.rs` + `style.rs`；truecolor 与 16 色混用（Next.js 16 色，Svelte truecolor） | 合并到 `framework.rs` 的 metadata 表；颜色、icon、匹配规则单表维护 |
| V4 | Emoji / 特殊符号散落无 ASCII fallback | `display.rs` / `cli.rs:141` / `logs.rs:62` / `watch.rs` 等 | 🔊 / ✓ / ✕ / ▸ / • / — 混用；Windows cmd.exe、CI 日志、低版本 terminfo 易乱码 | 引入 `--ascii` 开关，`TERM=dumb` 或非 UTF-8 locale 自动切换 |
| V5 | CPU 阈值 5% / 25% 硬编码 | `src/display.rs:172-178` | 无法适配高核机器或用户偏好 | 阈值走 config，或改为相对比例 |
| V6 | `watch` 是 append-only 日志 | `src/watch.rs:40-43` | 更像 `tail -f`，没有 dashboard 感 | 后续 TUI 方案可彻底重做（见 F7） |
| V7 | 无任何 spinner / 扫描反馈 | 所有入口 | 冷启动 ~200ms 无视觉反馈 | 仅对 >150ms 的命令加 spinner，避免短命令抖动 |
| V8 | 无主题 / 低对比度风险 | `src/style.rs` 全量 `gray` | 浅色 terminal 下 `gray` 几乎不可见 | 最低对比度保底；后期考虑 `NO_COLOR` 之外加 `PORTS_THEME=light/dark` |

### 功能新增机会

| # | 新增项 | 理由 | 实现思路 |
|---|---|---|---|
| F1 | `--json` 输出（`ports` / `ports ps` / `ports <port>`） | 最高 ROI：解锁 CI、脚本、IDE 插件消费 | `serde_json` 可选 feature，或手写（保持零依赖） |
| F2 | 查询过滤：`--framework`、`--pid`、`--project`、`--port-range` | 当前只有 `--all` / 默认两档，粒度太粗 | 统一在 `cli.rs` 的参数层 |
| F3 | 范围查询：`ports 3000-3010` | 目前只在 `kill` 支持；查询常用场景 | 复用 `kill.rs` 已有的 range 解析 |
| F4 | `ports open <port>` | 前端开发高频需求 | `open` / `xdg-open` / `start` 分平台分发 |
| F5 | `ports logs` 增强 `--grep` / `--since` | follow 模式下尤其有用 | 流式过滤，避免读入全文件 |
| F6 | Shell completion：`ports completion bash\|zsh\|fish` | CLI 工具标配 | 接入 `clap_complete`（依赖 F2 迁移到 clap） |
| F7 | TUI 模式：`ports watch --tui`（探索项） | 体验上限高，但工作量和依赖拖尾也最大 | `ratatui` + `crossterm`，建议在主路线稳定后再单独立项 |
| F8 | 配置文件 `~/.config/ports-rs/config.toml` | 默认 `--all`、忽略进程、自定义框架色 | 轻量 `toml` crate |
| F9 | `ports check 3000 5173 8080` | 端口冲突预检：dev 启动脚本可集成 | 直接复用 scanner，返回 exit code 区分 |
| F10 | 自定义 kill 信号 `--signal SIGHUP` | `-f` 只覆盖 SIGKILL，其他信号无法触达 | `kill.rs` 增加 signal 参数层 |
| F11 | `ports history`（后置） | "几小时前谁占过这个端口" 真实需求 | 需要 daemon；放到 v0.3+ 讨论 |

## Roadmap

下列阶段按**价值 / 工作量比**与**回归面可控性**共同排序；主线阶段以串行为主，避免在基础能力尚未收敛时并行推进高风险大项。每个阶段结束前需通过对应验收标准。

**建议版本映射**（各 Phase kickoff 前可微调）：

| Phase | 目标版本 |
|---|---|
| Phase 1 | v0.2.0 |
| Phase 2 | v0.3.0 |
| Phase 3 | v0.4.0 |
| Phase 4 | v0.5.0 |

### Phase 1 — 最小稳定性闭环

**目标**：优先修掉会卡死、会吞错、会漏检的问题，建立性能基线与最小可程序化消费能力。该阶段不引入参数层重构，也不做视觉改造。

**范围**：

- S1 `run_output` 真超时
- S2 SIGINT 统一跨平台（`ctrlc` crate）—— 原 Phase 3 项，作为稳定性问题前移
- S3 分层错误 + `--verbose` 原始错误
- S7 CI 加 clippy / fmt
- F1 `--json` 输出
- 性能基线：用 `hyperfine` 采集 `ports` / `ports ps` / `ports <port>` 冷启动与稳态耗时，作为 Phase 2 `clap` 决策与 Phase 4 spinner 阈值的客观依据

**验收标准**：

- 通过测试桩或专用挂起命令模拟 `lsof` / `ps` / `netstat` 阻塞场景，CLI 在 ≤ 3s 内返回并降级
- 权限不足、命令缺失、解析失败在默认模式下给出可区分的用户提示，`--verbose` 下可见原始错误
- Unix / Windows 下 `ports watch` 在 `Ctrl+C` 下 ≤ 1s 退出且不留残余终端状态
- CI workflow 包含 `cargo clippy -- -D warnings` 与 `cargo fmt --check`
- `ports --json` / `ports ps --json` / `ports 3000 --json` 输出通过 `jq .` 校验
- 现有非 JSON 命令行为与 `v0.1.0` 保持一致（`tests/` 现有用例全绿）
- `docs/benchmarks/` 下落库一份 hyperfine 基线（至少覆盖冷启动 + 三条最常用命令）

### Phase 2 — CLI 参数层与可组合能力

**目标**：在不引入平台改造的前提下，先把 CLI 契约整理清楚，为过滤、补全和后续扩展建立稳定参数层。

**前置决策**：在 Phase 2 kickoff 前结合 Phase 1 落库的体积/编译耗时基线，给出 Open Questions 中"是否引入 `clap`"的结论；下方 `clap` 相关条款依赖此决策。

**范围**：

- S6 参数层基础化：决策采用 `clap` 则完成迁移；决策自建则落地参数注册表（两条路径均需满足下方验收条款）
- F2 查询过滤：`--framework`、`--pid`、`--project`、`--port-range`
- F3 范围查询扩展到列表命令
- F6 shell completion（自建参数层路径下需单独实现 completion 生成器）

**验收标准**：

- 当前已有 CLI 语法保持向后兼容，新增参数帮助文本清晰且无歧义
- `ports 3000-3010`、`ports --framework nextjs`、`ports --pid 1234` 等典型组合可被稳定解析
- `ports completion bash|zsh|fish` 能生成可用补全脚本
- 若采用 `clap`：二进制体积与编译耗时相对 Phase 1 基线的增长在可接受范围内并有记录

### Phase 3 — 平台完整性与高频能力补齐

**目标**：补齐平台短板，优先解决 Windows / Linux 上的真实缺口，再补常用但不改变展示层架构的命令能力。

**前置**：扩展 `.github/workflows/ci.yml` 到 `ubuntu-latest` / `macos-latest` / `windows-latest` 三平台 matrix，否则本阶段平台相关验收标准无法自动化保障。

**范围**：

- S8 Windows `wmic` → `Get-NetTCPConnection` 迁移
- S4 `ps` locale 修复（LANG=C 或 `/proc/[pid]/stat`）
- S5 Linux 日志读 `/proc` 优先
- F4 `ports open`
- F5 `ports logs --grep` / `--since`
- F9 `ports check`
- F10 自定义 kill 信号

**验收标准**：

- CI matrix 在 Linux / macOS / Windows 三平台均执行 `cargo test` 通过
- Windows 11 无 `wmic` 环境可完整运行全部命令
- Linux Alpine 容器（无 `lsof`）`ports logs` 正常工作
- 非英文 locale 下 uptime / start time 相关输出仍可解析
- `ports open 3000` 在三平台默认浏览器打开
- `ports check 3000 5173 8080` 可通过 exit code 区分是否存在占用
- `ports kill --signal SIGHUP <port>` 等价路径可正确下发指定信号

### Phase 4 — 终端体验优化

**目标**：在主干行为和平台能力稳定后，再集中处理输出美观性与交互细节，避免 UI 变更掩盖功能回归。

**范围**：

- V1 响应式布局（`terminal_size` 驱动）
- V2 header 按命令类型按需出现 + `--quiet`
- V3 framework metadata 合并
- V4 emoji ASCII fallback 自动探测
- V7 spinner（仅超过阈值命令）
- V8 最低对比度保底

**验收标准**：

- `COLUMNS=40 ports` 与 `COLUMNS=200 ports` 都无错位，截图对比录入 `docs/` 快照
- `TERM=dumb ports` 无任何 ANSI 或非 ASCII 字节输出
- 非 TTY / CI / 重定向输出默认不打印装饰性 header
- spinner 仅在慢命令出现，且不污染管道输出

### Future Exploration

- F7 TUI 模式：单独立项评估，不纳入当前主路线承诺。前置条件是 Phase 1-4 收敛、`watch` 当前语义稳定、并完成依赖体积评估。
- F8 配置文件：在过滤、输出模式、颜色策略都稳定之后再考虑；否则容易把未定策略固化到用户配置中。
- V5 CPU 阈值配置化：依赖 F8，暂不提前承诺。
- V6 `watch` dashboard 化：与 F7 高度耦合，统一放入后续体验专题。

## Non-Goals

显式排除，避免 scope 膨胀：

- **F11 `ports history`**：需常驻 daemon，改动面大，推迟到 v0.3 之后再立项
- **自动 npm publish**：已由 release workflow 的 `publish-npm` job 处理，并通过 `npm-publish` environment 保留人工审批门禁
- **新增平台**（Linux arm64 / Windows arm64）：与本轮优化正交
- **GUI / 菜单栏 App**：定位依然是 CLI，本轮不偏离终端工具边界
- **云端 / 远程端口查询**：越界
- **与 Docker Compose / Kubernetes 的深度集成**：可由下游工具 + F1 JSON 完成

## Open Questions

- **是否引入 `clap`**：会带来编译时间与二进制体积增长。该问题必须在 Phase 2 开始前给出结论，并基于当前 `cargo build --release` 产物大小做基线对比。*备选*：保留手写 parser，`--json` / `--filter` 走自建参数注册表
- **TUI 依赖与发布策略**：`ratatui` + `crossterm` 是明显的依赖拖尾。若后续立项，需先决定是可选 feature 还是单独二进制，再评估是否进入正式 release artifact
- **配置文件 format**：TOML vs JSON vs 无配置（全走 env var）。该决策应晚于过滤参数、ASCII/plain/quiet 等输出策略稳定之后
- **颜色主题识别**：是否需要 `COLORFGBG` / light-dark 探测，还是只先做最低对比度保底；前者不应阻塞 Phase 4

## Change Log

| Date | Change |
|---|---|
| 2026-04-19 | Initial roadmap after v0.1.0 stable release |
| 2026-04-19 | Reordered phases for conservative execution: extracted TUI/config to future exploration, separated CLI contract, platform parity, and terminal UX work |
| 2026-04-19 | Pulled S2 SIGINT into Phase 1 as stability fix; added hyperfine baseline to Phase 1 scope; made Phase 2 `clap` adoption contingent on Open Question decision; promoted CI matrix to Phase 3 prerequisite; added proposed Phase-to-version mapping |
| 2026-04-21 | Completed Phase 3 platform parity and high-frequency commands: Unix `ps` locale hardening, Windows listener migration, Linux `/proc`-first log discovery, CI matrix prerequisite, `ports check`, `ports open`, `ports logs --grep/--since`, and `ports kill --signal` |
| 2026-04-21 | Completed Phase 4 terminal UX work: quiet/ascii global flags, header render config, width-aware layout, ASCII-safe glyph fallback, framework display metadata consolidation, non-interactive header suppression, low-contrast gray fallback, and slow-command spinner gating |
