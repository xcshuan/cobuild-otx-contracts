# Cobuild OTX Lock 与 Cobuild Core 问题修正建议

日期：2026-06-12

## 背景

近期补齐 Cobuild 安全测试覆盖时，新增了多组 `cobuild-otx-lock`、`cobuild-core`、`limit-order-type`、`limit-order-lock` 的单元测试和端到端测试。测试结果没有暴露出必须立即修复的安全漏洞，但暴露了几个容易误解的协议语义和错误码优先级问题。

本文档记录推荐修正项、可选改进项，以及不建议修改的行为。

## 结论

当前实现整体是合理的。尤其是：

- lock coverage 判断以当前 lock group 为单位，而不是以整笔交易所有 inputs 为单位。
- `MissingLockGroupCoverage` 在 `cobuild-core` 内部可达，不是死分支。
- `cobuild-otx-lock` 端到端场景中，某些缺 tx-level signature 的场景会先返回 `InvalidLockGroupWitness`，这是当前 witness carrier 校验顺序导致的可解释行为。
- limit-order 测试合约拒绝 tx-level 与 OTX 同 target 重复 action，是更保守的安全语义。

因此，不建议为了追求某个外部错误码而调整核心执行顺序。推荐的修正重点是文档化、注释化和少量测试辅助整理。

## 固定下来的行为语义

以下语义已经由近期测试固定，后续不应在没有新协议要求的情况下改变。

### Lock coverage 只看当前 lock group

`cobuild-core` 的 lock coverage 判断以当前正在执行的 lock group 为单位。

如果当前脚本是 `Lock A`，则 coverage 判断只检查所有 `Lock A` inputs 是否被 OTX aggregate input range 覆盖。交易中 `Lock B` 或其他 lock 的 inputs 不参与 `Lock A` 的 coverage 判断。

正确通过场景：

```text
inputs:
  0: Lock A，位于 OTX 内
  1: Lock B，位于 OTX 外
  2: Lock B，位于 OTX 外

当前执行 Lock A。
Lock A 已完全位于 OTX 内，因此 Lock A 不需要 tx-level signature。
Lock B 的签名责任由 Lock B 自己的 lock script 执行时判断。
```

正确失败场景：

```text
inputs:
  0: Lock A，位于 OTX 内
  1: Lock B，位置无关
  2: Lock A，位于 OTX 外

当前执行 Lock A。
Lock A 同时出现在 OTX 内和 OTX 外。
如果没有 tx-level signature，则当前 lock group 覆盖不完整。
```

### 错误码优先级

`MissingLockGroupCoverage` 在 `cobuild-core` planning 内可达，用于表达 current lock group 没有被 OTX 或 tx-level signature 完整覆盖。

在完整 `cobuild-otx-lock` 合约执行中，如果当前 lock group 需要 tx-level carrier，但 carrier witness 缺失或形状非法，执行可能更早返回 `InvalidLockGroupWitness`。

这两个错误含义不同：

```text
InvalidLockGroupWitness:
  tx-level carrier/witness 形状错误或缺失。

MissingLockGroupCoverage:
  planning 语义层发现 current lock group 覆盖不完整。
```

当前推荐保留这一错误优先级，不为了固定外部错误码而改变 witness carrier 校验顺序。

### Action origin 唯一性

limit-order 测试合约固定以下行为：

- OTX fill 路径应消费 OTX origin 的 action。
- tx-level wrong-target action noise 不影响 OTX 正确 action。
- tx-level 与 OTX 同 role、同 target、同业务 action 同时存在时，应作为重复相关 action 拒绝。

业务脚本不应猜测用户希望使用哪个 action。相关 action 不唯一时，拒绝比自动忽略其中一个 action 更安全。

### 覆盖清单位置

关键覆盖项通过 fixture-level manifest 固定。Integration test 调用 manifest，而不是直接散落 case count 和 case name 断言。

这样做的目的：

- 防止关键安全 case 被无声删除。
- 让 integration test 文件只负责执行 case。
- 将业务覆盖清单保留在 fixture 所属模块附近。

## 推荐修正项

### 1. 文档化 lock coverage 的判断边界

推荐在 `cobuild-core` 的设计文档或新增行为说明中明确：

```text
lock coverage 判断只针对当前执行的 lock group。
交易中其他 lock group 的 inputs 不影响当前 lock group 的 coverage 判断。
```

示例：

```text
inputs:
  0: Lock A，位于 OTX 内
  1: Lock B，位于 OTX 外
  2: Lock B，位于 OTX 外

当前执行 Lock A。
Lock A 的所有 inputs 都在 OTX 内，因此 Lock A 不需要 tx-level signature。
Lock B 是否需要签名由 Lock B 自己的 lock script 执行时判断。
```

对应失败场景：

```text
inputs:
  0: Lock A，位于 OTX 内
  1: Lock B，位置无关
  2: Lock A，位于 OTX 外

当前执行 Lock A。
Lock A 同时有 OTX 内 input 和 OTX 外 input。
如果没有 tx-level signature，则 current lock group 覆盖不完整。
```

这条语义应作为协议边界固定下来。

### 2. 给 `current_lock_needs_tx_level_signature` 和 `ensure_otx_lock_group_coverage` 补充注释

