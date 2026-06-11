# Cobuild 全面测试与解耦 Fixture Framework 重构分析

## 背景

本分析基于以下资料和当前代码：

- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- `docs/CobuildAgentDevelopGuide.md`
- `crates/cobuild-core/src/{layout,view,context,engine,hash}`
- `contracts/cobuild-otx-lock/src`
- `tests/src/framework`
- `tests/src/fixtures`
- `tests/tests`

目标不是简单增加用例数量，而是重构出一套抽象、全面、解耦的测试体系：测试矩阵必须覆盖协议边界、签名域、OTX 范围、action target、业务约束和跨脚本组合；底层 framework 必须让这些测试写起来短、稳定、可读，并且能方便构造负例。

本设计不以历史兼容为约束。现有 `tests/src/framework` 和 `tests/src/fixtures` 可以为了更好的抽象边界、可组合性和安全覆盖被重组、重命名、拆目录或替换 API。现有测试可以迁移到新模型，不需要维持旧 helper 的调用方式。

## 实施状态

状态：已完成，日期 2026-06-11。

对应实现分支：`test-framework-fixtures-refactor`，worktree：`.worktrees/test-framework-fixtures-refactor`。

最终实现结果：

- `framework` / `fixtures` 边界已落地；`framework` 不依赖 fixture、named test contracts 或 limit-order 业务语义。
- `framework` 已集中提供 Cobuild protocol builders、`TxShape` / `BuiltTxShape`、typed handles、resolved input facts、signing hash oracle、signing facts、expected outcome 和协议/交易形态 mutation。
- `fixtures/common` 已承接 named contract catalog、test assets 和 fixture 级 contract helpers。
- `fixtures/cobuild_otx_lock`、`fixtures/limit_order` 已承接具体业务场景、error catalog、coverage tags 和 expected outcomes。
- `cobuild_otx_lock`、`limit_order_type`、`limit_order_lock` integration tests 已迁移为 table-driven runner。
- `limit_order_lock` 已从旧 `LockFillCase` 大分支迁移为 `LockScenario` 数据模型，场景由 happy path、`BusinessMutation`、expected error、coverage 和 mutation fields 驱动。
- 旧 `OtxTransactionBuilder` 和返回裸 `CellInput` 的 stale limit-order builder 已删除。

验收命令均已通过：

- `cargo test -p tests --offline --test cobuild_otx_lock`
- `cargo test -p tests --offline --test limit_order_type`
- `cargo test -p tests --offline --test limit_order_lock`
- `cargo test --workspace --offline`

## 目标原则

- 抽象优先：测试只描述安全意图和业务场景，不手写交易拼装细节。
- 全面覆盖：所有协议规则都有正例、负例、边界例和至少一个端到端代表例。
- 解耦分层：CKB 原语、Cobuild 协议、签名/hash、交易变异、业务 fixture、断言各自独立。
- 组合驱动：用 scenario、mutation、expected outcome 组合测试矩阵，避免复制粘贴。
- 负例一等公民：framework 必须像支持成功路径一样支持畸形 witness、非法 mask、错误 permission、签名后变异。
- 安全语义显式：测试名、case 名、assertion 都要表达失败属于 Core、verifier 还是业务规则。

## 归属边界

这次重构最重要的边界是：`framework` 不是所有测试 helper 的归宿。它只放 Cobuild/CKB 测试底座；凡是跟 `tests/contracts`、`tests/vendor` 里的具体测试合约有关的能力，即使抽象得很通用，也放在 `fixtures`。

### 放在 framework

- Cobuild witness/message/action/OTX/seal 的构造和畸形构造。
- OTX layout、tx shape、witness sequence、mutation 这类协议级交易能力。
- signing hash oracle、signing facts、scope signing。
- resolved input facts、typed handles、index maps。
- 不含业务语义的 CKB cell/tx/script 基础结构。
- 不认识任何具体测试合约名字的基础部署原语。

### 放在 fixtures

- `limit-order-type`、`limit-order-lock`、`test-udt`、`test-nft`、proxy locks、always-success 等 named contracts 的部署和组合。
- Owner、Buyer、FeePayer、WrongOwner、OrderLockOwner 等 persona。
- UDT/NFT/test asset factory。
- Limit Order 的 action builder、state builder、error catalog、scenario、mutation。
- 与测试合约 exit code、业务语义、业务 asset layout 相关的一切抽象。

判断规则：如果一个 helper 的名字、字段或默认值需要知道某个测试合约、测试资产或业务角色，它属于 `fixtures`；如果它只知道 Cobuild Core 或 CKB 交易结构，它才属于 `framework`。

## 当前测试现状

已有测试覆盖了很多关键路径：

