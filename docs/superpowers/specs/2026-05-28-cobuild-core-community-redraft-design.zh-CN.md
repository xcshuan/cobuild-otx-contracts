# Cobuild Core 社区重写版设计

## 状态

本文档定义了一版拟议中的 CKB Cobuild Core 协议重写稿。
它的目标是用一份单一设计基线，取代目前由 overview 帖子、appendix、
PoC 特定实现选择以及讨论草案混合构成的分散状态，作为后续实现工作的统一依据。

除非被显式修订，否则本文档是当前工作区后续开发的权威设计基线。

## 范围

本文档定义：

- Cobuild Core、标准扩展、参考流程三层边界；
- Cobuild Core 的规范化 witness / 数据模型；
- Cobuild Core 的规范化哈希与签名规则；
- lock script 与 type script 的最小规范责任；
- Cobuild witness 与 legacy `WitnessArgs` 的共存规则；
- 错误模型，以及扩展 / 版本化边界。

本文档不定义：

- 应用特定的 `Action.data` schema；
- 通用资产 action 标准；
- 强制性的链下 packet 或 agent 流程；
- 应用特定的排序、批处理或市场结构。

## 目标

- 定义一套稳定、面向社区的 Cobuild Core，使其可被多个 lock script、
  type script、wallet 与 builder 实现。
- 保持与 legacy witness 编码以及非 Cobuild 脚本的前向兼容。
- 在核心 witness / 签名模型中同时支持 dynamic OTX 与细粒度签名控制。
- 在 core 数据模型中保留 `Action`，但不将 action 存在性设为全局有效性前提。
- 将 approved-action 这类更高层语义从 core 中移出，放入标准扩展层。

## 非目标

- 在 core 协议中标准化所有应用语义。
- 强制所有脚本同时支持 Cobuild 与 legacy 两套 flow。
- 标准化一套强制性的链下协作流程。
- 保留当前 PoC 中所有偶然形成的设计细节。

## 分层

Cobuild 被划分为三层。

### 1. Cobuild Core

Cobuild Core 是规范性的 witness 与验证协议。

它标准化：

- `WitnessLayout` 的编码方式，以及与 legacy witness 的 union id 隔离；
- core witness variant 及其语义；
- OTX scope 划分；
- dynamic OTX 语义；
- 细粒度签名覆盖语义；
- 标准签名消息哈希构造；
- 交易级 Cobuild 激活与局部验证选择规则；
- lock / type 的最小验证职责；
- 兼容、扩展与版本化规则。

Cobuild Core 不是应用 action 标准。

### 2. 标准扩展

标准扩展在 Cobuild Core 之上定义可选的高层语义。它可以标准化：

- 通用的 `Action.data` schema；
- 跨协议交互模式；
- approved-action 及类似模式；
- 某类应用领域的 action 家族；
- 比 Core 更强的脚本级约束。

标准扩展不得重定义 Core 的哈希、witness 语义或最小验证职责。

### 3. 参考流程

参考流程只是推荐的链下工程模式。

它可以标准化：

- `BuildingPacket`；
- `OtxBatch`；
- wallet 展示流程；
- signer / builder / agent 的交互模式；
- 推荐的 packet 版本化方式。

参考流程不是链上有效性规则的一部分。

## 术语

- `Core`：本文档定义的规范协议。
- `Extension`：构建在 Core 之上的可选标准。
- `Reference flow`：链下推荐方式，不是有效性规则。
- `Tx-level flow`：使用 transaction-level witness 的非 OTX Cobuild 签名流程。
- `OTX flow`：使用 `OtxStart` 与一个或多个 `Otx` witness 的 Cobuild 流程。
- `Base scope`：由原始 OTX 创建者签名的 OTX 部分。
- `Append segment`：后续追加的一个有序贡献片段。每个 segment 有自己的实体计数、
  flags 和 seals；append input owner 按 segment 签名。
- `TxWithMessage`：文档术语，表示 tx-level flow 中存在且仅存在一个有效的
  `SighashAll` witness，且其中携带唯一的 transaction `Message`。
- `TxWithoutMessage`：文档术语，表示 tx-level flow 中不存在有效的
  `SighashAll` witness，因此不存在 transaction-level `Message`。

`TxWithMessage` 与 `TxWithoutMessage` 只是用于选择哈希规则的描述性术语。
它们不是独立的链上对象，也不在交易中占据显式字段。

## Core 数据模型

### WitnessLayout

`WitnessLayout` 仍然是 Cobuild witness 的入口结构。

它的 union id 必须继续位于高位 custom-id 区间，以保证
`WitnessLayout` 与 legacy `WitnessArgs` 在编码层面可被无歧义地区分。

Core v1 使用以下 witness variant：

- `SighashAll`
- `SighashAllOnly`
- `OtxStart`
- `Otx`

### Action

Core 保留 `Action` 作为一等数据对象，但 action 存在性本身不是通用有效性条件。

