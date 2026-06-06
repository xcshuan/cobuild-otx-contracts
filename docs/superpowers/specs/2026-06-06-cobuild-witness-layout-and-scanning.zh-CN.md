# Cobuild Witness 编排与扫描规则

本文是对 `2026-05-28-cobuild-core-community-redraft-design.zh-CN.md` 中 witness
规则的展开说明，目的是帮助 review planBuilder 与链上 core 实现。本文不引入新的
协议规则；若与 redraft spec 冲突，以 redraft spec 为准。

这里讨论的重点不是某个具体 lock/type 的签名算法，而是 Cobuild 交易中 witness
应该如何摆放、脚本应该如何扫描、什么情况允许、什么情况必须失败，以及
`CobuildWitnessLayout` envelope 如何与 legacy `WitnessArgs` 在同一笔交易中共存。

## 1. Witness 中有哪几类东西

CKB 交易的 `witnesses` 是一个按索引排列的 bytes vector。Cobuild 在这个 bytes
位置上定义了新的 `CobuildWitnessLayout` envelope，但没有改变 CKB 对 witness 的基本模型：
每个 witness 仍然只是交易里的一个 bytes。

从 Cobuild Core 的角度，扫描 witness 时需要区分以下几类内容。

### 1.1 Legacy witness

Legacy witness 通常是 `WitnessArgs`，也可能是某些脚本私有 ABI 使用的 bytes。
Cobuild Core 不解释它们。

`CobuildWitnessLayout` 使用高位 custom union id，例如 `0xff00_0001` 这一类值，使它在编码
层面可以和 legacy `WitnessArgs` 无歧义地区分。一个 bytes 如果不能被解析成
`CobuildWitnessLayout`，并且开头也不是 Cobuild 当前关心的 custom union id，Cobuild
扫描应该把它当成 legacy 或未知 witness，而不是把整笔交易判错。

这点是 legacy 混合的基础：交易中可以同时存在 legacy witness 和 Cobuild witness。

### 1.2 Tx-level Cobuild witness

Tx-level witness 有两个 variant：

- `SighashAll { seal, message }`
- `SighashAllOnly { seal }`

`SighashAll` 是唯一可以携带 transaction-level `Message` 的 witness。只要某个验证
流程需要 tx-level `Message`，交易里最多只能存在一个有效 `SighashAll`。

`SighashAllOnly` 只携带 seal，不携带自己的 `Message`。当交易中存在唯一有效的
`SighashAll` 时，使用 `SighashAllOnly` 的 signer 也签同一个 tx-level signing hash，
这个 hash 覆盖那一个 `SighashAll.message`。因此在 `TxWithMessage` 场景下，
`SighashAll` 和 `SighashAllOnly` 的差异不是签名对象不同，而是 witness 容器是否携带
message。

当不存在有效 `SighashAll`，而当前 lock group 的首个 witness 是 `SighashAllOnly`
时，可以进入 `TxWithoutMessage`。这表示该 tx-level 签名不绑定 Cobuild
`Message`，但仍按 Cobuild tx-level hash 规则覆盖交易骨架与 unmatched witnesses。

### 1.3 OTX witness

OTX flow 使用两个 variant：

- `OtxStart`
- `Otx`

`OtxStart` 是一个 anchor。它给出第一个 OTX 在 inputs、outputs、cell deps、
header deps 四类实体上的起始索引；它自身所在的 witness 索引也标记 OTX witness
连续段的起点。

每个 `Otx` witness 表示一个 OTX 条目。一个 OTX 条目包含：

- OTX-level `Message`；
- base scope 的实体数量与 mask；
- append scope 的实体数量；
- append 权限；
- 一组 `SealPair`，用于 lock 对 base / append scope 分别签名。

OTX 的实体范围不是由 witness 自己写绝对索引列表，而是从 `OtxStart` 的 anchor
开始，按每个 `Otx` 的 count 逐个累加得到。因此 OTX 在完整交易中占用的是连续的
scope 分区。

## 2. 扫描结果应分成两类逻辑视图

协议不要求把 witnesses 物理扫描两遍，也不要求固定使用两个 collector。更准确的要求是：
witness 扫描的结果应分成两类逻辑视图：

