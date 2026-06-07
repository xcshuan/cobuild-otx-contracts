# Cobuild OTX 测试 Type Scripts 愿景设计

## 状态

本文是 `tests` 目录下 Cobuild OTX 示例 type scripts 的总愿景设计。它只定义测试夹具的目标、边界、交付顺序和每个例子的最小范围，不进入单个合约的具体 spec、implementation plan 或代码实现。

后续每个例子都必须单独完成：

1. 写独立 spec，明确 ABI、状态、action 语义、正反测试矩阵。
2. 写独立 implementation plan，列出测试、合约文件、fixture builder 和验证命令。
3. 测试驱动实现，只把示例合约放在 `tests` 相关目录中。

## 为什么需要这些测试 type scripts

当前仓库已经有 `cobuild-core`、`cobuild-otx-lock` 和 Rust 集成测试，覆盖了 Cobuild witness 布局、签名、OTX base/append scope 以及 lock 侧验证流程。下一步需要 type script 侧的示例夹具，用真实脚本验证 Core 暴露给 type scripts 的语义是否足够稳定、可组合、可测试。

这些示例 type scripts 的目的不是模拟完整业务协议，而是让测试用例能直接回答以下问题：

- `Message.Action` target 是否能正确指向 `input_type` 或 `output_type`。
- type script 能否区分 tx-level action、OTX-level action、base scope 和 append scope。
- base OTX 中声明的 intent 是否能约束 append 方补充的 inputs/outputs。
- 多个 action 修改同一个 type state 时，顺序、状态递进和失败条件是否清晰。
- 与当前 type 无关的 action 是否被忽略，错误 target 是否被 Core 或脚本拒绝。
- Cobuild Core 的 `TypeValidationPlan`、`TypeRelatedAction`、`ActionOrigin`、`OtxTypeRelation` 和 action query 语义是否被集成测试覆盖。

## 总体设计原则

这些合约是测试夹具和示例，不是生产合约。

设计原则：

- 小：每个例子只保留能覆盖 Cobuild OTX 语义的最少状态。
- 清晰：状态 cell data 使用直观字段，action data 使用测试专用 Molecule schema，字段命名直接对应测试意图。
- 可组合：fixtures 能组合 base participant、append participant、tx-level remainder 和 unrelated actions。
- 测试驱动：先写能表达语义的失败测试，再实现刚好足够通过的脚本。
- 不污染生产 crates：示例合约、ABI helper 和测试 builder 放在 `tests` 范围内，不把测试协议提升到 `cobuild-core` 或 `cobuild-types` 的公共协议。
- 不把业务完整性置于 Cobuild 语义之前：例如 limit order 不需要完整订单簿，AMM 不需要生产级定价和 LP 模型。

## 共同测试框架

四个例子应共享一套轻量测试组织方式，但不强行抽象成复杂 SDK。

建议结构在后续 spec 中细化：

- `tests` 下增加测试 type script 合约源码，每个例子一个独立小合约。
- `tests` 下增加 Rust fixture builder，负责部署示例 type script、构造 input/output cells、编码 Cobuild witnesses、运行 `ckb-testtool`。
- 保留现有 `Loader` 风格，从 `build/debug` 或 `build/release` 读取测试合约二进制。
- 每个例子都有单独集成测试文件，避免一个大测试文件承担所有行为。
- 可共享的 helper 只覆盖通用构造：script hash、Cobuild message/action 构造、OTX layout 构造、固定资产 cell、错误断言。

每个示例 type script 在运行时遵循同一模式：

1. 构建 Cobuild context。
2. 调用 type validation plan。
3. 遍历 `related_actions`。
4. 按 `ActionOrigin` 区分 tx-level 与 OTX-level action。
5. 对 OTX action 使用 `OtxMessageLayout` 和 `OtxTypeRelation` 限定本 action 可观察和可约束的 cells。
6. 按本脚本 ABI 解码 `Action.data`。
7. 校验当前 type state 的输入输出转换。

测试应同时覆盖 Core 负责的失败和脚本负责的失败：

- Core 负责：malformed Cobuild witness、非法 action role、action target 不存在、OTX layout 或 append permissions 非法。
- 示例 type script 负责：action data 畸形、不支持的 action union variant、状态转换错误、金额不足、target role 与 ABI 不匹配、重复或缺失必要 action。

## Action 编码建议

`Action.data` 应参考 `ref/repo/spore-contract` 的做法，使用测试专用 Molecule action schema，而不是手写固定二进制 ABI。