- `crates/cobuild-core/tests` 覆盖 view、plan、旧 API 边界和 entity 依赖隔离。
- `contracts/cobuild-otx-lock/tests` 覆盖 args、error mapping、verifier trait。
- `tests/tests/cobuild_otx_lock.rs` 覆盖 tx-level 签名、OTX base/append 签名、混合 tx-level + OTX、坏 seal、malformed witness、malformed OTX layout、两个 OTX。
- `tests/tests/limit_order_type.rs` 和 `tests/tests/limit_order_lock.rs` 覆盖 limit-order 的 NFT-for-UDT、create-order、payment binding、跨 OTX 支付、重复支付 output、业务 action 负例。
- `tests/src/framework` 已经把合约部署、cell、Cobuild message、OTX builder、tx builder、assertion 拆成模块。
- `tests/src/fixtures/limit_order` 已经把大量业务场景收敛成 enum case。

主要缺口在于：当前 framework 更擅长写“规范成功路径”和少量业务负例，但没有把协议场景、交易实体分段、签名事实、变异操作和业务语义解耦。对 Cobuild Core 的协议畸形交易、签名 preimage 变异、mask/permission 边界、witness 顺序变异、跨 OTX 组合变异支持不够直接。结果是某些安全测试要手写低层 Molecule 或直接复制 fixture 逻辑，成本高，容易漏测。

## 从当前测试暴露出的别扭点

### 1. 魔法索引散落在业务 fixture 中

`limit_order_lock_nft_for_udt_case_with` 里直接设置 `payment_output_index = 1/2/3`，并通过 `append_output`、`remainder_output`、第二个 OTX 的 append output 数量来间接制造“当前 OTX / 另一个 OTX / remainder”的差异。这说明当前 framework 没有暴露稳定的 output handle。

目标抽象不应让业务 fixture 猜索引。应由 `TxShape` 返回强类型句柄：

```rust
let payment = shape.otx(0).append_output("payment");
let other_payment = shape.otx(1).append_output("payment");
let remainder = shape.remainder_output("payment");
```

业务 action 绑定 payment 时传 handle，最后由 framework 解析成 OTX-local 或 tx-global index。这样可以避免改动 tx layout 后测试静默指向错误 output。

### 2. 一个 enum case 同时承担场景选择和故障注入

`LimitOrderLockFillCase` 同时表示：

- 基础合法路径：单订单、双订单、混合 type/lock。
- 业务字段变异：wrong UDT、wrong owner、insufficient amount。
- 协议位置变异：order input in append scope、payment in another OTX、tx-level fill order。
- 数据编码变异：malformed args、unknown action tag、malformed action。

这导致 fixture 里出现大量 `if matches!(case, ...)` 和 `match case`，每新增一个 case 都要检查多个分支是否交叉影响。目标模型应拆成：

- `HappyPath`：合法交易骨架。
- `Mutation`：破坏哪个不变量。
- `ExpectedOutcome`：预期失败位置和错误类别。

这样 `WrongUdt` 不会被迫关心 OTX 布局，`PaymentInAnotherOtx` 不会被迫关心 UDT 金额。

### 3. 签名生成与交易构造耦合太紧

`cobuild_otx_lock` fixture 里反复手写：

- 固定私钥、公钥 hash、lock args。
- 部署 lock contract。
- 构造 unsigned tx。
- 调用 test helper 计算 hash。
- 生成 seal。
- 替换 witness。

OTX 签名还依赖 `fixtures/otx_hash.rs` 这个 fixture-local mirror。这个结构会让签名测试很难组合变异，也很难复用到其他合约。

目标模型应把“谁签、签哪个 scope、hash 是什么、seal 放在哪个 witness”放进 `SigningModel`。业务或协议 fixture 只声明：

```rust
scenario.sign(lock_owner).for_otx_base(otx0);
scenario.sign(lock_owner).for_otx_append(otx0);
scenario.sign(fee_owner).for_tx_level();
```

framework 负责生成 signing hash、seal pair、carrier witness，并记录 `SigningFacts`。

### 4. resolved input 事实没有成为交易形态的一部分

`live_input` 只返回 `CellInput`，会丢掉 resolved `CellOutput` 和 data；`live_resolved_input` 虽然存在，但没有成为 tx builder 的默认事实来源。签名 hash、OTX base/append hash、安全变异都需要 resolved input output/data。

目标模型中 `TxShape` 或 `BuiltScenario` 必须保存完整 `ResolvedInputFacts`：

```rust
pub struct ResolvedInputFacts {
    pub input: CellInput,
    pub output: CellOutput,
    pub data: Bytes,
    pub lock_hash: [u8; 32],
    pub type_hash: Option<[u8; 32]>,
}
```

`SigningHashOracle` 应优先使用这些 facts，而不是要求每个测试重新传 `resolved_output.as_slice()` 或手动拼 input 数组。

### 5. `OtxTransactionBuilder` 把 layout policy 写死

当前 builder 固定：

- `start_input_cell = 0`
- `start_output_cell = 0`
- `start_cell_deps = self.cell_deps.len()`
- `start_header_deps = 0`
- 先全部 base inputs，再全部 append inputs
- 不支持 scoped header deps
- append/base cell deps 只能通过总 cell deps 间接表达，且 append cell deps 被断言为 0

