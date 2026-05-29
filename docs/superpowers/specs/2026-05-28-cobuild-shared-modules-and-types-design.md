# Cobuild 共享模块与类型系统设计

## 状态

本文档定义 Cobuild Core v1 在 Rust 侧的共享模块分层、Molecule 类型系统、
代码生成策略以及核心 API 边界。

除非被显式修订，否则本文档是当前工作区后续 Cobuild 基础库开发的权威设计基线。

## 与协议规范的关系

本文档建立在以下协议规范之上：

- [2026-05-28-cobuild-core-community-redraft-design.md](/home/xcshuan/contracts/ckb/cobuild-otx-contracts/docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md:1)
- [2026-05-28-cobuild-core-community-redraft-design.zh-CN.md](/home/xcshuan/contracts/ckb/cobuild-otx-contracts/docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.zh-CN.md:1)

这份共享模块设计不重定义 Cobuild Core 的协议语义，只负责把已定稿的协议语义
落成可复用的 Rust 工程边界。

若本文档与上面的协议规范发生冲突，应以上述协议规范为准，并修订本文档。

## 范围

本文档定义：

- Cobuild 共享 Rust crate 的分层与依赖方向；
- `cobuild-types` 的 Molecule schema 边界；
- `cobuild-core` 的 `no_std`、合约优先协议语义 API 边界；
- 生成代码、版本化、测试与验证要求；
- 后续 `cobuild` 基础库与 `cobuild-otx-lock` 的接入约束。

本文档不定义：

- 应用级 `Action.data` schema；
- `BuildingPacket`、wallet packet、builder packet、agent packet；
- 具体 lock 的签名算法实现；
- 具体 type script 的业务规则；
- 非 Rust 语言绑定的交付形式。

## 目标

- 用 Molecule 为 Cobuild Core v1 提供唯一规范的线格式来源。
- 把生成代码与协议语义逻辑彻底分离，不继承 PoC 的目录与模块历史包袱。
- 提供一套 `no_std` 优先、以合约可读性、可审计性与执行性能为先的 core 语义层。
- 让 `cobuild-types` 与 `cobuild-core` 直接服务 lock script / type script 的实现需求。
- 为后续 `cobuild` 基础库与 `cobuild-otx-lock` 提供稳定依赖基线。

## 非目标

- 把所有未来扩展都提前塞进 core schema。
- 为某个单独 demo 或某个单独 lock script 量身定制共享模块接口。
- 把 reference flow 对象抬升成 core 类型。
- 为链上与链下强行统一一套通用易用 API。
- 让 `cobuild-core` 直接负责验签、syscall 或脚本调度。

## 设计原则

### 1. Molecule 是线格式真相源

所有 Cobuild Core v1 的共享链上对象都必须先在 `.mol` 文件中定义，再生成 Rust 类型。
不得先设计 Rust 结构体，再反向拼出近似 schema。

### 2. Schema 最小化，语义进 Core

`cobuild-types` 只表达：

- 字段存在什么；
- 顺序是什么；
- 编码是什么。

`script_role`、`seal scope`、`append_permissions`、各类 mask 的合法取值与解释规则，
全部由 `cobuild-core` 负责，不能把语义约束塞回 schema 伪装成“更高级的类型”。

### 3. Core 是协议语义引擎，不是脚本模板

`cobuild-core` 必须输出结构化的解析结果与待验证任务，不能采用
“把 callback 传进来，我替你驱动整套 lock flow”的模板式 API。

### 4. Contract-First，测试替身第二

`cobuild-core` 首先服务合约实现，而不是服务链下工具的人机体验。

任何额外抽象只有在同时满足以下条件时才允许进入 core：

- 不削弱合约侧代码的可读性；
- 不扩大合约侧必须理解的概念面；
- 不为链下便利而牺牲链上执行路径的清晰度与性能。

因此，Core v1 不在当前阶段引入面向链下的 owned/domain 层。

### 5. 版本演进优先保守稳定

Core v1 的线格式对象和核心语义对象一旦落地，应视为冻结接口。
未来扩展优先通过新增 crate、新增扩展 schema 或新 major 版本实现，
而不是对 v1 core 对象做隐式兼容追加。

## 推荐工作区布局

后续实现建议直接采用两 crate 加一个显式 codegen 工具：