参考点：

- `ref/repo/spore-contract/lib/types/schemas/action.mol` 为业务 action 定义 table 与 union。
- `ref/repo/spore-contract/tests/src/utils/co_build.rs` 使用 generated builders 构造 action union，再把 `SporeAction::as_slice()` 放入 Cobuild `Action.data`。
- 链上脚本使用 generated readers 解析 action data，并按 union variant 分派业务语义。

本仓库测试 type scripts 建议采用同一模式：

```text
tests/schemas/test_actions.mol
  table LimitOrderFill { ... }
  table CrowdfundingContribute { ... }
  table NftMintBasic { ... }
  table AmmSwapXForY { ... }
  union TestAction {
      LimitOrderCreate,
      LimitOrderFill,
      LimitOrderCancel,
      CrowdfundingCreateCampaign,
      CrowdfundingContribute,
      CrowdfundingFinalize,
      CrowdfundingRefund,
      NftMintBasic,
      NftMintRare,
      NftMintWithSeed,
      AmmSwapXForY,
      AmmSwapYForX,
      AmmAddLiquidity,
      AmmRemoveLiquidity,
  }
```

通用约束：

- schema 和 generated Rust 代码属于 `tests` 夹具层，不加入 `cobuild-types` 的公共协议 schema。
- 每个示例合约只接受自己关心的 union variants，其他 variants 必须被当作 unrelated action 或 ABI mismatch 处理。
- 数值字段优先使用 Molecule `uint64` 或等价定长 little-endian array，资产或集合 id 使用 `Byte32`。
- 可变 metadata 在 MVP 中用 `data_hash: Byte32`、`metadata_hash: Byte32` 或 `seed: Byte32` 表示，不引入复杂可变长内容结构。
- `script_info_hash` 可使用测试常量，除非某个 spec 明确要验证它。
- action target 的 `script_hash` 必须指向当前示例 type 的 script hash；role 按每个 action 的 ABI 明确为 `InputType` 或 `OutputType`。
- Molecule 解析失败、未知 union variant、字段缺失或字段内容与状态不匹配都必须 fail closed。

使用 Molecule 的好处是 action data 与 Cobuild 生态现有示例保持一致，测试 builder 和链上 reader 都能使用 generated 类型，减少手写 offset/length 解析。schema 必须保持小而扁平，避免为了测试夹具引入完整业务协议。

## Scope 与消息可见性约定

每个示例 spec 都必须明确同一 action 在三种来源下如何表现：

- Tx-level message：action 作用于完整交易中当前 type 相关的 cells。若某个 MVP 只想测试 OTX 行为，可以明确拒绝 tx-level action。
- OTX-level message：action 只解释该 OTX 的 `base_*` 和 `append_*` ranges，不把 tx-level remainder 当成本 action 的 settlement。
- Append scope：append 方补充的 inputs/outputs 可以被 base intent 约束，但不能绕过 append permissions，也不能让 unrelated tx remainder 被计入 OTX 内部状态。

这不是因为 CKB 脚本无法读取完整交易，而是为了让测试夹具主动验证 Cobuild Core 提供的 OTX layout 和 relation 信息是否被正确使用。

## 用例 1：Limit Order

### 愿景

Limit Order 验证 OTX 最核心的 intent + append settlement 场景。用户在 base OTX 中表达“用 token X 换 token Y，成交价格不能低于 limit”，solver 或 counterparty 在 append scope 中补充流动性和 settlement outputs。

### 状态

最小 order state：

- `order_id: [u8; 32]`
- `owner_lock_hash: [u8; 32]`
- `offered_asset_id: [u8; 32]`
- `requested_asset_id: [u8; 32]`
- `offered_remaining: u64`
- `min_requested_per_offered: u64`
- `nonce: u64`

MVP 只要求完全成交。部分成交留到扩展阶段。

### Actions 与 target

- `CreateOrder`：创建 order cell，target role 为 `OutputType`。
- `FillOrder`：消费或更新已有 order cell，target role 为 `InputType`。
- `CancelOrder`：后续扩展，target role 为 `InputType`。

如果第一阶段需要进一步压缩范围，可以先实现 `FillOrder` over pre-existing order cell，把 `CreateOrder` 放进同一例子的第二个小阶段。Limit Order 的核心 OTX 覆盖点是 base order 约束 append settlement，而不是订单创建本身。

### MVP 范围