这对成功路径很方便，但不能作为全面安全测试的底座。协议测试必须能构造非零 start、prefix/suffix、header dep、scoped cell dep、非连续 witness、range 越界。目标 builder 应以 `TxShape` 为中心，而不是以当前 limit-order 成功交易顺序为中心。

### 6. expected outcome 写在测试文件里，fixture 不自描述

当前测试文件需要知道每个 case 应该断言哪个 input、哪个 script、哪个 exit code。fixture 返回 `(fixture, tx)`，没有返回“为什么应该失败”。这让 case 与 assertion 分离，新增或调整 case 时容易漏改测试。

目标 fixture 应返回：

```rust
pub struct BuiltCase {
    pub fixture: CobuildTestFixture,
    pub tx: TransactionView,
    pub facts: ScenarioFacts,
    pub expected: BusinessExpectedOutcome,
    pub coverage: Vec<CoverageTag>,
}
```

测试 runner 只负责执行 `built.expected.assert(&built.fixture, &built.tx)`。

### 7. framework 的测试反向依赖业务 fixture

`tests/src/framework/mod.rs` 的内部测试使用了 `crate::fixtures::limit_order` 扩展来验证 framework 行为。即使只在测试模块中，这也会模糊边界：framework 是否通用，不能靠 limit-order fixture 来证明。

目标状态下，framework 自测应只使用 framework 自己的 dummy action、dummy script、dummy cell。limit-order 应反过来证明业务 fixture 能消费 framework。

### 8. 与测试合约有关的抽象位置不应放进 framework

Persona、asset factory、named contract catalog、business error catalog 都很适合内化，但它们不应该进入 `framework`。例如 `Owner` / `Buyer` / `WrongOwner` 这些 persona 看起来通用，实际依赖测试合约的 lock 类型和业务角色；`TestUdt` / `TestNft` 依赖 tests 下的 fixture contracts；`LimitOrderErrorKind` 依赖 limit-order exit code。

这些应进入 `fixtures/common` 或具体业务 fixture。framework 只提供它们所需的底层能力：部署 bytes、构造 cell、记录 handle、生成 Cobuild witness、计算 signing hash。

## 安全风险面拆解

### 1. Witness 与全局激活

协议规定只要交易中存在 `WitnessLayout`，Cobuild-aware 脚本就必须进入 Cobuild 规则，不能因为本脚本 witness 不相关而回退 legacy。应覆盖：

- 无 Cobuild witness 时的 legacy/空义务行为。
- 有 `SighashAll`、`SighashAllOnly`、`OtxStart`、`Otx` 任一 witness 时的全局激活。
- 当前 lock/type 没有 action 但处于 Cobuild 激活交易中时，是否 fail closed 或无义务返回，取决于当前脚本位置。
- 当前 script group 非 carrier witness 非空时，是否拒绝。
- 多个 `SighashAll` 时是否拒绝 `DuplicateSighashAll`。
- `SighashAllOnly` 在存在唯一 `SighashAll.message` 时是否签 `TxWithMessage`，不存在时是否签 `TxWithoutMessage`。

### 2. OTX layout 与范围

`OtxLayoutCollector` 要保证 `OtxStart` 唯一、`Otx` 连续、至少一个 `Otx`、每个 `Otx` 至少一个 base input，且所有范围在交易内。应覆盖：

- `Otx` 出现在 `OtxStart` 前。
- 重复 `OtxStart`。
- `OtxStart` 后没有 `Otx`。
- `OtxStart` 与 `Otx` 中间插入非 OTX witness。
- 多个 OTX 的 input/output/cell_dep/header_dep 范围连续递进。
- `start_*` 指向非零偏移，且前置实体不属于 OTX。
- base/append count 超出交易实体数量。
- `base_input_cells = 0`。
- append count 非零但 permission bit 未开启。
- permission 高 4 位非零。
- mask 长度不足、过长、padding bit 非零。

这些测试不能只放单元测试。至少要有少量端到端锁合约测试验证错误码映射和链上执行路径。

### 3. 签名域与 hash preimage

`hash/mod.rs` 是核心安全边界。应覆盖正负两类：

- 正例：`TxWithMessage`、`TxWithoutMessage`、`OtxBase`、`OtxAppend` 的完整 preimage 形态。
- 负例：修改 message、append_permissions、base mask、base input since、previous_output、resolved input output/data、base output 被 mask 覆盖字段、append input/output/cell_dep/header_dep 后，原签名必须失效。
- 负例：修改 base output 未覆盖字段时，base 签名可保持有效；但若业务脚本依赖该字段，应由业务脚本拒绝。
- append hash 必须绑定 base hash：base scope 变化后 append seal 不能复用。
- local index 必须进入 hash：相同实体重排后签名不能错误复用。
- 长度前缀必须进入 hash：不同 byte 拼接边界不能产生同 hash 语义。

当前 `tests/src/fixtures/otx_hash.rs` 已有一份 fixture-local hash mirror，但没有形成通用变异 DSL。建议将“签名后 mutate 交易，然后断言 seal 失效”的能力抽到 framework。

### 4. Action target 与相关性