```text
crates/
  cobuild-types/
    Cargo.toml
    schemas/
      core.mol
      witness.mol
    src/
      generated/
        core.rs
        witness.rs
      lib.rs

  cobuild-core/
    Cargo.toml
    src/
      lib.rs
      error.rs
      semantic.rs
      mask.rs
      witness.rs
      layout.rs
      loader.rs
      hash.rs
      tasks.rs
      context.rs
```

允许未来新增：

- `crates/cobuild-extensions-*`
- `crates/cobuild-ckb-std-*`
- `crates/cobuild-offchain-*`
- `crates/cobuild-otx-lock`

但不允许把它们的职责倒灌回 `cobuild-types` 与 `cobuild-core`。

## Crate 分层与依赖方向

依赖方向固定为：

```text
cobuild-core -> cobuild-types
cobuild-core -> ckb-std
```

禁止：

- `cobuild-types` 依赖 `cobuild-core`
- 为 core 再包一层面向链下复用的 provider trait
- 为链下便利把全量 owned transaction snapshot 塞进 `cobuild-core`
- 为链下便利把额外的 owned/domain API 塞进 `cobuild-core`

## `cobuild-types`

### 职责

`cobuild-types` 只负责：

- 保存 Cobuild Core v1 的 `.mol` 源文件；
- 保存由这些 schema 生成的 Rust 代码；
- 对外暴露稳定的 Molecule entity / reader 模块路径。

它不负责：

- 哈希规则；
- witness 语义解释；
- scope 划分；
- builder / wallet / agent packet；
- 任何脚本业务逻辑。

### 只包含 Core 规范对象

`cobuild-types` 中只允许出现以下对象：

- `Action`
- `ActionVec`
- `Message`
- `SighashAll`
- `SighashAllOnly`
- `SealPair`
- `SealPairVec`
- `OtxStart`
- `Otx`
- `WitnessLayout`

明确不允许放入：

- `BuildingPacket`
- `ResolvedInputs`
- `ScriptInfo`
- `OtxBatch`
- 任何 reference flow 对象
- 任何为了 Rust 使用手感而额外创造的中间类型

### Schema 文件划分

推荐把 schema 固定成两份：

- `schemas/core.mol`
  - 定义所有 Core v1 对象
- `schemas/witness.mol`
  - 仅定义 `WitnessLayout` union

不再延续 PoC 中把 packet、script info、resolved inputs 混入同一 schema 的做法。

### `core.mol`

`core.mol` 应定义为：

```text
import blockchain;

table Action {
  script_info_hash: Byte32,
  script_role: byte,
  script_hash: Byte32,
  data: Bytes,
}

vector ActionVec <Action>;

table Message {
  actions: ActionVec,
}

table SighashAll {
  seal: Bytes,
  message: Message,
}

table SighashAllOnly {
  seal: Bytes,
}

table SealPair {
  script_hash: Byte32,
  scope: byte,
  seal: Bytes,
}

vector SealPairVec <SealPair>;

table OtxStart {
  start_input_cell: Uint32,
  start_output_cell: Uint32,
  start_cell_deps: Uint32,
  start_header_deps: Uint32,
}

table Otx {
  message: Message,

  append_permissions: byte,

  base_input_cells: Uint32,
  base_input_masks: Bytes,

  base_output_cells: Uint32,
  base_output_masks: Bytes,

  base_cell_deps: Uint32,
  base_cell_dep_masks: Bytes,

  base_header_deps: Uint32,
  base_header_dep_masks: Bytes,

  append_input_cells: Uint32,
  append_output_cells: Uint32,
  append_cell_deps: Uint32,
  append_header_deps: Uint32,

  seals: SealPairVec,
}
```

约束如下：

- 不引入 `flag`
- 不引入 `fixed_*` / `dynamic_*`
- 不引入 `ScriptInfo`
- 不引入 `option` 或 `union` 包装来模拟 Rust 枚举手感
- `append_permissions`、`script_role`、`scope` 均保留为 `byte`
- 所有 mask 保留为 `Bytes`

### `witness.mol`

`witness.mol` 应定义为：

```text
import core;

union WitnessLayout {
  SighashAll: 4278190081,
  SighashAllOnly: 4278190082,
  Otx: 4278190083,
  OtxStart: 4278190084,
}
```

约束如下：

- 这 4 个 discriminant 在 Core v1 中冻结
- 后续如需新增 witness variant，只能追加新的 discriminant
- 不允许重排、复用、替换现有 discriminant