- 一个 base order input 被完全成交。
- `FillOrder` 出现在 OTX-level message 中。
- append scope 提供 counterparty 的资产输入或 settlement 输出。
- owner 收到的 requested asset 数量必须满足 limit price。
- order output 被移除或 remaining 变为 0。
- unrelated action 不影响当前 order type。

### 后续扩展

- `CreateOrder` output_type creation 测试。
- 部分成交和 remaining amount 递减。
- `CancelOrder` 和 owner 授权。
- 多个 order action 在同一 message 中按顺序处理。

### 必须失败

- requested asset 给 owner 的数量不足。
- 价格低于 limit。
- `FillOrder` 使用 `OutputType` target 或 target hash 不存在。
- action 在 tx-level message 中试图满足 OTX-only settlement，若该 spec 明确只允许 OTX。
- append scope 缺少必要 input/output。
- unrelated action 被错误计入成交。

### 覆盖的 Cobuild 语义

- OTX base intent 约束 append settlement。
- `input_type` action target。
- append inputs/outputs 可组合。
- OTX action 只按当前 OTX layout 解释，不吸收 tx-level remainder。

## 用例 2：Crowdfunding

### 愿景

Crowdfunding 验证多个 append participants 聚合到同一个 campaign state。base OTX 创建或打开 campaign，append scope 中多个 backer 贡献资金，达标后输出 success state 或 reward marker。

### 状态

最小 campaign state：

- `campaign_id: [u8; 32]`
- `creator_lock_hash: [u8; 32]`
- `goal: u64`
- `deadline: u64`
- `raised: u64`
- `status: u8`

MVP 使用简单状态：`status = open | successful`。退款和过期处理后续扩展。

### Actions 与 target

- `CreateCampaign`：创建 campaign state，target role 为 `OutputType`。
- `Contribute`：更新已有 campaign state，target role 为 `InputType`。
- `Finalize`：把 open campaign 转为 successful，target role 为 `InputType`。
- `Refund`：后续扩展，target role 为 `InputType`。

### MVP 范围

- 一个 campaign state input 转换为一个 campaign state output。
- 多个 `Contribute` action 可出现在同一个 OTX message 中。
- 每个 contribution 必须在 OTX append scope 中有对应资金 output 或被 state 增量明确计入。
- `raised` 必须等于旧 `raised` 加上所有有效 contribution。
- `Finalize` 只有在 `raised >= goal` 时通过。
- tx-level remainder 中的资金不能被计入 OTX contribution。

### Tx-level Finalize 用例

Crowdfunding 的 `Finalize` 是本系列推荐的 tx-level message 主用例。它表达的是“项目达标后的最终结算”，适合用 `SighashAll` 携带 tx-level `Message`，而不是继续放在可 append 的 OTX settlement 中。

推荐交易形态：

- tx-level `Message` 同时包含 `CrowdfundingFinalize` 和 NFT reward mint action。
- `CrowdfundingFinalize` target 指向 campaign `input_type`。
- NFT reward mint action target 指向 NFT minter `output_type`。
- 交易消费 open campaign state，输出 successful campaign state。
- 同一笔交易 mint reward NFT，或输出轻量 reward marker。
- 相关 lock 使用 tx-level 全交易签名，覆盖唯一 tx-level message、campaign state transition、reward output、fee/change/remainder。

这个用例验证的不是“一个 type 调用另一个 type”。CKB 中两个 type script 仍然各自运行；tx-level message 是共同协调层，让 campaign type 和 NFT minter type 在同一笔完整交易中消费各自 action，并通过全交易签名保证 finalize 与 reward mint 原子绑定。

必须覆盖的失败面：

- campaign finalize 成功但 reward NFT output 缺失。
- reward NFT mint 存在但 campaign 未达标或未转 successful。
- tx-level message 中 action target role 错误，例如把 reward mint 指向 `input_type`。
- 两个 type action 之一被替换或移除后，相关签名或 type 校验失败。
- OTX-level contribution 或 tx-level remainder 被错误当作 finalize 的新增 raised amount。

### 后续扩展

- `CreateCampaign` 与首次 contribution 同一 OTX。
- deadline 检查。
- `Refund` 和 backer 退款路径。
- reward marker 或 reward NFT mint。
- append permissions 的正反矩阵扩展到 inputs、outputs、cell deps。

### 必须失败

- `Finalize` 时 `raised < goal`。
- contribution action 存在但 state 未增加。
- state 增量大于 append scope 中可证明的 contribution。
- append permissions 不允许追加对应 inputs/outputs。
- 多个 contribution 顺序导致的累计值与输出 state 不一致。
- tx-level remainder 被错误计入 OTX raised。