`CurrentScriptContext::validate_message_targets` 以全交易 input/output hash 集合验证 target。应覆盖：

- `script_role = 0/1/2` 分别命中 input lock、input type、output type。
- role 非法值 fail closed。
- action target hash 不存在 fail closed。
- action target 存在但当前脚本不相关：不能误当作当前业务 action。
- 同一 message 多个匹配 action：如果业务要求唯一，应拒绝。
- tx-level action 和 OTX action 同时存在时，业务脚本只消费正确 origin。
- type 脚本在 base output 中出现但 type mask 未覆盖时，`output_type_in_base_covered` 必须为 false，并由业务规则决定是否拒绝。

### 5. Lock 覆盖与混合输入

`ensure_otx_lock_group_coverage` 防止同一 lock group 的一部分 input 在 OTX 内、一部分在 OTX 外却没有 tx-level 签名。应覆盖：

- 当前 lock 所有 input 都在 OTX range 内：只需要 OTX signature。
- 当前 lock 同时有 OTX 内和 OTX 外 input：必须有 tx-level signature，否则 `MissingLockGroupCoverage`。
- 当前 lock 只在 append input：需要 append seal。
- 同一 lock 同时在 base 和 append：需要 base seal 与 append seal 两个 scope。
- seal pair 缺失、重复、scope 非法、script_hash 错误。
- 交易中有其他 lock 的 input 不应影响当前 lock 的覆盖判断。

### 6. Type plan 与业务脚本

`TypePlanBuilder` 不做签名验证，但它决定业务脚本能看见哪些 action 和 OTX relation。应覆盖：

- type 在 input base、input append、output base、output append 各自出现。
- type 只被 action target 提到，但不在当前 OTX scope 中，标记为 `TargetOnly`。
- type 同时在 scope 中且有 action，`TypeActionOtxScope::InOtxScope` 携带完整 relation。
- type 在 tx-level scope 但不在 OTX range 内时，tx-level action 可见。
- type 在 OTX range 内时，tx-level action 不能被误用来完成 OTX 业务语义。

### 7. Limit Order 业务安全

现有 limit-order 覆盖已经较强，但建议补齐矩阵表达，而不是继续散落增加 case：

- create order：state/action 一致性、type-id、NFT proxy output、input/output group shape。
- fill order：payment output index 必须指向当前 OTX append output，不可指向另一个 OTX 或 remainder output。
- payment output 必须绑定 UDT type、owner lock、amount。
- buyer NFT output 必须绑定 buyer lock、NFT type、原 NFT data。
- 多订单不能复用同一个 payment output；type order 与 lock order 混合也不能复用。
- tx-level message 与 OTX message 同时存在时，fill action 必须来自 OTX message。
- unknown action tag 与 malformed action payload 必须返回稳定错误码。

## 推荐测试分层

### A. Core 单元测试

放在 `crates/cobuild-core/src` 内部测试或 `crates/cobuild-core/tests`：

- `MaskView`、`OtxLayoutCollector`、`MessageView`、`CurrentScriptContext`、plan 数据结构。
- 使用手写最小 Molecule bytes 或实体 builder，避免启动 ckb-testtool。
- 目标是精确验证协议分支和错误类型。

### B. Core 集成式 host 测试

放在 `crates/cobuild-core/tests`，使用测试 reader 或 cached parts：

- 构造完整 raw transaction / resolved input / witness cursor。
- 验证 hash preimage、layout range、type relation。
- 不部署合约，不跑 VM，速度要快。

### C. Lock 合约端到端测试

放在 `tests/tests/cobuild_otx_lock.rs`：

- 只选安全关键路径和错误码映射。
- 每个 Core 错误类别至少有一个端到端 case。
- 签名相关负例必须端到端跑 VM，确认 verifier 和错误码链路一致。

### D. 业务合约端到端测试

放在 `tests/tests/limit_order_type.rs` 和 `tests/tests/limit_order_lock.rs`：

- 用业务场景 enum 表达变体。
- 每个业务不变量至少一正一负。
- 重点覆盖跨 OTX、remainder、tx-level/OTX 混合、多订单、多脚本混合。

### E. 结构守护测试

继续保留 `contract_template_layout.rs`、`workspace_layout.rs`、`makefile_layout.rs`：

- 防止生产合约重新引入旧 API、全交易 Vec load、`entity` 依赖、unsafe、错误 hash type。
- 这类测试不是行为测试，但能阻止架构退化。

## 测试矩阵建议

建议维护一个显式矩阵，避免“看起来很多测试但关键组合未覆盖”。

| 维度 | 必测取值 |
| --- | --- |
| Flow | no Cobuild, TxWithoutMessage, TxWithMessage, OTX only, Tx + OTX |
| Script role | input lock, input type, output type |
| OTX scope | base input, append input, base output, append output, outside OTX |
| OTX count | single OTX, two OTX, OTX with prefix tx-level entities |
| Signature scope | tx-level, base, append, base + append same lock |
| Action source | tx-level message, OTX message, absent, wrong target, duplicate |
| Mutation target | message, permission, mask, input, resolved input, output, cell_dep, header_dep, witness order |
| Expected result | pass, Core fail closed, verifier fail, business semantic fail |