1. tx-level witness 摘要：判断是否存在任意 `CobuildWitnessLayout`，收集
   `SighashAll` / `SighashAllOnly` 摘要，识别唯一 tx-level `Message`。
2. OTX layout 视图：识别 `OtxStart` 与连续 `Otx` 段，计算每个 OTX 的实体范围。

实现可以在同一次顺序读取 witnesses 时解析一次 `CobuildWitnessLayout`，然后同时更新
这两类视图。tx-level 摘要回答：
“这笔交易是否激活 Cobuild？tx-level message 在哪里？当前 lock group 的首个
witness 是否是 tx-level Cobuild carrier？” OTX layout 视图回答：“是否存在合法 OTX
序列？每个 OTX 覆盖哪些 input/output/cell dep/header dep 范围？”

不要把这两类语义混成一个隐式大状态。一个好的实现可以有一个
`CobuildWitnessScanner` 负责物理扫描与解析，但它的输出仍应是边界清楚的 tx-level
摘要和 OTX layout 视图，再让 lock/type 的验证规划基于这些视图选择自己相关的 flow。

## 3. Cobuild 激活规则

对 Cobuild-aware 脚本来说，激活条件是交易中存在任意 `CobuildWitnessLayout`
envelope。
这个条件只看存在性：

- 不要求 `CobuildWitnessLayout` 唯一；
- 不要求它一定是 `SighashAll`；
- 不要求所有 Cobuild witness 都已经通过完整 Core 校验；
- 不要求交易里的每个脚本都支持 Cobuild。

如果完全没有 `CobuildWitnessLayout`，Cobuild-aware 脚本可以按自己的 legacy 规则验证。

如果至少存在一个 `CobuildWitnessLayout`，该 Cobuild-aware 脚本必须在 Cobuild Core
规则集下评估与自己相关的验证义务。它不能因为“当前 script group 的 witness 看起来
像 legacy”或“当前 action 跟我无关”就把整笔交易当成 legacy-only 交易处理。

这条规则只约束 Cobuild-aware 脚本。交易中的 legacy-only 脚本仍然按自己的 legacy
规则运行。CKB 共识只关心每个脚本执行是否通过，不要求同一笔交易里所有脚本使用同一
种 witness 解析模式。

## 4. Tx-level witness 扫描

Tx-level 扫描应该逐个 witness 做轻量分类：

- 空 bytes：记为空 witness；
- 可解析为 `SighashAll`：记录其 `Message` cursor，标记为 tx-level carrier；
- 可解析为 `SighashAllOnly`：标记为 tx-level carrier；
- 可解析为其他 `CobuildWitnessLayout`：标记 Cobuild 已出现，但不当作 tx-level carrier；
- 不能解析为 `CobuildWitnessLayout`，且开头不是保留 Cobuild union id：当作 legacy；
- 开头是保留 Cobuild union id（`SighashAll`、`SighashAllOnly`、`OtxStart`
  或 `Otx`）但结构畸形：扫描阶段直接 fail-closed。

这里的“carrier”只是指某个 lock group 首位用来放 tx-level seal 的 witness。它不是
全局唯一概念。多个 lock group 可以各自有自己的 tx-level carrier witness，其中最多
一个可以是 `SighashAll`，其他需要 tx-level seal 的 group 通常使用
`SighashAllOnly`。

### 4.1 唯一 `SighashAll`

当验证流程需要 tx-level `Message`，或者需要决定 `TxWithMessage` / `TxWithoutMessage`
时，必须扫描所有 witness 中的有效 `SighashAll`：

- 没有有效 `SighashAll`：没有 tx-level `Message`；
- 恰好一个有效 `SighashAll`：其 `message` 是唯一 tx-level `Message`；
- 多个有效 `SighashAll`：失败；
- 扫描范围内存在使用保留 Cobuild union id 的畸形 witness：失败。

`SighashAllOnly` 不参与唯一 message 的竞争，因为它不携带 message。

### 4.2 当前 lock group 的 tx-level carrier

对一个 lock script 来说，tx-level seal 只在“当前 lock 存在 OTX scope 之外的
remainder inputs”时才需要。若需要 tx-level seal，则当前 lock group 的首个 witness
必须是有效的 `SighashAll` 或 `SighashAllOnly`。