```text
table Action {
  script_info_hash: Byte32,
  script_role: byte,   // 0=input_lock, 1=input_type, 2=output_type
  script_hash: Byte32,
  data: Bytes,
}
```

`script_role` 是 core 对象的一部分，用于让 `Action` 能无歧义地标识它目标脚本所处的位置。

Core v1 规定：

- `0`：`input_lock`
- `1`：`input_type`
- `2`：`output_type`

其他值在 Core v1 中均非法。

### Message

```text
table Message {
  actions: ActionVec,
}
```

`actions` 可以为空。

空的 `actions` vector 表示该 witness 没有携带供当前脚本消费的标准化 action 语义。
这在 Core 中是合法的。

### SighashAll

```text
table SighashAll {
  seal: Bytes,
  message: Message,
}
```

当脚本需要 transaction-level `Message` 时，交易中最多只能存在一个有效的
`SighashAll` witness。

### SighashAllOnly

```text
table SighashAllOnly {
  seal: Bytes,
}
```

`SighashAllOnly` 是只承载 seal 的 witness 容器。

它自身不携带 `Message`。在 `TxWithMessage` 中，使用 `SighashAllOnly`
的签名者仍然签与 transaction-level 相同的签名消息哈希，该哈希覆盖唯一的
`SighashAll.message`。

### LockSeal

`LockSeal` 用于 OTX witness 内部，将一个 lock script hash 绑定到一个加密 seal。

```text
table LockSeal {
  script_hash: Byte32,
  seal: Bytes,
}
vector LockSealVec <LockSeal>;
```

`LockSeal` 自身不携带 scope byte。它所在的位置决定签名来源：

- `Otx.base_seals` 中的 seal 用于 OTX base signing hash。
- `OtxAppendSegment.seals` 中的 seal 用于该 append segment 的 signing hash。

同一个 lock script 可以同时出现在 base input range 和一个或多个 append
segment input range 中。在这种情况下，该 lock 必须在每个相关 seal vector 中
分别提供一个独立的 `LockSeal`。

### OtxStart

```text
table OtxStart {
  start_input_cell: Uint32,
  start_output_cell: Uint32,
  start_cell_deps: Uint32,
  start_header_deps: Uint32,
}
```

`OtxStart` 标记交易中属于第一个 OTX 的各类实体起始索引。
`OtxStart` 自身的 witness 索引则标记 OTX witness 序列的起点。

`OtxStart` 是最终聚合交易的运行时分区元数据，
不属于创建者签名覆盖的 `OtxBase` 或 `OtxAppendSegment` 哈希域。

### Otx

Core v1 定义一个统一的 `Otx` 对象，覆盖 dynamic OTX 与细粒度签名控制。

```text
table Otx {
  message: Message,

  append_permissions: byte,   // bit 0=input, 1=output, 2=cell_dep, 3=header_dep

  base_input_cells: Uint32,
  base_input_masks: Bytes,

  base_output_cells: Uint32,
  base_output_masks: Bytes,

  base_cell_deps: Uint32,
  base_cell_dep_masks: Bytes,

  base_header_deps: Uint32,
  base_header_dep_masks: Bytes,

  append_segments: OtxAppendSegmentVec,

  base_seals: LockSealVec,
}
```

每个 append segment 有自己的计数、flags 和 seals：

```text
table OtxAppendSegment {
  segment_flags: byte,

  input_cells: Uint32,
  output_cells: Uint32,
  cell_deps: Uint32,
  header_deps: Uint32,

  seals: LockSealVec,
}
vector OtxAppendSegmentVec <OtxAppendSegment>;
```

设计意图：

- `append_permissions` 是创建者签下的权限位图，用于表示 append segments
  是否允许包含额外 input、output、cell dep 或 header dep。
- `base_*` 字段定义原始创建者签名的 OTX 范围。
- `append_segments` 定义有序的追加实体范围。每个 segment 携带自己的
  finality/coverage flags 和 append seals。
- 细粒度覆盖只作用于 base scope。
- append segments 在 Core v1 中使用全字段覆盖。

对于一个合法的 `Otx`，`base_input_cells` 必须大于 0。

原因：

- base scope 携带创建者授权的 `Message` 与 `append_permissions`；
- 如果没有至少一个 base input，就没有任何 lock owner 对 base scope 进行签名；
- 因此 Core v1 禁止“只有 append segment 授权、但 base 本身无人签名”的空壳 OTX。

Core v1 中 `append_permissions` 的 bit 定义为：

- bit 0：允许追加 inputs
- bit 1：允许追加 outputs
- bit 2：允许追加 cell deps
- bit 3：允许追加 header deps

bit 4 到 bit 7 为保留位，必须为 0。

如果任何 append segment 在某类实体上的 count 非 0，但对应 permission bit 为 0，
则该 `Otx` 非法。这个检查针对“某类实体是否至少被追加过”的聚合事实，而不是
每个 segment 的计数策略。