每个新增业务测试应能指出自己覆盖矩阵中的哪几个格子。否则容易堆出重复测试。

## Framework 目标抽象

重构后的 framework 应围绕五个稳定抽象，而不是围绕当前测试文件的便利函数：

### 1. `ProtocolSpec`

描述 Cobuild 协议对象本身：

- tx-level carrier：`SighashAll` / `SighashAllOnly`
- OTX carrier：`OtxStart` / `Otx`
- message/action/seal pair
- append permission
- base mask
- OTX segment count

它只关心协议字段，不关心具体合约、业务资产或 CKB context。

### 2. `TxShape`

描述交易实体布局：

- prefix inputs/outputs/cell_deps/header_deps
- OTX segments
- suffix/remainder entities
- witness sequence
- script group witness placement

它负责把“OTX 范围”和“交易真实索引”映射清楚。所有跨 OTX、安全边界和 out-of-range 测试都应通过 `TxShape` 表达。

`TxShape` 的输出不能只是 `TransactionView`。它还必须产出索引映射和 resolved facts：

```rust
pub struct BuiltTxShape {
    pub tx: TransactionView,
    pub inputs: EntityIndexMap<InputHandle>,
    pub outputs: EntityIndexMap<OutputHandle>,
    pub cell_deps: EntityIndexMap<CellDepHandle>,
    pub header_deps: EntityIndexMap<HeaderDepHandle>,
    pub witnesses: WitnessIndexMap<WitnessHandle>,
    pub resolved_inputs: Vec<ResolvedInputFacts>,
    pub otx_ranges: Vec<OtxRangeFacts>,
}
```

所有业务 action、mutation、signing hash 都应引用 handle 或 facts，而不是引用裸 `usize`。

### 3. `SigningModel`

描述签名事实和 hash mirror：

- tx-level signing fact
- OTX base signing fact
- OTX append signing fact
- signing hash 计算接口
- carrier witness index
- signed digest
- seal pair ownership
- sign 后可复用的 `SigningFacts`

业务 fixture 不直接计算 hash。测试只声明“这个 lock 需要 base seal / append seal / tx-level seal”，由 `SigningModel` 生成。

`signing_hash` 必须做成 framework 里的稳定接口，而不是散落在业务 fixture 或个别测试 helper 中。它是测试侧的 protocol oracle，用于两类场景：

- 生成正确 seal：根据 tx shape、witness、resolved input、scope 计算 tx-level / OTX base / OTX append signing hash。
- 验证安全变异：签名后修改某个字段，再通过同一接口证明 digest 已变化，或直接断言旧 seal 失效。

建议接口按签名域拆开：

```rust
pub trait SigningHashOracle {
    fn tx_without_message(&self, tx: &TransactionView, inputs: &[ResolvedInput]) -> [u8; 32];
    fn tx_with_message(
        &self,
        tx: &TransactionView,
        inputs: &[ResolvedInput],
        message: &CobuildMessage,
    ) -> [u8; 32];
    fn otx_base(&self, built: &BuiltScenario, otx_index: usize) -> [u8; 32];
    fn otx_append(
        &self,
        built: &BuiltScenario,
        otx_index: usize,
        base_hash: [u8; 32],
    ) -> [u8; 32];
}
```

具体类型名可以调整，但边界必须清楚：hash oracle 接收 framework 的交易形态和 resolved input 事实，不依赖 limit-order，不读取业务 action payload 语义，不调用生产合约入口。

`SigningFacts` 应记录足够多的信息，方便测试解释失败：

```rust
pub struct SigningFacts {
    pub signer: SignerId,
    pub scope: SignatureScope,
    pub carrier: WitnessHandle,
    pub script_hash: [u8; 32],
    pub signing_hash: [u8; 32],
    pub seal: Vec<u8>,
}
```

签名后 mutation 需要能对比 mutation 前后的 signing hash；这应由 framework 提供断言：

```rust
assert_hash_changed(&before, &after, SignatureScope::OtxAppend);
assert_old_seal_rejected(&mutated_case, signer, SignatureScope::OtxAppend);
```

### 4. `Mutation`

描述负例变异：

- witness 插入、删除、替换、重排
- 修改 message/action/seal
- 修改 input/output/cell_dep/header_dep
- 修改 resolved input data
- 修改 permission/mask/count/start index
- 把 payment 指向另一个 OTX 或 remainder

mutation 必须发生在抽象层，避免每个 fixture 复制交易 builder 代码。

mutation 应分协议级和业务级：

- `ProtocolMutation`：非法 mask、permission 高位、重复 `SighashAll`、OTX witness 非连续、seal scope 错误。
- `TxShapeMutation`：把 output 移到另一个 OTX、移到 remainder、替换 input、替换 resolved data、重排 outputs。
- `BusinessMutation`：wrong UDT、wrong owner、insufficient amount、wrong NFT type、malformed action payload。