“当前 lock group 的首个 witness”应该按 CKB 的 group input 语义理解，也就是
`Source::GroupInput` 下 group index 为 0 的 witness。对于当前 lock group 的其他
group inputs，对应 witness 必须不存在或为空，除非该 lock 自己的非 Cobuild ABI
明确允许额外数据，并且这些额外数据仍被该 lock 的 Cobuild 签名规则覆盖。

这条规则保留了 legacy sighash-all 的核心约束：同一 lock group 只在首个位置放签名
数据，其他同组 witness 不能悄悄携带未被签名语义约束的数据。

## 5. OTX layout 扫描

OTX 扫描不关心 `SighashAll` / `SighashAllOnly` 的 seal。它只关心
`OtxStart` 和 `Otx`。

推荐扫描流程如下：

1. 从 witness index 0 开始顺序扫描。
2. 空 witness、legacy witness、tx-level Cobuild witness、其他非 OTX
   `CobuildWitnessLayout` 都可以忽略。
3. 如果遇到有效 `OtxStart`：
   - 若之前已经见过有效 `OtxStart`，失败；
   - 记录它的 witness index 和四类实体起始索引；
   - 后续有效 `Otx` 必须从下一个 witness index 开始连续出现。
4. 如果遇到有效 `Otx`：
   - 若还没有 `OtxStart`，失败；
   - 若它不是紧跟在 `OtxStart` 或前一个 `Otx` 之后，失败；
   - 记录 `(witness_index, OtxView)`。
5. 如果某个 bytes 开头是 OTX 相关 union id，但不能解析成合法 `OtxStart` 或
   `Otx`，失败。
6. 扫描结束后：
   - 没有 `OtxStart`：表示没有 OTX flow；
   - 有 `OtxStart` 但没有任何连续 `Otx`：失败；
   - 对每个 `Otx` 验证内部结构，并用 range cursor 累加得到 scope；
   - 累加结果不能超过交易中对应实体总数。

这个流程的关键点是：有效 `Otx` 只能出现在唯一的连续 OTX 段里。不能在
`OtxStart` 之前放一个 `Otx`，不能在连续段结束后又放一个 `Otx`，也不能只有
`OtxStart` 没有 `Otx`。

### 5.1 OTX 连续段示例

允许：

```text
witness[0] = legacy WitnessArgs
witness[1] = WitnessLayout::OtxStart
witness[2] = WitnessLayout::Otx
witness[3] = WitnessLayout::Otx
witness[4] = empty
witness[5] = WitnessLayout::SighashAll
```

这里 OTX 段是 `[1, 3]`：`OtxStart` 在 1，连续 `Otx` 在 2 和 3。后面的
`SighashAll` 不是 OTX witness，不破坏已经结束的 OTX 段。

不允许：

```text
witness[0] = WitnessLayout::Otx
witness[1] = WitnessLayout::OtxStart
witness[2] = WitnessLayout::Otx
```

原因是 `Otx` 出现在 `OtxStart` 之前。

不允许：

```text
witness[0] = WitnessLayout::OtxStart
witness[1] = WitnessLayout::Otx
witness[2] = legacy WitnessArgs
witness[3] = WitnessLayout::Otx
```

原因是 `Otx` 段被 legacy witness 打断后又出现了新的 `Otx`。

不允许：

```text
witness[0] = WitnessLayout::OtxStart
witness[1] = WitnessLayout::SighashAll
```

原因是有 `OtxStart` 但没有任何 `Otx`。

### 5.2 OTX scope 如何从 counts 得到

假设 `OtxStart` 指定：

```text
start_input_cell = 2
start_output_cell = 1
start_cell_deps = 0
start_header_deps = 0
```

第一个 `Otx` 的 counts 是：

```text
base_input_cells = 1
append_input_cells = 2
base_output_cells = 1
append_output_cells = 0
```

那么第一个 OTX 的 input scope 是：

```text
base_inputs   = [2, 3)
append_inputs = [3, 5)
```

output scope 是：

```text
base_outputs   = [1, 2)
append_outputs = [2, 2)
```

第二个 `Otx` 会从上一个 OTX 消费结束的位置继续累加。也就是说，如果第一个 OTX
已经把 next input 推到 5，第二个 OTX 的 base input 就从 5 开始。