## OTX Scope 模型

对于每个 `Otx`，Core 定义一个 base scope 和零个或多个有序 append segments：

- `base scope`
- `append segment 0`
- `append segment 1`
- ...

单个 `Otx` 覆盖的实体按以下顺序布局：

- base inputs
- append segment 0 inputs
- append segment 1 inputs
- ...
- base outputs
- append segment 0 outputs
- append segment 1 outputs
- ...
- base cell deps
- append segment 0 cell deps
- append segment 1 cell deps
- ...
- base header deps
- append segment 0 header deps
- append segment 1 header deps
- ...

每个 OTX 在每类实体上都消费交易中的一个连续切片。
不同 OTX 依据 `OtxStart` 锚点以及遍历 `Otx` 序列时累加的计数连续排布。
实现可以缓存一个 OTX 的 aggregate append ranges，但这些 ranges 必须由有序
segment counts 派生，不是独立的 witness schema。

Core 不定义任何“全局 transaction mode”。scope 只由消费相关 OTX witness
的脚本在本地解释。

## 细粒度覆盖模型

### 通用规则

- 细粒度签名控制只作用于 Core v1 的 base scope。
- `1` 表示对应字段或条目被 base-scope 签名哈希覆盖。
- `0` 表示其不被 base-scope 签名哈希覆盖。
- mask 字节按逐项 bit-pack 编码，字节内部采用最低有效位优先顺序。
- mask 最后一个字节中未使用的 padding bit 必须为 0。
- mask 字节长度必须精确匹配对应 count 所需的 bit 数。

对于 Core v1，四类 mask 的字节长度公式为：

- `base_input_masks.len == ceil(base_input_cells * 2 / 8)`
- `base_output_masks.len == ceil(base_output_cells * 4 / 8)`
- `base_cell_dep_masks.len == ceil(base_cell_deps / 8)`
- `base_header_dep_masks.len == ceil(base_header_deps / 8)`

### Base Input Mask

每个 base input 使用 2 bit：

- bit 0：`since`
- bit 1：`previous_output`

对于 base input，mask 只作用于 `CellInput` 字段。

对应的 resolved input `CellOutput` 及其 data 始终被 base-scope
签名哈希完整覆盖。这样既保留了旧 OTX 设计的安全意图，也允许对 `CellInput`
属性进行更细粒度的控制。

更准确地说：

- 将 `previous_output` mask 掉，放松的是对“被消费 UTXO 精确身份”的承诺；
- 继续对 resolved input `CellOutput` 与 data 做哈希，保留的是对“被消费状态内容”的承诺；
- 因此，当 `previous_output` 不被覆盖时，Core v1 只允许把该 input
  替换成另一个从签名哈希视角看拥有相同 `CellOutput` 与 data 的 input cell。

Core v1 有意不允许 base input mask 同时去掉 resolved input `CellOutput`
与 data 的覆盖。因为如果既不约束 outpoint 身份、又不约束 resolved
cell/data 内容，那么 base input 替换就会对 core 协议来说过于宽松。

### Base Output Mask

每个 base output 使用 4 bit：

- bit 0：`capacity`
- bit 1：`lock`
- bit 2：`type`
- bit 3：`data`

一个 output slot 可以覆盖这些字段中的任意子集，甚至一个都不覆盖。
Core 允许这种情况存在。具体脚本或扩展可以施加更强约束。

### Base CellDep Mask

每个 base cell dep 使用 1 bit：

- bit 0：整个 `CellDep`

### Base HeaderDep Mask

每个 base header dep 使用 1 bit：

- bit 0：整个 `Byte32`

## 哈希与签名域

### 通用规则

Core v1 标准化精确的签名前像结构。
实现方在自称兼容 Core 时，不得自行选择拼接顺序或字段 framing。

签名域隔离必须通过 BLAKE2b personalization 实现，而不是额外的 witness 字段。

签名前像的序列化必须对“有序字段序列”保持单射。换言之，Core v1 必须防止
不同的逻辑字段元组在拼接后坍缩成同一字节串，例如：

- `(23, 4)` 与
- `(2, 34)`

在拼接后不可区分。

为了保证这一点，Core v1 采用以下 framing 规则：

- `byte`、`u32`、`u64` 这类定宽标量，按其固定 canonical 宽度序列化；
- 所有变长原始字节串，都必须先写入 little-endian `u32` 长度，再写原始 bytes；
- 所有变长列表，都必须先写入 item count，再写 item payload；
- canonical Molecule 编码对象如果其编码本身可自定界，或在标准化序列中边界
  已无歧义，则可以直接追加；
- 即使本地实现可以通过其他方式恢复边界，也不得省略 Core 要求的 count 或
  length framing。

Core v1 使用四个签名域：

- `TxWithMessage`
- `TxWithoutMessage`
- `OtxBase`
- `OtxAppendSegment`