### 公开模块路径

`cobuild-types` 对外公开的 Rust 模块路径应稳定为：

- `cobuild_types::core`
- `cobuild_types::witness`

`src/generated/` 是内部代码生成目录，不应成为外部调用方依赖的路径约定。

### 生成代码策略

Core v1 推荐采用“`.mol` 为真相源，生成后的 Rust 文件纳入版本控制”的策略。

原因：

- 便于 code review 直接看到生成代码变化；
- 不要求下游构建环境安装额外的 Molecule 代码生成工具；
- 避免把 schema 演进隐藏在 `build.rs` 的隐式副作用里。

因此：

- schema 更新必须伴随重新生成的 Rust 代码一起提交；
- 代码生成通过仓库内显式命令触发；
- 不依赖下游 crate 构建阶段自动生成。

本文档不强制具体命令名，但要求它是仓库内可重复执行的显式流程。

## `cobuild-core`

### 职责

`cobuild-core` 是 Cobuild Core v1 的协议语义层。

它负责：

- 识别 `WitnessLayout`
- 解析 `Message` / `Action`
- 校验 `script_role`、`scope`、`append_permissions`、mask 合法性
- 推导 OTX layout 与 remainder layout
- 构造标准签名前像与哈希
- 为 lock / type 输出结构化待验证任务与消息访问结果

它不负责：

- 调用 syscall 读取交易
- 实现具体验签算法
- 强制某个 lock / type 的私有业务策略
- 提供 builder / wallet packet API

### 依赖与类型基线

`cobuild-core` 应保持 `no_std`，允许使用 `alloc`。

它应直接依赖：

- `cobuild-types`
- `molecule`
- 官方 CKB packed 类型 crate
- 最小必要的哈希依赖

推荐它面向 `ckb_gen_types::packed` 这一层工作，而不是直接面向
`ckb_types::core::TransactionView`。

这样可以把协议语义固定在最小、最明确的链上数据模型上。
如果未来需要链下便利层，应放到单独的 `cobuild-offchain-*` crate 中，而不是倒灌回 core。

### 模块划分

推荐划分为：

- `error`
  - 协议错误类型
- `semantic`
  - `ScriptRole`、`SealScope`、`AppendPermissions` 等受限语义类型
- `mask`
  - 各类 base scope mask 的解码与验证
- `witness`
  - `WitnessLayout` 识别与轻量封装
- `layout`
  - OTX 分区、remainder 分区、索引区间推导
- `provider`
  - 面向合约和测试替身的最小数据输入边界
- `hash`
  - 标准前像构造与签名哈希
- `tasks`
  - lock/type 消费的结构化任务
- `context`
  - 聚合上述能力的只读分析上下文

### 语义新类型

以下对象必须在 `cobuild-core` 中变成受限语义类型，而不是让调用方反复手写
`u8` / `Bytes` 解释逻辑：

- `ScriptRole`
  - `InputLock`
  - `InputType`
  - `OutputType`
- `SealScope`
  - `Base`
  - `Append`
- `AppendPermissions`
  - bit 0: append inputs
  - bit 1: append outputs
  - bit 2: append cell deps
  - bit 3: append header deps
- `BaseInputMask`
  - 每个 input 使用 2 bit
  - bit 0: `since`
  - bit 1: `previous_output`
- `BaseOutputMask`
  - 每个 output 使用 4 bit
  - bit 0: `capacity`
  - bit 1: `lock`
  - bit 2: `type`
  - bit 3: `data`
- `BaseDepMask`
  - `cell_dep` / `header_dep` 均按每项 1 bit 解释

Core v1 规定所有 mask bit 采用正向语义：

- `1` 表示被该 scope 签名覆盖
- `0` 表示不被该 scope 签名覆盖

另一个必须固定的点是：

- `BaseInputMask` 只作用于 `CellInput` 字段本身
- 即使 `previous_output` 未被覆盖，对应 resolved input 的 `CellOutput` 与 data
  仍然必须被 base scope 签名哈希完整覆盖

这条规则是 Core v1 的协议要求，不允许在共享模块实现中放宽。

### `loader.rs` 与按阶段最小输入

`cobuild-core` 应直接依赖 `ckb-std`，并在 crate 内提供**面向合约路径的 syscall
loader**。但协议逻辑本身不应强依赖“整笔交易已被完整读入内存”。

Core v1 应采用两层边界：

