# CurrentScriptContext 构建逻辑重修设计

## 背景

`CurrentScriptContext` 同时服务两类判断：

- 当前正在运行脚本相关的 input/output indices；
- `Message.Action` target 在完整交易范围内是否存在。

这两类数据范围不同。当前脚本 indices 只关心当前 lock/type 脚本命中的 cells；action target 存在性检查必须覆盖完整交易中的 input lock、input type、output type script hashes。

## 目标

让 `CurrentScriptContext::from_reader` 成为唯一生产构建入口，并让构建流程保持直观：

- 初始化 `CurrentScriptContext`；
- 一轮扫描 transaction inputs；
- 一轮扫描 transaction outputs；
- 扫描时同时维护当前脚本 indices 和完整交易 script hash 集合。

不引入与 `CurrentScriptContext` 字段重复的中间结构，不在 `SyscallTxReader` 增加 script hash 测试 fixture，不保留测试专用 target hash 状态。

## 数据结构

`CurrentScriptContext` 保持三个核心字段：

- `current_script: CurrentScript`
- `indices: CurrentScriptIndices`
- `script_hashes: ScriptHashes`

`CurrentScriptIndices` 使用 enum 表达当前脚本类型：

- `Lock { input_indices: Vec<usize> }`
- `Type { input_indices: Vec<usize>, output_indices: Vec<usize> }`

`ScriptHashes` 使用三个 `BTreeSet<[u8; 32]>` 保存完整交易范围内可作为 action target 的 script hashes：

- `input_locks`
- `input_types`
- `output_types`

## 构建流程

`CurrentScriptContext::from_reader(reader, current_script)` 直接执行扫描。

Input 扫描：

1. 调用 `reader.input_lock_hash(index)`。
2. 插入 `script_hashes.input_locks`。
3. 如果 `current_script` 是相同 hash 的 `InputLock`，记录 input index。
4. 调用 `reader.input_type_hash(index)`。
5. 如果存在 type hash，插入 `script_hashes.input_types`。
6. 如果 `current_script` 是相同 hash 的 `Type`，记录 input index。

Output 扫描：

1. 调用 `reader.output_type_hash(index)`。
2. 如果存在 type hash，插入 `script_hashes.output_types`。
3. 如果 `current_script` 是相同 hash 的 `Type`，记录 output index。

`validate_message_targets(message)` 不接收 reader，只遍历 message actions，并通过 `self.script_hashes.contains(role, hash)` 判断 target 是否存在。

## 测试策略

`context.rs` 的单元测试可以在同一个 `#[cfg(test)] mod tests` 内使用普通 test helper 直接构造 `CurrentScriptContext` 私有字段，覆盖：

- lock context 只记录当前 lock 命中的 input indices；
- type context 只记录当前 type 命中的 input/output indices；
- `script_hashes` 覆盖完整交易范围并支持 target validation；
- 缺失或未知 role 的 action target 被拒绝。

`engine.rs` 中目前依赖 `CurrentScriptContext::from_script_hashes` 的测试应迁移或改写，避免为了跨模块测试保留生产可见构造入口。

## 验证

实现后运行：

- `cargo fmt --check`
- `cargo test -p cobuild-core`
- `cargo test --test contract_template_layout`
- `git diff --check`
- `cargo test`

并检查禁用残留：

```sh
rg "ScriptHashScan|target_hashes_for_tests|TargetHashesForTests|with_cell_script_hashes_for_tests|CellScriptHashesForTests|cell_script_hashes" crates/cobuild-core/src tests
```

该命令不应在生产代码或测试中发现残留。`SyscallTxReader` 不应包含 script hash 测试 fixture。