这些只是哈希规则名称，不是独立的 witness variant。

Core v1 同时固定以下 16-byte BLAKE2b personalization 常量：

- `TxWithMessage`：`b"ckbcb_twm_core1\0"`
- `TxWithoutMessage`：`b"ckbcb_tnm_core1\0"`
- `OtxBase`：`b"ckbcb_otb_core1\0"`
- `OtxAppendSegment`：`b"ckbcb_ots_core1\0"`

这些 byte string 是规范内容。实现必须精确使用它们，不得在运行时替换为更长的
人类可读名称。

### TxWithMessage

当交易中存在且仅存在一个有效 `SighashAll` witness 时，选择 `TxWithMessage`。

交易中的所有 tx-level Cobuild lock signer，包括使用 `SighashAllOnly`
的那些 signer，都必须签同一个 `TxWithMessage` 签名哈希。

其前像为：

1. 唯一 `SighashAll` 中的 `Message` 的 Molecule bytes
2. tx hash
3. 对每个 input 索引 `i`：
   - resolved input `CellOutput` 的 Molecule bytes
   - input data 长度，little-endian `u32`
   - input data bytes
4. 对每个索引 `>= inputs_len` 的 witness：
   - witness 长度，little-endian `u32`
   - witness bytes

### TxWithoutMessage

当不存在有效 `SighashAll` witness，且某个 tx-level Cobuild lock script
使用 `SighashAllOnly` 时，选择 `TxWithoutMessage`。

其前像与 `TxWithMessage` 相同，只是省略第 1 步。

### OtxBase

`OtxBase` 只覆盖某个 OTX 的 base scope。

其前像为：

1. 当前 `Otx` 中 `Message` 的 Molecule bytes
2. `append_permissions`，1 byte
3. `base_input_cells`，little-endian `u32`
4. `base_input_masks` 长度，little-endian `u32`
5. `base_input_masks` bytes
6. 对每个 base input slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 若 mask bit 0 为 `1`，则加入 `since`，little-endian `u64`，否则加入零值
   - 若 mask bit 1 为 `1`，则加入 `previous_output` 的 canonical Molecule bytes，
     否则加入默认 out point bytes
   - resolved input `CellOutput` 的 Molecule bytes
   - resolved input data 长度，little-endian `u32`
   - resolved input data bytes
7. `base_output_cells`，little-endian `u32`
8. `base_output_masks` 长度，little-endian `u32`
9. `base_output_masks` bytes
10. 对每个 base output slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 对以下每个 output 字段位置，若被覆盖则追加真实值，否则追加 canonical 默认值：
     - `capacity`，little-endian `u64`，或零值
     - `lock` 的 canonical Molecule bytes，或默认 script
     - `type` 的 canonical Molecule option bytes，或 empty option
     - output data 长度，little-endian `u32`，随后是 data bytes，或零长度
11. `base_cell_deps`，little-endian `u32`
12. `base_cell_dep_masks` 长度，little-endian `u32`
13. `base_cell_dep_masks` bytes
14. 对每个 base cell dep slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 若被覆盖则加入 `CellDep` 的 canonical Molecule bytes，否则加入默认 cell dep bytes
15. `base_header_deps`，little-endian `u32`
16. `base_header_dep_masks` 长度，little-endian `u32`
17. `base_header_dep_masks` bytes
18. 对每个 base header dep slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 若被覆盖则加入 header dep `Byte32`，否则加入 32 个零字节

这样设计的原因是：

- mask 自身被哈进前像，避免不同覆盖策略共享同一语义前像；
- OTX-local slot index 被哈进前像，避免在 OTX scope 内通过字段省略制造重排歧义，
  同时不把 OTX 绑定到完整交易里的绝对位置；
- 未覆盖的 base 字段用确定性的默认值表示，而不是直接省略，因此每个 slot 都有稳定的
  字段位置前像；
- resolved input cell 与 input data 始终被完整覆盖；
- append 权限被哈进前像，使 append segment 的可用性由创建者显式授权，而不是默认存在。

### OtxAppendSegment

`OtxAppendSegment` 覆盖一个 append segment，并将它绑定到一个特定的 base scope。

定义：

`base_scope_commitment = Blake2b_OtxBase(OtxBase_preimage)`

其中：

- `OtxBase_preimage` 是上一小节中定义的、精确标准化的 `OtxBase` 前像；
- `Blake2b_OtxBase` 表示一次使用标准化 `OtxBase` personalization 的 BLAKE2b 哈希调用。

得到的 32-byte digest 即为 `base scope commitment`。

Core v1 不会在这个 digest 之上再做第二次额外哈希。

每个 append segment 的 hash 都从 base scope commitment 开始。后续前像取决于
`segment_flags`。

如果 `coverage_previous_segments` 未设置，前像为：