因此 planBuilder 不应该把 OTX scope 理解成每个 OTX 自带的绝对范围列表。正确模型是
一个 `LayoutRangeCursor`：从 `OtxStart` 的四个起点开始，每处理一个 OTX 就
`take_range(count)`。

### 5.3 OTX 内部结构错误

有效 OTX 还必须满足内部结构约束。典型约束包括：

- base input 数量不能为 0。Core v1 不允许只有 append 授权、base 本身无人签名的
  空壳 OTX；
- append counts 必须被 `append_permissions` 允许；
- base input/output/cell dep/header dep masks 的长度与未使用 bit 必须合法；
- `SealPair.scope` 只能是 base 或 append；
- counts 累加不能溢出，也不能超过交易实际实体数量。

这些错误属于 OTX layout 错误。对处理该交易的 Cobuild-aware 脚本，应 fail-closed。
不要把它们记录成“无效但可忽略的 OTX 段”，否则 planBuilder 很容易在后续相关性判断
里出现“当前脚本刚好不相关所以放过畸形 OTX”的错误。

## 6. Remainder 与 OTX-covered 范围

OTX 序列把完整交易中的一段实体范围划给 OTX。对每类实体，remainder 是两段范围的
并集：

```text
[0, otx_start) 以及 [otx_end, entity_count)
```

这里的 `otx_start` 是 `OtxStart` 对应实体的起始索引，`otx_end` 是所有连续 OTX
累加后的结束索引。

对于 lock script，tx-level remainder 重点看 inputs。如果当前 lock hash 在这些
remainder input 中出现，说明该 lock 还有不属于相关 OTX-covered input 的消费，
因此需要 tx-level seal。

对于 type script，tx-level remainder 可以看 inputs 和 outputs。如果当前 type hash
出现在 OTX 之外的相关 cell 中，或者 tx-level message 里有指向当前 type hash 的
action，则该 type 可以消费 tx-level `Message` 并按自己的业务规则校验。

## 7. Lock script 如何选择 flow

对当前 lock script，Cobuild flow 不是三选一，而是可能同时包含多个义务：

- 若当前 lock hash 出现在一个或多个 OTX 的 base input scope 中，需要为每个相关
  OTX 验证 base seal；
- 若当前 lock hash 出现在一个或多个 OTX 的 append input scope 中，需要为每个相关
  OTX 验证 append seal；
- 若当前 lock hash 还出现在 OTX scope 之外的 remainder inputs 中，需要验证一个
  tx-level seal。

因此一个 lock 的执行可能同时验证多个 OTX seal 加一个 tx-level remainder seal。

### 7.1 OTX lock 签名义务

OTX lock 签名义务由“当前 lock hash 是否真实出现在该 OTX 的 input scope 中”决定：

- 出现在 base input scope：需要 `(lock_hash, base)` 的 `SealPair`；
- 出现在 append input scope：需要 `(lock_hash, append)` 的 `SealPair`；
- 同时出现在 base 与 append：两个 seal 都需要；
- 只是在 OTX `Message` 的 `input_lock` action 中被提到，但不在该 OTX input scope
  中：该 message 与当前 lock 相关，当前 lock 可以检查 action target 存在性，但不
  因此产生 OTX 签名义务。

最后一点很重要。action 可以调用完整交易中真实存在的其他 lock/type，即使目标脚本
不在当前 OTX local scope 里。action 相关性与签名义务不能混为一谈。

### 7.2 Tx-level lock 签名义务

如果当前 lock 有 remainder inputs，则必须检查当前 lock group 的首个 witness。

允许：

```text
input[0] lock = L
input[1] lock = L
input[2] lock = M

witness[0] = WitnessLayout::SighashAll
witness[1] = empty
witness[2] = WitnessLayout::SighashAllOnly
```

这里 lock `L` 的 group-leading witness 是 `witness[0]`，lock `M` 的 group-leading
witness 是 `witness[2]`。如果 `witness[0]` 携带唯一 tx-level `Message`，
`witness[2]` 的 `SighashAllOnly` 仍签同一个 `TxWithMessage` hash。

不允许：