- `loader.rs`
  - 负责通过 `ckb-std` 的 `load_*` syscall 按需读取 witness、input、output、
    cell dep、header dep、resolved input cell 及其 data；
  - 对外提供少量 contract-facing helper；
  - 允许内部做最小必要缓存，但不承诺“全交易快照”语义。
- 协议算法模块
  - `layout.rs`、`hash.rs`、`tasks.rs`、`context.rs`
  - 只消费**按阶段拆开的 concrete input**
  - 不要求共享一个 `TxView` 或 `provider trait`

推荐的第一层最小输入对象是 `LayoutTx`：

```rust
pub struct LayoutTx {
    pub witnesses: alloc::vec::Vec<molecule::bytes::Bytes>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}
```

它只服务于 OTX layout 计算，不承载 tx hash、resolved input、output data
等其他阶段无关信息。

这条边界的含义是：

- 合约主路径优先通过 syscall 按需加载，而不是先 materialize 全交易；
- 测试可以直接构造 `LayoutTx`、后续 hash-specific parts 或 task-specific
  fixture；
- `cobuild-core` 不暴露泛化 provider trait，也不以全量 `TxView` 作为长期
  公共接口；
- 如果未来需要链下便利层，应在独立 crate 中桥接，而不是反向扩大 core API。

### 哈希实现约束

`cobuild-core::hash` 必须严格实现协议规范里已经定稿的 4 个标准签名域：

- `TxWithMessage`
- `TxWithoutMessage`
- `OtxBase`
- `OtxAppend`

实现约束如下：

- 不允许再引入 PoC 风格的额外“近似等价”哈希模式名
- 不允许把 `SighashAllOnly` 当成单独的第五种协议域
- 所有 domain separation 必须与协议规范保持一致
- 必须使用协议规范写死的 16-byte BLAKE2b personalization 常量

签名前像构造必须满足单射要求：

- 定宽标量按固定宽度编码
- 变长原始字节串必须带长度前缀
- 变长列表必须先编码 count
- 不允许通过裸拼接留下 `23,4` 与 `2,34` 这类边界歧义

OTX 相关实现还必须写死：

- `base_scope_commitment = Blake2b_OtxBase(OtxBase_preimage)`
- 不再对这个 digest 额外做第二次哈希
- append scope 不支持字段级 mask
- append scope 的哈希必须绑定 `base_scope_commitment`

这一层实现的是协议规范，不是便捷近似版。

### 只读分析上下文

推荐 `cobuild-core` 提供一个中心只读对象，例如 `CobuildContext<'a>`，
由它聚合 witness 解析、layout 计算和任务生成结果。

它的职责应该是：

- 持有对当前阶段 concrete input 或 loader 结果的只读引用
- 惰性或一次性解析相关 Cobuild witness
- 缓存 OTX layout 结果
- 按脚本查询 lock/type 相关任务

它不应该：

- 替调用方决定是否 fallback 到 legacy
- 替 lock 调用验签函数
- 替 type 决定“没有 Action 是否失败”

### 任务输出，而非 callback 输入

`cobuild-core` 的锁脚本接口必须是“输出待验证任务”，而不是“接收 callback”。

推荐至少提供：

- `TxLevelLockTask`
  - `carrier_witness_index`
  - `mode` (`TxWithMessage` / `TxWithoutMessage`)
  - `signing_message_hash`
  - `seal`
  - `script_hash`
- `OtxLockTask`
  - `otx_index`
  - `scope` (`Base` / `Append`)
  - `signing_message_hash`
  - `seal`
  - `script_hash`
  - `covered_ranges`
- `TypeMessageAccess`
  - 该 type 可见的 tx-level message
  - 该 type 可见的 OTX-level message
  - 与当前 type 相关的 `Action` 集合

调用方拿到这些任务后，自行决定：

- 是否验签
- 是否要求必须存在相关 `Action`
- 是否启用更严格的脚本私有策略

### 错误模型

`cobuild-core` 的错误类型必须把不同层级的协议错误分开，但不再为“底层数据来源错误”
预留泛型参数。既然 core 直接依赖 `ckb-std`，就应该提供一个**具体的**
`LoadError` 来表示 syscall / contract-loading 失败。

推荐顶层错误形状为：

```rust
pub enum CoreError {
    Load(LoadError),
    Decode(DecodeError),
    Semantic(SemanticError),
    Layout(LayoutError),
    Hash(HashError),
}
```