当前实现的行为正确，但阅读成本较高。建议在两个函数附近补充短注释：

- `current_lock_needs_tx_level_signature` 判断的是当前 lock group 是否有 input 位于 OTX aggregate input range 外。
- `ensure_otx_lock_group_coverage` 是兜底检查：当已经有 OTX signature 但没有 tx-level signature 时，当前 lock group 必须完全位于 OTX aggregate input range 内。
- 该检查不关心 other-lock inputs。

建议注释不应过长，重点说明“current lock group only”。

### 3. 明确错误码优先级

测试显示：

- `MissingLockGroupCoverage` 在 core planning 内可达。
- 但在真实 `cobuild-otx-lock` E2E 中，如果当前 lock group 需要 tx-level carrier，而 carrier witness 缺失或形状非法，可能先返回 `InvalidLockGroupWitness`。

推荐文档化这个优先级：

```text
InvalidLockGroupWitness 是 tx-level carrier/witness 形状错误。
MissingLockGroupCoverage 是 core coverage 语义错误。
当同一交易同时满足两类错误条件时，入口构建 tx-level requirement 可能先触发 InvalidLockGroupWitness。
```

除非协议明确要求外部稳定错误码必须是 `MissingLockGroupCoverage`，否则不建议修改执行顺序。

### 4. 文档化 limit-order action origin 语义

limit-order 测试合约现在体现了以下语义：

- OTX action 是 OTX fill 路径应消费的 action origin。
- tx-level wrong-target action noise 不应影响 OTX 正确 action。
- tx-level 与 OTX 同 role、同 target、同业务动作共存时，应作为 duplicate related action 拒绝。

推荐在测试合约设计文档中补充：

```text
业务脚本只接受唯一相关 action。
同 target 的 tx-level action 与 OTX action 同时存在时，业务脚本不得猜测用户意图，应拒绝。
```

这比自动忽略其中一个 action 更安全。

## 可选改进项

### 1. 整理 core host test helper

新增测试为 `CurrentScriptContext` 增加了 `input_lock_for_tests`。如果后续 `cobuild-core` plan/engine 测试继续增加，可以考虑集中整理 `cfg(test)` helper。

短期不建议重构。当前 helper 范围小、只在测试编译，成本可控。

### 2. 为 coverage checklist 建立 fixture-level manifest

当前集成测试使用 case count 和关键 case name 断言防止覆盖被无声删除。后续如果 case 继续增长，可以把这些断言抽到 fixture-level manifest，例如：

```rust
assert_cobuild_otx_lock_coverage_manifest(&cases);
assert_limit_order_type_coverage_manifest(&cases);
assert_limit_order_lock_coverage_manifest(&cases);
```

这样 integration test 文件会更短，关键覆盖项也更集中。

### 3. 对错误码策略建立专门测试说明

如果后续协议要求某些错误码对外稳定，可以新增一份“错误码优先级矩阵”，明确：

- malformed witness 优先级；
- malformed OTX layout 优先级；
- invalid message target 优先级；
- missing seal / duplicate seal / invalid scope 优先级；
- missing lock group coverage 与 invalid lock group witness 的优先级。

当前还没有必要改变实现。

## 不建议修改项

### 1. 不建议把 lock coverage 改成全交易 input 判断

错误做法：

```text
只要交易中任何 input 位于 OTX 外，就要求当前 lock 提供 tx-level signature。
```

这会导致 other-lock inputs 污染 current-lock validation，错误拒绝合法交易。

正确边界是：

```text
只检查当前 lock group 的 inputs 是否完整落在 OTX aggregate input range 内。
```

### 2. 不建议为了 `MissingLockGroupCoverage` 改变 witness 校验顺序

当前 E2E 中某些场景先返回 `InvalidLockGroupWitness`。这个行为合理，因为缺少合法 tx-level carrier 本身就是更早发生的 witness 结构错误。

如果强行让 `MissingLockGroupCoverage` 先返回，可能需要绕过或推迟 tx-level carrier 校验，反而会让错误边界更模糊。

### 3. 不建议自动忽略 tx-level 与 OTX duplicate action

同 target duplicate action 应拒绝，而不是由业务脚本猜测应该消费哪一个。自动忽略 tx-level action 会隐藏交易构造错误，也可能为后续业务脚本引入歧义。

### 4. 不建议引入 framework 到 fixtures 的反向依赖

近期测试覆盖已经证明当前边界足够表达这些安全场景：

- framework 提供通用 `TxShape`、typed handles、mutation、signing oracle。
- fixtures 负责业务场景、expected outcome、coverage tags。

不应为了个别测试便利破坏该边界。

## 建议后续落地顺序

1. 在 `cobuild-core` 相关函数附近补充短注释，说明 current-lock-only coverage 和错误优先级。
2. 在现有中文 spec 或新 behavior notes 中记录本文档的协议语义。
3. 如果后续继续扩展 coverage，再考虑 fixture-level manifest helper。
4. 暂不修改 `cobuild-otx-lock`、`cobuild-core`、limit-order 测试合约的核心行为。

## 当前判断

当前实现没有明确需要立即修复的安全问题。推荐修正方向是提升可读性、可维护性和协议语义透明度，而不是改变执行行为。