```text
input[0] lock = L
input[1] lock = L

witness[0] = WitnessLayout::SighashAll
witness[1] = non-empty legacy bytes
```

原因是同一 lock group 的非首位 witness 非空。除非 lock 自己明确声明这类额外数据
属于它的 Cobuild ABI 且被签名规则覆盖，否则 Core 默认应该拒绝。

## 8. Type script 如何选择 message

Type script 的 Cobuild 职责是 message consistency，而不是 owner signature。

它首先仍然要执行自己的原生状态转移验证。Cobuild 不替代 UDT、NFT、DAO 或其他 type
脚本的应用规则。

在 Cobuild 激活后，type script 可以从以下来源消费 message：

- 某个 OTX 的 base 或 append input/output scope 包含当前 type hash；
- 某个 OTX `Message` 中存在指向当前 type hash 的 `input_type` 或 `output_type`
  action；
- OTX 之外的 transaction remainder 中包含当前 type hash，并且存在唯一
  tx-level `SighashAll.message`；
- 唯一 tx-level `SighashAll.message` 中存在指向当前 type hash 的
  `input_type` 或 `output_type` action。

action target 可以指向不在当前 OTX local cell 范围内的 type script，只要该 target
在完整交易中真实存在。也就是说，OTX A 的 message 可以要求完整交易中另一个不在
OTX A scope 里的 type script 做检查。planBuilder 不能只在当前 OTX local scope 里
查 action target。

Core 不强制每个 type script 必须找到 action。某个 type 是否要求 action 存在，是
该 type 的私有策略。但一旦它选择消费某个 action，就应该 fail-closed：

- 只消费 `(script_role, script_hash)` 指向自己的 action；
- 除非 ABI 明确定义多 action 语义，否则拒绝多个匹配 action；
- action data 结构错误、语义不一致或与相关 scope 不匹配时失败。

## 9. Action target 的全交易存在性检查

对当前脚本消费或用于签名验证的每个 tx-level 或 OTX-level `Message`，必须验证其中
所有 action target 在完整交易中真实存在：

- `input_lock` 必须匹配至少一个 input lock script hash；
- `input_type` 必须匹配至少一个 input type script hash；
- `output_type` 必须匹配至少一个 output type script hash。

这里强调“完整交易”，不是当前 OTX scope。原因是 action 的目标是脚本角色与脚本
hash，不是 OTX local index。OTX local scope 决定签名覆盖和局部状态验证范围；
action target 存在性则防止 message 引用一个交易里根本不存在的脚本。

实现上，planBuilder 可以在扫描交易实体时建立 script hash 索引：

- lock hash -> input indices；
- input type hash -> input indices；
- output type hash -> output indices。

这样既能回答“目标是否存在”，也能回答“某个 lock/type 是否出现在某个 OTX range 或
remainder range 中”。索引可以直接从 syscall reader 扫描时构建，不需要先缓存
`input_locks` / `input_types` / `output_types` 三组完整 raw vector 再二次建索引。

## 10. Cobuild 与 legacy 如何混合

Cobuild 与 legacy 的混合要分清两个层面：交易层面可以混合，脚本执行层面必须明确
选择自己的验证模式。

允许的混合：

```text
input[0] 使用 legacy-only lock
input[1] 使用 Cobuild-aware type

witness[0] = WitnessArgs
witness[1] = WitnessLayout::SighashAll
```

legacy-only lock 不认识 `CobuildWitnessLayout`，但如果它的 legacy sighash 规则覆盖了
unmatched witnesses，仍可能把 `witness[1]` 的 bytes 签进去。Cobuild-aware type
可以读取 `witness[1]` 中的 `Message`，找到指向自己的 action 并验证状态转移。

也允许：

```text
input[0] lock = Cobuild-aware L
input[1] lock = legacy-only M

witness[0] = WitnessLayout::SighashAll
witness[1] = WitnessArgs
```

lock `L` 按 Cobuild flow 验证。lock `M` 按 legacy flow 验证。交易不要求两者使用
同一种 witness 模式。

不允许的是 Cobuild-aware 脚本看到交易里存在 `CobuildWitnessLayout` 后，为了绕过
Cobuild 相关错误而 fallback 到 legacy-only。例如：