约束如下：

- `LoadError` 只表示 syscall、本地 contract loader 或按索引加载失败
- `DecodeError` 只表示 Molecule / witness 编码非法
- `SemanticError` 只表示字段值或语义组合非法
- `LayoutError` 只表示 OTX 分区、计数、区间或顺序非法
- `HashError` 只表示签名前像构造所需输入不成立

不得把这些错误全部压扁成一个整数错误码。

## 与未来合约和基础库的集成规则

### `cobuild` 基础库

未来的 Cobuild Rust 基础库应建立在两层之上：

- `cobuild-types` 提供线格式
- `cobuild-core` 提供协议语义

如果未来需要链下便利层，应单独放在 `cobuild-offchain-*` 中，
不能在 core 内部演化出第二套面向链下的易用 API。

### `cobuild-otx-lock`

未来的 `cobuild-otx-lock` 应：

- 直接依赖 `cobuild-core`
- 只把自己的签名校验与脚本私有策略叠加在 core 输出的任务之上

`cobuild-otx-lock` 不应复制：

- OTX witness 解析
- OTX layout 推导
- Cobuild 标准哈希
- `SealPair` 查找逻辑

这些都必须来自共享模块。

### Type script 接入

未来任何 Cobuild-aware type script 应通过 `cobuild-core` 获取：

- 与自己相关的 tx-level / OTX-level `Message`
- 过滤后的相关 `Action`
- 当前 scope 视角下的布局信息

type script 是否要求：

- 必须存在相关 `Action`
- 没有 `Message` 就失败
- 多个 `Action` 是否允许并存

这些是脚本私有策略，不在共享模块中硬编码。

## 版本化规则

### Core v1 冻结面

以下内容在 Core v1 中视为冻结：

- `core.mol` 中所有 table / vector 对象的字段顺序与字段含义
- `witness.mol` 中现有 4 个 discriminant
- `cobuild-core` 对 `script_role`、`scope`、mask、append 权限的语义解释
- 标准签名前像与哈希规则

### 可接受的 v1 演进

Core v1 允许的演进只包括：

- 修正文档歧义而不改变行为
- 修复实现 bug 以匹配既定规范
- 新增不影响现有语义的辅助 API

### 不可接受的 v1 演进

以下变化不应以 v1 patch / minor 的形式出现：

- 修改已有 Molecule 字段顺序
- 复用或替换 `WitnessLayout` discriminant
- 改变已有 bit 位含义
- 为链下便利把新的 owned/domain 抽象塞进 `cobuild-core`
- 把 reference flow 对象加入 `cobuild-types`

这类变化应视为：

- 新扩展 crate，或
- Core v2 设计议题

## 测试与验证要求

### `cobuild-types`

必须具备：

- schema 生成结果与源 `.mol` 同步的检查
- `WitnessLayout` discriminant 稳定性测试
- 关键对象的 golden encoding 测试

### `cobuild-core`

必须具备：

- `script_role` / `scope` / append 权限非法值测试
- mask 长度与位解释测试
- duplicate `(script_hash, scope)` `SealPair` 测试
- `OtxStart + Otx` layout 合法性测试
- tx-level 与 OTX-level 哈希向量测试
- malformed witness fail-closed 测试
- 相关脚本与无关脚本的局部 flow 选择测试

## 实现前置约束

后续实现计划必须遵守以下约束：

- 先实现 `cobuild-types`
- 再实现 `cobuild-core`
- `cobuild-otx-lock` 必须建立在 `cobuild-core` 稳定 API 之上
- 在 `cobuild-core` API 稳定前，不开始 contract 特定 helper 泛滥扩张
- 不在当前一期里实现链下 convenience crate

## 结论

本设计把 Cobuild 共享 Rust 模块固定为两层：

- `cobuild-types`：唯一的 Core v1 Molecule 线格式来源
- `cobuild-core`：`no_std` 协议语义与任务生成层

这套分层故意切断了 PoC 时代最容易失控的几类耦合：

- schema 与 reference flow 的耦合
- 生成代码与手写语义的耦合
- core 逻辑与 syscall 的耦合
- core 逻辑与链下 convenience 层的耦合
- 共享模块与具体 lock / type 业务策略的耦合

后续开发应直接以本文档为准，而不再沿用旧 PoC 的 crate 边界与类型组织方式。