1. `base scope commitment`
2. 自身 `segment_flags`
3. 自身 segment input count，little-endian `u32`
4. 对每个自身 segment input slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 完整 `CellInput` 的 canonical Molecule bytes
   - resolved input `CellOutput` 的 Molecule bytes
   - resolved input data 长度，little-endian `u32`
   - resolved input data bytes
5. 自身 segment output count，little-endian `u32`
6. 对每个自身 segment output slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 完整 output `CellOutput` 的 Molecule bytes
   - output data 长度，little-endian `u32`
   - output data bytes
7. 自身 segment cell dep count，little-endian `u32`
8. 对每个自身 segment cell dep slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 完整 `CellDep` 的 Molecule bytes
9. 自身 segment header dep count，little-endian `u32`
10. 对每个自身 segment header dep slot `i`：
   - OTX-local slot index `i`，little-endian `u32`
   - 完整 header dep `Byte32`

如果 `coverage_previous_segments` 已设置，前像为：

1. `base scope commitment`
2. previous segment count，little-endian `u32`
3. 对每个 previous segment，按顺序写入：
   - previous `segment_flags`
   - previous segment inputs、outputs、cell deps、header deps，使用上面相同的
     full-field count-and-item 编码
4. 自身 `segment_flags`
5. 自身 segment inputs、outputs、cell deps、header deps，使用上面相同的
   full-field count-and-item 编码

`Message` 已经被 `OtxBase` 覆盖；`OtxAppendSegment` 不再重复写入它。own-only
segment hash 不编码自身 segment index。previous-coverage segment hash 通过 previous
segment 的数量和有序内容绑定位置，而不是通过额外的 `previous_segment_index` 字段。

Core v1 有意不支持 append segment 上的细粒度 mask。

## Cobuild 激活与局部验证

对于支持 Cobuild 的脚本，Cobuild 激活是交易级的。

如果交易中任何 witness 被编码为 `WitnessLayout`，那么该交易中的每个
Cobuild-aware lock 或 type script 都必须在 Cobuild Core 规则集下评估自己的验证。
这类脚本不得仅因为自身 script group witness 或 message 在本地不相关，就忽略
Cobuild envelope 并 fallback 到 legacy-only 验证。

激活条件取决于 Cobuild `WitnessLayout` envelope 是否存在，而不是它是否唯一，
也不是所有 Cobuild witness 是否已经满足其余 Core 有效性规则。

这个交易级激活规则不要求交易里的每个脚本都支持 Cobuild。legacy-only 脚本可以
在同一笔交易中共存，并继续使用它自己的 legacy 规则验证。

Cobuild 激活后，每个 Cobuild-aware 脚本具体承担哪些验证义务，仍然是局部的、
相关性驱动的、并且 fail-closed。

### Lock Script 的 Flow 选择

对于已激活 Cobuild 交易中的某个 Cobuild-aware lock script：

- 如果当前 script group 的第一个 witness 是有效的 `SighashAll` 或
  `SighashAllOnly`，则该脚本必须对不属于任何相关 OTX-covered input 的那部分
  交易进入 tx-level Cobuild flow。
- 如果交易包含一个有效的 OTX 序列，且当前 lock 出现在一个或多个 OTX 的
  base input scope 或 append segment input scope 中，则该脚本也必须对这些 OTX
  signing origins 进入对应的 OTX flow。
- 若两者都不成立，该脚本在本次执行中没有 Cobuild 签名验证义务。它仍然不得仅为
  绕过已激活的 Cobuild 规则集，而将整笔交易当作 legacy-only 交易处理。

同一次 lock script 执行中，可以同时验证：

- 一个或多个 OTX seal；
- 一个 tx-level 的 remainder seal。

### Type Script 的 Flow 选择

对于已激活 Cobuild 交易中的某个 Cobuild-aware type script：

- 如果该脚本出现在某个相关 OTX scope 的 input / output 范围内，它可以读取
  该 OTX 的 `Message`。
- 如果该脚本出现在所有相关 OTX scope 之外，且交易中存在唯一有效的
  `SighashAll`，它也可以读取 tx-level `Message`。
- 如果不存在与之相关的有效 CoBuild `Message` 或 `Action`，Core 不强制其失败。
  该脚本仍然必须完成自身原生的状态转移验证。
- 一个 type script 可以施加更严格的策略，例如在缺少相关 `Message` 或
  `Action` 时拒绝通过；但这属于脚本私有策略，而不是 Core 默认要求。

### OTX 序列检测

只有满足以下全部条件时，才认为 OTX flow 存在：

- 存在且仅存在一个有效 `OtxStart`；
- 从 `OtxStart` 后一个 witness 开始，存在一段连续的有效 `Otx` witness 序列；
- 按 inputs、outputs、cell deps、header deps 累积得到的 OTX base 和 append
  segment 划分无溢出、无重叠、且彼此一致。

### 验证流程

本小节给出 Cobuild-aware 脚本的规范验证流程。实现可以用不同代码组织方式，
但最终验证决策必须等价。