### 覆盖的 Cobuild 语义

- 多 append participant 聚合。
- OTX append scope 的 inputs/outputs 计数和权限。
- 同一 type state 上多 action 顺序处理。
- tx-level remainder 与 OTX scope 边界。
- tx-level message 被 campaign type 和 NFT minter type 同时消费。
- `input_type` finalize action 与 `output_type` reward mint action 在同一笔全签名交易中原子绑定。

## 用例 3：NFT Minter

### 愿景

NFT Minter 是轻量版 NFT/Spore 风格测试 type。它根据 collection state、mint counter 和 action 参数 mint 出 NFT，用来重点验证 `output_type` action target、counter 递增和一个 message 中多个 mint action。

### 状态

最小 collection state：

- `collection_id: [u8; 32]`
- `mint_counter: u64`
- `supply_cap: u64`

NFT output data 使用测试专用简化结构：

- `collection_id: [u8; 32]`
- `serial: u64`
- `kind: u8`
- `metadata_hash_or_seed: [u8; 32]`

collection state cell 和 NFT output 可以共享同一个测试 minter type，data tag 区分 state cell 与 NFT cell。这样同一个 type script 能在 output group 中验证 minted NFTs，也能在 state cell 上验证 counter。

### Actions 与 target

- `MintBasic`：target role 为 `OutputType`。
- `MintRare`：target role 为 `OutputType`。
- `MintWithSeed`：target role 为 `OutputType`。

### MVP 范围

- 一个 collection state input 变成一个 collection state output。
- 一个 message 中允许多个 mint action。
- NFT output 数量必须等于 mint action 数量。
- `mint_counter` 增量必须等于 mint action 数量。
- minted NFT 的 serial 从旧 counter 开始按 action 顺序递增。
- metadata 或 traits 由 action data 决定。
- `mint_counter + mint_count <= supply_cap`。

### 后续扩展

- 不同 mint action 对 metadata 的更丰富映射。
- rare mint 权限或稀有度限制。
- seed 去重。
- tx-level mint 与 OTX-level mint 的对比测试。
- 与 crowdfunding reward marker 组合。

### 必须失败

- output NFT 数量与 action 数量不一致。
- counter 未递增或递增过多。
- serial 顺序不符合 action 顺序。
- metadata hash 或 seed 与 action data 不一致。
- 超过 supply cap。
- action target 不存在、role 不是 `OutputType`，或 output type hash 不匹配。

### 覆盖的 Cobuild 语义

- `output_type` action target。
- 同一 message 多 action。
- output-only target 与 stateful type 共同验证。
- action data 驱动输出 cell data。

## 用例 4：AMM Swap

### 愿景

AMM Swap 使用一个 pool type script 维护 x/y reserves。每个 swap action 都是针对池子状态的一次状态变更，用来验证多个 action 修改同一个 pool state、action 顺序影响结果、输入输出守恒和 slippage 检查。

### 状态

最小 pool state：

- `pool_id: [u8; 32]`
- `asset_x_id: [u8; 32]`
- `asset_y_id: [u8; 32]`
- `reserve_x: u64`
- `reserve_y: u64`
- `fee_bps: u16`

MVP 不引入 LP token supply。添加和移除流动性留到扩展阶段。

### Actions 与 target

- `SwapXForY`：target role 为 `InputType`。
- `SwapYForX`：target role 为 `InputType`。
- `AddLiquidity`：后续扩展，target role 为 `InputType`。
- `RemoveLiquidity`：后续扩展，target role 为 `InputType`。

### MVP 范围

- 一个 pool state input 转换为一个 pool state output。
- 一个 message 中允许多个 swap action。
- swap action 按 message 顺序逐个更新临时 reserves。
- 每个 action 都检查 `min_output`。
- 最终输出 pool state 必须等于顺序执行后的 reserves。
- unrelated pool action 或其他 pool id action 不影响当前 pool。

MVP 的定价公式可以使用简化 constant product：

```text
amount_in_after_fee = amount_in * (10000 - fee_bps) / 10000
amount_out = reserve_out * amount_in_after_fee / (reserve_in + amount_in_after_fee)
```

该公式仅用于测试，不声称生产经济安全性。

### 后续扩展

- `AddLiquidity`。
- `RemoveLiquidity`。
- LP supply。
- 多 pool 同交易互不干扰。
- 更完整的整数舍入和 dust 失败矩阵。

### 必须失败