`ProtocolMutation` 和通用 `TxShapeMutation` 属于 framework。`BusinessMutation` 属于 fixtures。业务 fixture 可以组合 framework mutation，但不应自己手改 witness 或交易索引。

### 5. `ExpectedOutcome`

描述验证结果：

- pass
- lock exit
- input type exit
- output type exit
- verifier failure
- Core error category
- business error category

测试文件不应散落硬编码 assertion 细节。每个 scenario 自带 expected outcome，测试 runner 统一执行。

## tests/src/framework 当前问题

### 1. `OtxBuilder` 默认过于“成功路径”

当前 builder 会自动给 base input mask 设置 `vec![0]`，base output mask 设置 full mask，但缺少显式 API 构造：

- 非法 mask 长度。
- padding bit 非零。
- permission 高位非零。
- append count 与 permission 不匹配。
- 自定义 base input/output mask bit。
- base/append cell_dep/header_dep。

目标 API 应提供：

- `append_permissions_raw(u8)`
- `base_input_masks_raw(Vec<u8>)`
- `base_output_masks_raw(Vec<u8>)`
- `base_cell_dep_masks_raw(Vec<u8>)`
- `base_header_dep_masks_raw(Vec<u8>)`
- `allow_append_cell_deps()`
- `allow_append_header_deps()`
- `cover_base_input_since(index)`
- `cover_base_input_previous_output(index)`
- `cover_base_output_capacity/lock/type/data(index)`
- `uncover_base_output_*`

### 2. `OtxTransactionBuilder` 只能生成规范顺序

当前 builder 强制输出 base inputs、append inputs、base outputs、append outputs、remainder outputs，并自动生成规范 `OtxStart + Otx...` witness。它适合正例，但不适合 layout 负例。

目标状态不是在现有 builder 上继续堆选项，而是拆成两层：

- `TxShapeBuilder`：从抽象 segment 生成规范或非规范交易形态。
- `TxMutator`：对已生成交易做签名前/签名后变异。
- 支持 witness 插入、删除、替换、重排。
- 支持 `OtxStart` raw override。
- 支持多个 OTX 的实体分段，显式命名每段。
- 支持 header_dep 与 scoped cell_dep。
- 支持 prefix/suffix inputs/outputs/cell_deps，区别 OTX range 和 remainder。

### 3. 缺少“签名后变异”的一等 API

安全测试常见模式是：

1. 构造有效交易。
2. 计算正确 seal。
3. 修改某个被签名字段。
4. 断言 verifier fail 或业务 fail。

当前 fixtures 多数是直接搭最终交易，变异逻辑分散。目标 API 应新增：

- `SignedCase { fixture, tx, signing_facts }`
- `SigningFacts` 记录 tx hash、base hash、append hash、carrier witness index、scope、script hash。
- `TxMutator::replace_input(index, input)`
- `TxMutator::replace_output(index, TestCellOutput)`
- `TxMutator::replace_witness(index, Bytes)`
- `TxMutator::replace_cell_dep(index, CellDep)`
- `TxMutator::append_remainder_output(...)`
- `assert_verify_failure_after_mutation(...)`

### 4. framework 与 fixtures 边界不够清晰

目标边界：

- `framework` 只提供 Cobuild/CKB 协议底座，不包含 named test contracts、test assets、persona、limit-order 业务语义。
- `fixtures/common` 可以提供 tests 合约通用抽象，例如 persona、test asset factory、named contract deployment catalog。
- `fixtures/limit_order` 描述业务状态、业务 action、业务场景 enum、业务 mutation、业务 error catalog。
- `fixtures/cobuild_otx_lock` 只描述锁签名与协议级场景。
- `fixtures/otx_hash` 不应继续作为业务 fixture 的私有工具；可复用的 hash/signing mirror 应迁移到 `framework/signing`，并清楚标注为 test oracle。

### 5. Assertion 应变成结果模型

当前 assertion 已能按 lock/type/output type 和 exit code 断言，但测试仍然需要手写很多重复代码。目标是分两层结果模型：

- framework 提供协议级 `ExpectedOutcome`，只表达 script 位置、Core/verifier 类错误和通用 pass/fail。
- fixtures 提供业务级 `BusinessExpectedOutcome`，表达 limit-order/test-contract error kind，并映射到具体 exit code。

还应补充：

- `assert_core_error_lock(tx, input, CoreExit)`
- `assert_business_error_type(tx, input, LimitOrderExit)`
- `assert_verify_failure(tx, input)`
- `assert_no_failed_tx_dump_delta(before)`

这样测试名和断言能同时表达“协议失败”还是“业务失败”，但业务错误目录不进入 framework。

## 目标模块结构