- 交易中有畸形 `OtxStart`；
- 当前 Cobuild-aware lock 恰好没有 OTX 签名义务；
- 实现于是忽略 OTX layout 错误，转去 legacy flow。

这不允许。OTX layout 错误对处理该交易的 Cobuild-aware 脚本必须 fail-closed。

## 11. 允许与禁止的快速清单

允许：

- 同一交易中同时有 legacy `WitnessArgs` 和 `CobuildWitnessLayout`；
- legacy-only 脚本和 Cobuild-aware 脚本在同一交易中共存；
- 多个 lock group 各自使用 tx-level carrier witness；
- 唯一一个 `SighashAll` 携带 tx-level `Message`，其他 tx-level signer 使用
  `SighashAllOnly`；
- 没有 `SighashAll`，但某个 lock group 使用 `SighashAllOnly` 进入
  `TxWithoutMessage`；
- OTX 连续段前后出现 legacy witness、空 witness 或 tx-level Cobuild witness；
- action target 指向完整交易中存在、但不在当前 OTX local scope 内的 lock/type；
- type script 根据自己的 ABI 决定缺少 action 时是否失败。

禁止：

- 在需要唯一 tx-level `Message` 的流程里出现多个有效 `SighashAll`；
- 当前 lock 有 tx-level remainder inputs，但 group-leading witness 不是有效
  `SighashAll` / `SighashAllOnly`；
- 同一 lock group 的非首位 witness 非空且没有被该 lock 的 Cobuild ABI 明确覆盖；
- 多个有效 `OtxStart`；
- 有 `OtxStart` 但没有任何 `Otx`；
- `Otx` 出现在 `OtxStart` 之前；
- `Otx` 不连续，或者在 OTX 连续段结束后再次出现；
- OTX counts 溢出或超过交易实体总数；
- OTX base input 数量为 0；
- append counts 超出 `append_permissions`；
- mask 长度或未使用 bit 非法；
- `SealPair.scope` 不是 base/append；
- action target 在完整交易中不存在；
- Cobuild-aware 脚本在交易已激活 Cobuild 后通过 fallback legacy-only 忽略相关
  Cobuild 错误。

## 12. 对 planBuilder 的 review 要点

review planBuilder 时，可以按下面的问题检查实现是否贴合 witness 规则。

### 12.1 扫描阶段是否外显

好的结构应该能明确看到：

- 先读取交易 counts；
- 顺序扫描 witnesses；
- 通过 `CobuildWitnessScanner` 解析一次 `CobuildWitnessLayout`；
- 同时构建 `WitnessScan` 一类的 tx-level 摘要和 `OtxLayoutScan` 一类的 OTX layout
  视图；
- 扫描交易实体并建立 script hash -> indices 索引；
- 后续 lock/type 规划只消费这些视图，不重新隐式扫描一遍。

如果 planBuilder 在多个函数里反复从 syscall 读取同一类 witness，或者在相关性判断
时才临时解析 OTX layout，说明流程还不够外显。

### 12.2 是否避免缓存所有 witness bytes

链上实现不应该为了方便而缓存 `Vec<Vec<u8>>` 形式的全部 witnesses。更合理的方式是：

- 扫描时只把当前 witness 读成 bytes；
- tx-level 扫描保存摘要或 `Cursor`；
- OTX 扫描只保存需要参与后续 layout/hash 的 `OtxView`；
- hash 某些 unmatched witness 时，再按绝对 index 读取对应 witness。

这样能减少内存峰值，也避免“为了后面可能用到”而把整笔交易 witness 全量复制。

### 12.3 OTX 错误是否直接返回

Cobuild witness layout 错误不应该以 `Invalid` 状态在多个对象之间传递。更清晰的方式是：

- `push_witness()` 发现保留 Cobuild union id 对应的畸形 witness、重复 start、
  非连续 OTX 时直接返回 error；
- `finish()` 发现只有 start 没有 OTX、内部结构非法、range 越界时直接返回 error；
- 调用方看到 error 后 fail-closed。

如果代码里存在“先记录 invalid，后面看当前脚本是否相关再决定是否报错”的流程，就要
特别警惕。这容易把全局 OTX 编排错误变成局部可忽略错误。

### 12.4 是否把 action 相关性和签名义务分开