对于每笔已激活 Cobuild 的交易，Cobuild-aware 脚本首先准备共享的 Cobuild 视图：

1. 检测交易中是否存在被编码为 `WitnessLayout` 的 witness。若不存在，该流程不激活，
   脚本可以使用自己的 legacy 规则。
2. 如果至少存在一个 Cobuild `WitnessLayout` envelope，则为当前 Cobuild-aware 脚本
   激活 Cobuild 验证。激活条件只看存在性，不看唯一性，也不要求所有 Cobuild
   witness 已经合法。
3. 扫描 witnesses，识别可选的 OTX 序列：
   - 查找有效 `OtxStart` witness；
   - 如果存在多个有效 `OtxStart`，则必须失败；
   - 如果恰好存在一个有效 `OtxStart`，则以它的 witness 索引和实体索引作为
     OTX anchor；
   - 从 `OtxStart` 后一个 witness 开始，收集连续的有效 `Otx` witness 序列；
   - 任何有效 `Otx` witness 都不得出现在这段连续序列之外；
   - 从 anchor 开始，按收集到的 OTX 序列累积计数，计算每个 OTX 的 base scope 与
     append segment scopes。
   对每类实体，transaction remainder 是 `OtxStart` anchor 之前的范围，与该类实体
   最后一个累积 OTX scope 之后的范围的并集。
4. 构建 tx-level message 视图：
   - 在需要 tx-level 唯一性的场景中，如果存在有效 `SighashAll` witness，则必须
     恰好只有一个；
   - 这个唯一 `SighashAll.message` 是 tx-level `Message`；
   - `SighashAllOnly` 永远不携带自己的 `Message`，但在存在 tx-level `Message`
     时签同一个 tx-level hash。
5. 对当前脚本消费或用于签名验证的每个 tx-level 或 OTX-level `Message`，必须针对
   完整交易验证所有 action target：
   - `input_lock` action 必须指向某个真实存在的 input lock script hash；
   - `input_type` action 必须指向某个真实存在的 input type script hash；
   - `output_type` action 必须指向某个真实存在的 output type script hash。

Cobuild-aware lock script 随后按以下流程验证 owner 授权：

1. 对每个与当前 lock script 相关的 OTX：
   - 判断当前 lock script hash 是否出现在该 OTX 的 base input scope、任意 append
     segment input scope，或两者都出现；
   - 如果 OTX `Message` 中存在指向当前 lock script hash 的 `input_lock` action，
     即使该 lock hash 不在当前 OTX local input scopes 中，该 OTX `Message` 也与当前
     lock 相关；
   - 如果出现在 base scope，必须在 `Otx.base_seals` 中为 `current_lock_hash`
     找到且仅找到一个 `LockSeal`，计算 `OtxBase`，并按该 lock 自己的加密规则验证 seal；
   - 对每个包含该 lock hash 的 append segment input scope，必须在该 segment 的
     `seals` 中为 `current_lock_hash` 找到且仅找到一个 `LockSeal`，计算
     `OtxAppendSegment`，并按该 lock 自己的加密规则验证 seal；
   - 相关 OTX base 或 append segment 中缺少 seal、重复 seal、seal 结构错误或签名
     无效，都必须失败。
   指向当前 lock 的 action 本身不会创建 OTX 签名要求；只有 lock hash 真实出现在
   OTX base 或 append segment input scope 中时，才要求对应的 OTX lock 签名。
2. 判断当前 lock 是否存在不属于任何相关 OTX-covered input scope 的 tx-level
   remainder inputs。若不存在，本次 lock 执行不需要 tx-level seal。
3. 如果存在 tx-level remainder inputs：
   - 当前 lock script group 的第一个 witness 必须是有效的 `SighashAll` 或
     `SighashAllOnly`；
   - 同一 lock script group 中所有非首位 witness 必须不存在或为空，除非该 lock
     自己的非 Cobuild ABI 明确定义了额外数据，并且这些数据仍被其 Cobuild 签名规则覆盖；
   - 当存在唯一有效 `SighashAll` 时，选择 `TxWithMessage`；
   - 当不存在有效 `SighashAll` 且 group-leading witness 是 `SighashAllOnly` 时，
     选择 `TxWithoutMessage`；
   - 计算所选 tx-level 签名哈希，并按该 lock 自己的加密规则验证 group-leading seal。
4. 如果交易已激活 Cobuild，但 OTX 验证和 tx-level remainder 验证都与当前 lock
   无关，则 generic Core planning 对本次执行没有 Cobuild 签名验证义务。具体参考
   lock contract 可以更严格；`cobuild-otx-lock` 在 planned signature requirement
   set 为空时会失败。Cobuild-aware lock 仍然不得仅为了忽略相关 Cobuild 错误而把交易
   当成 legacy-only 交易处理。

Cobuild-aware type script 随后按以下流程验证 message consistency：