```text
tests/src/framework/
  assertions.rs       // 通用 pass/fail/error-code assertion
  cells.rs            // CellOutput/TestCellOutput/TestResolvedInput/live input
  deploy.rs           // 无 named contract 语义的部署原语
  scripts.rs          // script hash、hash 转换
  molecule.rs         // 最小 raw witness/action/message bytes helper
  cobuild/
    mod.rs
    message.rs        // ActionSpec, MessageBuilder
    otx.rs            // OtxSpec, OtxBuilder, mask API
    witness.rs        // SighashAll/SighashAllOnly/OtxStart/Otx witness API
    layout.rs         // OtxSegment, OtxLayoutSpec, expected ranges
  tx/
    mod.rs
    builder.rs        // 正常交易 builder
    malformed.rs      // 畸形 layout/witness builder
    mutate.rs         // 签名后变异
  signing/
    mod.rs
    tx.rs             // tx-level hash mirror/sign
    otx.rs            // otx hash mirror/sign
    keys.rs           // fixed keys, pubkey hash
    oracle.rs         // SigningHashOracle / SigningFacts
  scenario/
    mod.rs
    outcome.rs        // 协议级 ExpectedOutcome
    runner.rs         // 不含业务语义的表驱动执行

tests/src/fixtures/
  common/
    contracts.rs      // named test contract catalog: always-success, test-udt, test-nft, proxy locks
    personas.rs       // Owner/Buyer/FeePayer/WrongOwner 等测试角色
    assets.rs         // TestUdt/TestNft/asset pair factories
    errors.rs         // fixture-level error catalogs shared by test contracts
  cobuild_otx_lock/
    mod.rs            // cobuild-otx-lock contract scenarios
    errors.rs         // lock contract exit catalog
    cases.rs
  limit_order/
    mod.rs
    actions.rs        // typed create/fill action builders
    state.rs          // order/payment/nft state builders
    scenarios.rs      // HappyPath
    mutations.rs      // BusinessMutation
    errors.rs         // LimitOrderErrorKind
```

这不是为了目录好看，而是为了让每一层依赖方向单向流动：

```text
fixtures/limit_order + fixtures/cobuild_otx_lock
  -> fixtures/common
  -> framework/scenario
  -> framework/signing + framework/tx + framework/cobuild
  -> framework/cells + framework/deploy + framework/scripts
```

禁止反向依赖：`framework` 不依赖任何 fixture；`framework/deploy` 不认识 named test contracts；`signing` 不依赖 limit-order；`tx` 不理解 action payload；`fixtures/limit_order` 不直接手写 Molecule witness。

## 可继续内化的 Fixture 抽象

这些能力应该内化，但位置是 `fixtures`，不是 `framework`。

### 1. Persona

`Owner`、`Buyer`、`FeePayer`、`WrongOwner`、`OrderLockOwner` 应统一创建 lock、script hash、auth identity、可签名身份。它们依赖测试合约和业务角色，因此属于 `fixtures/common/personas.rs`。

### 2. Test Asset Factory

`TestUdt`、`TestNft`、wrong UDT、wrong NFT、NFT payload、UDT amount data 应进入 `fixtures/common/assets.rs` 或 `fixtures/limit_order/state.rs`。framework 只提供 typed cell 和 output handle。

### 3. Named Contract Catalog

`always-success`、`test-udt`、`test-nft`、`input-type-proxy-lock`、`limit-order-type`、`limit-order-lock` 的部署函数属于 `fixtures/common/contracts.rs` 或对应业务 fixture。framework 只提供“部署一段 bytes 得到 script/cell_dep”的底层能力。

### 4. Business Error Catalog

`LimitOrderErrorKind::InsufficientPayment`、`MalformedAction`、`WrongNftType` 这类错误属于具体合约 fixture。测试不应硬编码 `10/11/12`，但映射也不应放在 framework。

### 5. Business Action Binding

`LimitOrderAction::fill(payment_handle, buyer)`、`LimitOrderAction::create(order_state)` 属于 `fixtures/limit_order/actions.rs`。它可以消费 framework 的 `OutputHandle` 并解析出 action 需要的 index，但 action payload 语义不能进入 framework。

### 6. Business Scenario Runner

业务 runner 可以封装 deploy named contracts、创建 persona、创建 test assets、套用业务 mutation、断言 business error。它建立在 framework 的 protocol runner 上，但错误目录和默认资产都来自 fixtures。

## 推荐抽象归属

### 1. Framework-owned: OTX Segment

OTX segment 是 Cobuild layout 抽象，属于 framework。它用结构化 segment 替代散落的 count：

```rust
pub struct OtxSegment {
    pub base_inputs: Vec<CellInput>,
    pub append_inputs: Vec<CellInput>,
    pub base_outputs: Vec<TestCellOutput>,
    pub append_outputs: Vec<TestCellOutput>,
    pub base_cell_deps: Vec<CellDep>,
    pub append_cell_deps: Vec<CellDep>,
    pub base_header_deps: Vec<[u8; 32]>,
    pub append_header_deps: Vec<[u8; 32]>,
}
```

builder 从 segment 自动计算 count。非法 count、非法 mask、非法 witness 顺序不通过业务 fixture 手写，而通过 framework 的 `ProtocolSpec` raw override 或 `ProtocolMutation` 注入。