lock 的 OTX 签名义务来自 lock hash 出现在 OTX input scope 中，不来自 action。
action 指向当前 lock 只说明该 message 与当前 lock 相关，需要做 target 存在性等
检查，但不能凭空要求当前 lock 对一个不包含自己 input 的 OTX 签名。

type 的 message 消费相关性可以来自 scope，也可以来自 action。并且 action target
存在性要查完整交易，不是只查当前 OTX local scope。

如果 planBuilder 用一个布尔值同时表达“message 相关”和“必须签名”，这个抽象很可能
太粗。

### 12.5 是否只在需要时使用绝对 index

OTX hash 内部很多字段使用 OTX-local slot index，这是为了让 OTX 不绑定到最终完整
交易的绝对位置。但链上脚本在完整交易中验证时，仍然需要绝对 index 来：

- 从 syscall 读取 input/output/cell dep/header dep；
- 判断某个 script hash 是否落在某个 OTX range 或 remainder range；
- 检查 action target 是否在完整交易中真实存在；
- 读取 `inputs_len..witnesses_len` 的 unmatched witnesses 做 tx-level hash。

因此 planBuilder 不应该完全隐藏绝对 index。更好的方式是：layout 层暴露
`Range { start, count }`，hash 层在需要时把 local index 转成 tx index。

### 12.6 是否直接构建全量 script hash indices

为了判断当前 lock/type 是否出现在某些范围里，planBuilder 需要 script hash 索引。
但不需要为全交易的所有 script hash 都保存 indices。更清晰的做法是在 context
创建时就传入当前脚本类型和 hash，然后只构建当前脚本的 indices。

推荐数据形状是：

```text
CurrentScriptIndices =
  Lock { input_indices: Vec<usize> }
  | Type {
      input_indices: Vec<usize>,
      output_indices: Vec<usize>,
    }
}
```

当前 lock/type 的 range 判断、remainder 判断、lock group witness 检查都只需要当前
脚本 indices。

但 action target validation 不能只看当前脚本。对当前脚本消费或用于签名验证的完整
`Message`，仍然必须针对完整交易验证所有 action target 是否真实存在。因此这部分
应该按需扫描完整交易：

- `input_lock` action：扫描 input lock hashes；
- `input_type` action：扫描 input type hashes；
- `output_type` action：扫描 output type hashes。

这样 context 里不需要保存非当前脚本的 indices，同时仍然满足完整交易 target
existence 语义。

### 12.7 命名是否贴近协议语义

命名应该直接表达协议概念：

- `OtxView` 比 `otx_witness` 更好，因为后续关心的是解析后的 OTX 视图；
- `carrier_witness_index` 若用于当前 lock group，应在上下文里说明它是
  group-leading tx-level seal carrier；
- `WitnessScan` 若只关心 tx-level sighash 摘要，应避免让读者误以为它完成了所有
  witness layout 校验；
- `LayoutRangeCursor` 比一组零散的 `take_input` / `take_output` helper 更能表达
  “按 counts 连续消费 range”。

好的命名能让读者在不读实现细节时就知道这段代码对应 spec 哪一段流程。

## 13. 推荐的整体数据流

一个清晰的 planBuilder 可以按下面的数据流组织：

```text
SyscallTxReader
  -> preload counts
  -> CobuildWitnessScanner scans witnesses once
       -> WitnessScan
       -> OtxLayoutScan::None | OtxLayoutScan::Complete
  -> scan current script indices from CurrentScript { role, hash }
       -> CurrentScriptContext
  -> build CobuildContext for the current script
  -> for current lock/type:
       -> select relevant OTX messages
       -> select tx-level remainder message/seal
       -> validate action targets by scanning the full transaction on demand
       -> produce signature/message validation plan
```

这条流里的每一步都应该有明确输入和输出。尤其是：

- counts 是 reader 内部缓存，不需要在函数之间反复传；
- witness 扫描不应该偷偷读取交易实体；
- script hash 索引不应该依赖 witness layout；
- OTX layout 错误应该在 layout 扫描阶段暴露；
- lock/type 的相关性判断应该只消费已经构建好的 context。

这样的实现更容易 review，也更接近协议本身的分层：witness 编排、scope 计算、
script hash 相关性、签名/message 验证，各自边界清楚。