1. 首先执行自身原生状态转移验证。Cobuild 不替代该脚本的应用特定有效性规则。
2. 对每个 base 或 append input/output scope 包含当前 type script hash 的 OTX，
   或者 OTX `Message` 中存在指向当前 type script hash 的 `input_type` /
   `output_type` action 的 OTX，type script 可以消费该 OTX `Message`。该
   action target 可以指向不在当前 OTX local cell 范围内的 type script，只要该
   target 在完整交易中真实存在。
3. 对所有 OTX scope 之外的交易 remainder，如果当前 type script hash 出现在相关
   input 或 output 范围内，或者唯一 tx-level `SighashAll.message` 中存在指向当前
   type script hash 的 `input_type` / `output_type` action，type script 可以消费该
   tx-level `Message`。
4. 当消费某个 `Message` 时，type script：
   - 必须只消费通过 `(script_role, script_hash)` 指向自己的 action；
   - 除非自身 ABI 明确定义 multi-action 语义，否则应该拒绝多个匹配 action；
   - 必须根据自身应用规则，针对 action target 以及相关 OTX scope 或
     transaction-remainder scope 中的 cells 验证被消费 action 的 `data`；
   - 对被消费 action data 的结构错误或不一致必须 fail-closed。
   type script 无法在链上取得或验证某个 action 的完整链下 `ScriptInfo`，Core 也不要求
   它在链上验证 `Action.script_info_hash`。wallet 与 reference-flow tooling 负责解析
   `ScriptInfo`、检查 hash、解析 `Action.data`，并把解析后的语义展示给 signer。
5. 如果不存在相关的有效 Cobuild `Message` 或 `Action`，Core 不要求 type script
   失败。type script 可以自行定义更严格的必须存在策略。

## Lock Script 的职责

在 Core 中，lock script 负责：

- 证明当前 owner 授权了相关消费；
- 证明相关的 Cobuild 已签数据未被篡改。

lock script 不负责解释应用特定的 `Action.data` 业务语义。

在 tx-level Cobuild flow 中，lock script 必须：

- 从 `SighashAll` 或 `SighashAllOnly` 中取得 group-leading `seal`；
- 根据交易形态选择 `TxWithMessage` 或 `TxWithoutMessage`；
- 计算标准签名哈希；
- 按其自身加密逻辑验证该 seal。

在 OTX flow 中，lock script 必须：

- 判断自己是否出现在 base input scope、append segment input scope 或两者皆有；
- 在每个相关 seal vector 中为该 script hash 找到且仅找到一个 `LockSeal`；
- 按需计算 `OtxBase` 和 / 或 `OtxAppendSegment`；
- 按其自身加密逻辑验证对应的 seal。

如果存在 tx-level 或 OTX-level 的非空 `Message`，lock script 必须验证其中每个
`Action.script_role + Action.script_hash` 都确实指向完整交易中的某个真实脚本位置：

- `input_lock` 必须匹配至少一个 input lock script hash；
- `input_type` 必须匹配至少一个 input type script hash；
- `output_type` 必须匹配至少一个 output type script hash。

Core 不要求 lock script 解释 `Action.data`。

## Type Script 的职责

在 Core 中，type script 首先始终负责自己的原生状态转移规则。

Cobuild 在其上增加的是一个可选的 message consistency 层。

如果某个 type script 选择消费 `Action`：

- 它只能消费通过 `(script_role, script_hash)` 指向自己的 action；
- 除非其 ABI 显式定义了多 action 语义，否则它应拒绝有歧义的多重匹配 action；
- 一旦它消费了某个 action，就必须在用当前 scope 对其进行验证时 fail-closed。

Core 不要求每个 type script 必须要求 action 存在。
某个 type script 可以将“必须有 action”设为其私有策略。

## 畸形 Witness 处理

使用保留 Cobuild `WitnessLayout` union id 的畸形 witness，对处理该交易的每个
Cobuild-aware 脚本都必须 fail-closed。

- 任何以保留 Cobuild `WitnessLayout` union id（`SighashAll`、
  `SighashAllOnly`、`OtxStart` 或 `Otx`）开头、但无法解析为对应合法 layout
  的 witness，都必须导致 Cobuild witness 扫描失败。
- 多个有效 `OtxStart` witness 必须导致 OTX layout 扫描失败。
- 任何有效 `Otx` witness 出现在唯一连续 OTX 序列之外，都必须导致 OTX layout
  扫描失败。
- 非空 witness bytes 如果不使用保留 Cobuild `WitnessLayout` union id，Core 将其视为
  legacy 或脚本私有 witness bytes。

## 错误模型

Core 标准化失败类别，但不标准化全网统一 numeric error code。

以下 OTX layout 情况必须失败：