### 2. Fixture-owned: Scenario + Mutation

业务 fixtures 使用两层枚举。`HappyPath` 表示合法交易骨架，`Mutation` 表示破坏哪个不变量：

```rust
enum LimitOrderHappyPath {
    TypeNftForUdt,
    LockNftForUdt,
    MixedTypeAndLock,
}

enum LimitOrderMutation {
    PaymentOutputWrongUdt,
    PaymentOutputOutOfCurrentOtx,
    ReusePaymentOutput,
    TxLevelActionInsteadOfOtxAction,
    BuyerNftWrongType,
}
```

这样可以避免一个 enum 混合“基础场景”和“故障注入”，提升组合测试能力。最终 case 由 `HappyPath + Vec<BusinessMutation> + BusinessExpectedOutcome` 组成。它可以调用 framework 的 `ProtocolMutation` 和 `TxShapeMutation`，但业务 mutation 类型本身留在 fixtures。

### 3. Fixture-owned: Expected Outcome

每个 case 返回 expected outcome，而不是测试文件硬编码全部错误码：

```rust
pub enum LimitOrderExpectedOutcome {
    Pass,
    LockExit { input: InputHandle, error: LimitOrderLockErrorKind },
    InputTypeExit { input: InputHandle, error: LimitOrderTypeErrorKind },
    OutputTypeExit { output: OutputHandle, error: LimitOrderTypeErrorKind },
    CoreFailure { script: ScriptHandle, error: CoreErrorKind },
}
```

测试文件可以表驱动：

```rust
for case in cases {
    let built = case.build();
    built.expected.assert(&built.fixture, &built.tx);
}
```

这能减少重复测试样板，但仍保留每个 case 的名字和意图。

framework 可以提供协议级 `ExpectedOutcome` 和 script exit assertion 原语；业务错误枚举、错误码映射、默认断言位置属于 fixtures。

### 4. Fixture-owned: Coverage Manifest

每个业务 fixture 应暴露 coverage manifest，用来说明它覆盖了哪些矩阵维度：

```rust
pub struct CoverageTag {
    pub flow: FlowKind,
    pub script_role: ScriptRoleKind,
    pub otx_scope: OtxScopeKind,
    pub signature_scope: SignatureScopeKind,
    pub action_source: ActionSourceKind,
    pub mutation: Option<MutationKind>,
}
```

测试 runner 可以打印或断言关键 tag 存在。这样“全面”不是靠人工感觉，而是有可检查的覆盖索引。

## 重构执行优先级

### P0：先建抽象骨架

- `ProtocolSpec` / `TxShape` / `SigningModel` / `Mutation` / `ExpectedOutcome` 五个核心抽象。
- 多 OTX segment、prefix/remainder entity、witness sequence 的统一 builder。
- raw permission/mask/count/start index override。
- signing facts 与签名后 mutation。
- business fixture 与 framework 的依赖边界清理。

### P1：安全关键补洞

- OTX permission 高位非零、append count 未授权。
- mask 长度与 padding bit。
- 重复 `SighashAll`。
- `OtxStart` 后 witness 非连续。
- 同 lock base + append 双 scope seal。
- OTX 内外同 lock 缺 tx-level 签名。
- 签名后修改 message、permission、resolved input data、append output data。
- action target role 非法或 target 不存在的端到端错误码。

### P2：覆盖质量提升

- 为 type relation 的 `output_type_in_base_covered` 加业务端到端用例。
- 为 cell_dep/header_dep scope 和 hash 变异加 host 测试。
- 为 tx-level 与 OTX action 同时存在的业务脚本消费规则加矩阵。
- 增加 coverage checklist 文档，要求每次新增业务合约时填写。

## 简洁性原则

全面测试不等于每个组合都端到端跑 VM。推荐规则：

- 纯解析、mask、layout 算法：单元测试。
- hash preimage：host 测试 + 少量端到端签名失效测试。
- 错误码映射、verifier、链上 syscall 路径：端到端测试。
- 业务不变量：端到端测试，但用表驱动和 scenario/mutation 组合减少重复。
- 架构边界：文本守护测试。

每个测试只断言一个安全不变量。多个断言可以共享 fixture，但不要在一个测试里混合多个失败原因，否则后续失败定位会变差。

简洁性来自抽象，不来自少测。新增 case 时优先复用 `HappyPath + Mutation + ExpectedOutcome`，不要新增一个完整 fixture 函数。只有当业务骨架本身不同，才新增 `HappyPath`。

## 结论

当前测试基础已经能支撑 limit-order 主路径和大量业务负例，但要达到“抽象、全面、解耦”，需要把 framework 从“正例 builder + 业务 helper”升级为“协议规格 + 交易形态 + 签名模型 + 变异模型 + 结果模型”的测试底座。

最值得先做的是抽出五个核心模型并迁移 limit-order fixtures。安全补洞测试应基于新模型补齐，而不是继续在旧 helper 上追加特殊 case。这样后续新增合约或协议规则时，可以用短测试表达复杂安全场景，而不是复制低层交易组装代码。