- `amount_out < min_output`。
- output reserves 与顺序执行结果不一致。
- 输入资产或输出资产不足以支持 reserves 变化。
- 多个 action 被错误并行处理，忽略顺序影响。
- 其他 pool id 的 action 被计入当前 pool。

### 覆盖的 Cobuild 语义

- 多 action 修改同一 state。
- action 顺序影响状态。
- `input_type` target。
- OTX-level action 与 tx-level action 的可见性边界。

## 和 cobuild-core 当前语义的关系

这些示例 type scripts 应该验证 Core 已经提供或正在提供的语义，而不是重复实现 Core。

Core 负责：

- 解析 Cobuild witness。
- 识别 tx-level message 和 OTX message。
- 验证 OTX layout、append counts 和 append permissions。
- 校验 `Message.Action` target role 与 script hash 是否存在于完整交易范围。
- 为 type script 生成 `TypeValidationPlan`。
- 提供 `TypeRelatedAction`、`ActionOrigin`、`OtxMessageLayout` 和 `OtxTypeRelation`，帮助 type script 理解 action 来源和 OTX 关系。

测试 type scripts 负责：

- 解码自己的 `Action.data`。
- 定义每个 action variant 的 target role。
- 决定 tx-level action、OTX-level action 是否被接受。
- 按 action 顺序验证状态转换。
- 使用 OTX layout 限定本 action 的 settlement 范围。
- 验证自己的业务最小不变量。

如果某个失败可以由 Core 统一拒绝，就不要在示例 type script 中重复设计复杂防线；测试应断言 Core 失败即可。如果失败依赖示例 ABI 或状态，则由对应 type script 拒绝。

## 明确不做什么

本系列不做：

- 生产级限价订单撮合或订单簿。
- 生产级 crowdfunding 资金托管、退款争议和权限模型。
- 完整 NFT/Spore 标准兼容。
- 生产级 AMM 经济模型、LP 份额、安全舍入和价格预言机。
- 完整 token 标准或 UDT 协议实现。
- 复杂 off-chain SDK。
- 把测试 action schema 加入 `cobuild-types` 公共协议 schema。
- 为了减少测试代码而引入难以理解的大型抽象。

## 分阶段交付顺序

### Phase 0：愿景文档

只交付本文档，不改代码。

### Phase 1：Limit Order

交付顺序：

1. 写 Limit Order spec。
2. 写 Limit Order implementation plan。
3. 测试驱动实现最小 type script 和 fixtures。
4. 覆盖完全成交、价格失败、owner 收款不足、append settlement 和 unrelated action。

### Phase 2：Crowdfunding

交付顺序：

1. 写 Crowdfunding spec。
2. 写 Crowdfunding implementation plan。
3. 测试驱动实现 campaign state、contribution 聚合和 finalize。
4. 覆盖多 append participant、append permissions、raised 累计和 tx-level remainder 边界。

### Phase 3：NFT Minter

交付顺序：

1. 写 NFT Minter spec。
2. 写 NFT Minter implementation plan。
3. 测试驱动实现 collection counter 和 output_type mint actions。
4. 覆盖多 mint action、counter、metadata、supply cap 和错误 target role。

### Phase 4：AMM Swap

交付顺序：

1. 写 AMM Swap spec。
2. 写 AMM Swap implementation plan。
3. 测试驱动实现 pool reserves 和 swap actions。
4. 覆盖顺序执行、slippage、reserves 更新、unrelated pool action 和 OTX/tx-level 可见性。

## 每阶段完成标准

每个示例阶段完成时必须具备：

- 独立 spec，明确状态结构、action 编码、target role、scope 行为和失败矩阵。
- 独立 implementation plan，包含具体文件路径、测试步骤、实现步骤和验证命令。
- 最小合约源码只位于 `tests` 相关目录。
- 集成测试可证明 Cobuild Core 语义覆盖点，而不是只证明业务 happy path。
- 反例测试覆盖至少一种 Core 负责失败和一种 type script 负责失败。
- 文档说明哪些扩展被刻意留到后续阶段。

## 设计校验清单

后续写每个例子的 spec 前，应再次逐项确认：

- 这个 type script 维护什么状态。
- 它消费哪些 `Message.Action`。
- 每个 action target 指向 `input_type` 还是 `output_type`。
- tx-level message、OTX base scope、OTX append scope 下如何表现。
- 哪些行为必须失败。
- 哪些测试直接证明 Cobuild Core 的语义被覆盖。
- 哪些业务能力明确不做。