- 被选中的 `WitnessLayout` 结构非法；
- 出现多个有效 `OtxStart`；
- `Otx` witness 序列不连续或结构非法；
- `Otx` 满足 `base_input_cells == 0`；
- OTX scope 划分溢出、重叠或不一致；
- `script_role` 的值非法；
- `append_permissions` 的保留位非法；
- append segment flags 的保留位非法；
- 非最后一个 append segment 没有设置 `allow_more_segments_after`；
- 任一 append segment 在某类实体上的 count 非 0，但对应 append-permission bit 为 0；
- mask 长度非法；
- mask 中保留 padding bit 非 0；
- 缺少所需的 `LockSeal`；
- `Otx.base_seals` 内或同一个 `OtxAppendSegment.seals` 内，对同一 `script_hash`
  存在重复 `LockSeal`；
- 签名本身非法或验签失败；
- 在要求唯一性的上下文中出现多个 `SighashAll`；
- 当前脚本所选择消费的精确 Core 哈希 / 选择规则的其他任何失败。

Core 不将“缺少 action”或“缺少 message”定义为通用错误。
这仍由具体脚本策略决定。

## Legacy 共存

Core 允许 legacy `WitnessArgs` 与 `WitnessLayout` 在同一交易中共存。

Core 只保证：

- 两种编码可被区分；
- Cobuild 激活与局部验证选择是确定性的；
- 脚本如果愿意，可以安全地保持 legacy-only。

Core 不要求：

- 每个 lock script 都同时支持 legacy 与 Cobuild；
- 每个 type script 都同时支持 legacy 与 Cobuild；
- 交易里的每个脚本都使用同一种 witness 模式验证。

## 版本化

本文档定义的是 Core v1。

Core v1 遵循以下演进规则：

- 现有字段含义不得被 repurpose；
- 现有 bit 含义不得原地更改；
- v1 中所有保留值 / 保留 bit / 保留 byte 都必须为 0；
- 不兼容的语义变化必须通过新的 witness variant 或新的 table 结构引入；
- 不得通过“放宽解析器”这种方式进行隐式升级。

如果未来协议修订需要不兼容的 OTX 语义，应新增新的 witness variant，
例如 `OtxV2` 一类对象，而不是直接改变 v1 `Otx` 的含义。

## 标准扩展边界

标准扩展可以：

- 定义通用 `Action.data` schema；
- 为特定脚本定义更强的 action 要求；
- 定义 approved-action 及类似高层模式；
- 定义特定领域的 packet 与 batching 约定；
- 定义链下 UX 期望。

标准扩展不得：

- 重定义 Core witness variant；
- 重定义 Core 签名域或哈希构造；
- 重定义 Core 的 Cobuild 激活或局部验证选择规则；
- 重定义 Core 的 lock / type 最小职责边界；
- 要求所有 Core-compatible script 都理解该扩展。

### Approved-Action 的位置

Approved-action 被明确放在 Core 之外，属于标准扩展层。

它可以成为资产与 DeFi 交互的标准 action 家族之一，
但它不是 Core 有效性的前提，Core 也不要求通用脚本天然理解它。

## 参考流程边界

`BuildingPacket`、`OtxBatch`、signer / builder / agent 角色以及 wallet
展示流程属于参考流程层。

它们可以独立于 Core 演进，只要不声称改变了链上有效性规则。

参考流程的修订本身不构成 Core 协议版本升级。

## 迁移指引

对于基于本文档开展的后续实现工作：

- 应将本文档视为规范目标，而不是当前 PoC schema；
- 应将现有 PoC 视为实现参考与迁移输入，而不是真正的规范；
- 应优先编写直接实现 Core v1 的新库与新合约，即使需要为旧 PoC 布局添加
  adapter layer；
- 应将扩展特定逻辑与 Core 的 parsing / hashing 层分离。

## 主要设计决策摘要

- 采用三层模型：Core、标准扩展、参考流程。
- 在 Core 中保留 `Action`，但不把 action 存在性设为 Core 级强制项。
- 给 `Action` 添加 `script_role`，以消除 action 目标位置歧义。
- 将 dynamic OTX 与细粒度签名控制放入 Core。
- 将 OTX 建模为 `base scope + ordered append segments`。
- 强制使用创建者签下的 `append_permissions`，使 append segments 的可用性变成
  明确授权，而不是默认存在。
- 在 Core v1 中只对 base scope 应用细粒度 mask。
- 使用 seal vector 的位置（`base_seals` 或 segment `seals`）决定签名来源。
- 标准化精确的签名前像构造，并使用 BLAKE2b personalization 进行域隔离。
- 保留 `TxWithMessage` 与 `TxWithoutMessage` 作为哈希规则说明术语，而不是
  链上字段。
- 对 Cobuild-aware 脚本使用交易级 Cobuild 激活，同时保持具体验证义务是局部的、
  相关性驱动的。
- 允许 type script 自行决定在缺少 action / message 时是否失败；Core 不强制统一答案。
- 将 approved-action 放入标准扩展层。
- 将 `BuildingPacket` 等流程对象保留在参考流程层，而不是把它们当作有效性规则。
