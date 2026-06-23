# Cobuild OTX Append Segment Replacement Design

## Status

本文记录当前实现中的 Cobuild OTX append segment 语义。该语义已经替代旧的
单一 append scope：schema、layout scanner、signing hash、signature planning
和测试 fixture 都以 `append_segments` 为准。

本文不定义一套与 Core 并行的长期 extension。当前实现采用以下协议形态：

- `Otx` schema 使用 base coverage 和 `append_segments`；
- 旧的单一 append scope、`append_*_cells` 字段和整体 `OtxAppend` hash
  已不再作为目标语义；
- 多人独立拼接优先使用多个 OTX；
- 当应用需要“一个 base intent 下的多段 append contribution”时，使用新的
  append segment 语义。

## Motivation

旧单 append 设计中的 `OtxAppend` 是一个整体 append scope。只要某个 lock 出现在
append input scope 中，它就需要签同一个 append hash。这个 hash 覆盖所有
append inputs、outputs、cell deps 和 header deps。

这种规则简单、强一致，但不适合一种更松耦合的协作模式：

1. A 创建一个 base intent；
2. B、C、D 后续分别追加自己的 inputs 和 outputs；
3. B 只想签 `base + B contribution`，不想因为 C 或 D 追加内容而重签；
4. 这些 contribution 又必须被解释为同一个 base intent 的组成部分，而不是
   几个完全独立的 OTX。

如果第 4 点不成立，应该使用多个 OTX，而不是 append segment。

## Non-Goals

- 不替代多个 OTX 的常规聚合模型。
- 不新增长期并行的 `SegmentedOtx` witness variant。
- 不提供通用业务约束语言。
- 不让 segment 签名隐式保证最终交易的全局经济正确性。
- 不支持第一版中的复杂 coverage commitment，例如签后续 segment count 或
  following segment commitment。

## When To Use Multiple OTXs Instead

以下场景优先使用多个 OTX：

- CoinJoin 或独立输入输出聚合；
- 批量支付；
- fee sponsor 或 fee bump；
- 多个用户各自贡献 input/output，最终只需要同一笔 tx 原子打包；
- 各参与方没有共享同一个 base message 或 base intent 的需求。

多个 OTX 的优点是 Core v1 已经支持，签名边界清楚，脚本不需要理解新的
segment layout。

## When Append Segment Is Useful

append segment 适合更窄的场景：

- 一个 maker intent 被多个 taker 分段 fill；
- 一个 base order 允许多方追加履约片段；
- 一个共享 quote、auction、batch intent 需要多个 later contributors；
- 业务脚本希望枚举“同一个 base intent 下的所有 contribution”；
- 每个 contributor 只愿意签自己的 contribution，但仍愿意绑定同一个
  `base_hash`。

如果应用安全性要求每个 contributor 都确认最终完整 append 内容，可以要求每个
segment 设置 `coverage_previous_segments=1`，或在业务层要求最后一段覆盖全部
前序 segment。旧的整体 append scope 不作为长期保留方案。

## Data Model

当前 OTX 直接修改 `Otx` schema，不新增长期并行 witness envelope。协议目标是
只保留 append segment 语义；测试框架也不提供默认修改第一个 append segment 的
旧 single-append 兼容 helper。

概念 schema：

```text
table LockSeal {
  script_hash: Byte32,
  seal: Bytes,
}

vector LockSealVec <LockSeal>;

table OtxAppendSegment {
  segment_flags: byte,

  input_cells: Uint32,
  output_cells: Uint32,
  cell_deps: Uint32,
  header_deps: Uint32,

  seals: LockSealVec,
}

vector OtxAppendSegmentVec <OtxAppendSegment>;

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

  append_segments: OtxAppendSegmentVec,

  base_seals: LockSealVec,
}
```

`base_*` 字段沿用 Core v1 的 base coverage 规则。`append_segments` 替代旧的
单组 `append_*_cells`。`base_seals` 替代旧的 `seals`，append seal 移到每个
`OtxAppendSegment.seals` 中。seal 自身不再携带 base/append scope，签名位置由
所在字段决定。

## Append Permissions

`append_permissions` 只表达 base 作者允许后续追加哪些实体类型：

```text
bit 0: allow_append_inputs
bit 1: allow_append_outputs
bit 2: allow_append_cell_deps
bit 3: allow_append_header_deps
bits 4..7: reserved, MUST be zero
```

reserved bits 非零时 witness 无效。

这里不引入类似 PSBTv2 `Has SIGHASH_SINGLE` 的
`preserve_append_input_output_pairing` bit。Bitcoin 需要这个 flag 是因为
`SIGHASH_SINGLE` 自带 input index 和 output index 的配对语义；OTX append
segment 没有同样的内建 sighash 规则。如果某个业务需要 input/output 配对，应由
业务脚本按 segment 边界和 action 语义检查，而不是放进 OTX core 的全局
permission bit。

## Segment Flags

第一版只定义两个 flag：

```text
bit 0: allow_more_segments_after
bit 1: coverage_previous_segments
bits 2..7: reserved, MUST be zero
```

语义：

- bit 0 为 `0` 时，当前 segment 必须是最后一个 append segment；
- bit 0 为 `1` 时，当前 segment 签名者允许后续继续追加 segment；
- bit 1 为 `0` 时，当前 segment seal 只签 `base + own segment`；
- bit 1 为 `1` 时，当前 segment seal 签
  `base + all previous segments + own segment`；
- bits 2..7 非零时 witness 无效。

bit 0 给 contributor 一个最低限度的 finality 控制。bit 1 允许同一扩展同时
支持独立贡献模型和有序接力模型。更复杂的承诺，例如绑定后续 segment count
或 following segment commitment，不进入第一版 flags。

有效取值：

```text
0x00 = final segment, sign base + own segment
0x01 = allow later segments, sign base + own segment
0x02 = final segment, sign base + previous segments + own segment
0x03 = allow later segments, sign base + previous segments + own segment
```

## Transaction Layout

最终交易中的实体仍然只出现一次。segment 只提供 witness 中的分段边界。

对一个新版 `Otx`，实体排列为：

```text
inputs:
  base inputs
  segment 0 inputs
  segment 1 inputs
  ...
  segment N inputs

outputs:
  base outputs
  segment 0 outputs
  segment 1 outputs
  ...
  segment N outputs

cell deps:
  base cell deps
  segment 0 cell deps
  segment 1 cell deps
  ...
  segment N cell deps

header deps:
  base header deps
  segment 0 header deps
  segment 1 header deps
  ...
  segment N header deps
```

layout scanner 先读取 base counts，再按 `append_segments` 顺序累加 counts，得到
每个 segment 在最终交易中的实体范围。实现中的 runtime layout 可以缓存整体
append inputs/outputs/cell deps/header deps 范围以便复用，但这些 aggregate ranges
必须由同一次 scanner 游标推进结果生成，不能成为独立于 segment counts 的第二份
witness 语义来源。segment 的位置由 `append_segments` vector index 表达，不在
segment layout 中重复保存 `segment_index`。

## Signing Domains

扩展引入一个新的 signing domain：

```text
OtxAppendSegment
```

`OtxBase` 沿用旧 base coverage 规则，但 preimage 中不再包含旧的
`append_*_cells`。它继续覆盖 `message`、`append_permissions` 和 base entity range，
用于把所有 append segment 绑定到同一个 base intent。

每个 segment 的签名 hash 覆盖范围由 `coverage_previous_segments` 决定。

当 bit 1 为 `0` 时，hash 只覆盖同一个 base commitment 和该 segment 自身：

```text
OtxAppendSegmentHash =
  hash(
    base_hash,
    segment_flags,
    segment input count and full segment inputs,
    segment output count and full segment outputs,
    segment cell dep count and full segment cell deps,
    segment header dep count and full segment header deps
  )
```

当 bit 1 为 `1` 时，hash 覆盖同一个 base commitment、所有 previous segments
和该 segment 自身：

```text
OtxAppendSegmentHash =
  hash(
    base_hash,
    previous segment count,
    for each previous segment:
      previous segment flags,
      previous segment input count and full previous segment inputs,
      previous segment output count and full previous segment outputs,
      previous segment cell dep count and full previous segment cell deps,
      previous segment header dep count and full previous segment header deps,
    own segment flags,
    own segment input count and full own segment inputs,
    own segment output count and full own segment outputs,
    own segment cell dep count and full own segment cell deps,
    own segment header dep count and full own segment header deps
  )
```

`base_hash` 已经覆盖 `message`，因此 append segment hash 不再直接重复覆盖
`message`。`base_hash` 必须进入 hash，避免 segment 被搬到另一个 base intent 下
复用。

当 bit 1 为 `0` 时，segment 是 positionless own-only authorization：签名者
授权该 segment 自身的 inputs/outputs/deps/header_deps 可以以任意 append 位置
加入指定 base。它不承诺自己是第几个 segment。

当 bit 1 为 `1` 时，`previous segment count` 和按顺序写入的 previous segment
commitments 间接绑定了当前位置；如果前序 segment 的内容或顺序不对，验签无法
通过。因此不需要额外把 own `segment_index` 或 previous segment index 写入 hash。

当 bit 1 为 `1` 时，previous segment 的 flags 也必须进入 hash，避免前序段的
finality 或 coverage 语义在后续接力签名中被替换。

## Lock Signature Requirements

对每个 relevant segment：

- 如果当前 lock script hash 出现在该 segment 的 input range 中，必须找到且只
  找到一个 `LockSeal`；
- seal 使用 `OtxAppendSegmentHash` 验证；
- 一个 lock 同时出现在 base 和一个或多个 segment 中时，需要分别提供 base
  seal 和对应 segment seal；
- segment action 本身不创建 lock 签名要求，签名要求仍由 input ownership 决定。

## Validation Rules

基础规则：

- `append_segments` 可以为空；为空时该 witness 等价于只有 base coverage 的 OTX；
- 每个 segment 的 reserved flag bits 必须为零；
- 如果 segment `i` 的 `allow_more_segments_after` 为零，则 `i` 必须是最后一个
  segment；
- 如果 segment `i` 不是最后一个 segment，则 `allow_more_segments_after` 必须
  为一；
- 如果 segment `i` 的 `coverage_previous_segments` 为一，则它的 signing hash
  必须覆盖 segment `0..i` 的完整 append 实体和 flags；
- 如果 segment `i` 的 `coverage_previous_segments` 为零，则它的 signing hash
  不得覆盖其他 segment 的 append 实体，也不得绑定自己的 append position；
- `append_permissions` 的 reserved bits 必须为零；
- segment counts 必须与对应 append permissions 兼容；
- 每个 required segment seal 必须唯一；
- segment layout 必须与最终交易实体范围一致。

## Size Impact

append segment 会让 witness 变大，但不会重复交易 inputs 或 outputs。

额外开销来自：

- `append_segments` vector；
- 每个 segment 的 table offsets；
- 每个 segment 的 `segment_flags`；
- 每个 segment 的四个 count 字段；
- 每个 segment 自己的 seal vector。

在 secp256k1 recoverable seal 场景中，签名本身仍然是主要体积来源。每个
segment 至少需要一个 65-byte seal，加上 32-byte script hash 和 Molecule 编码
开销。segment 元数据通常是几十字节级别。

## Comparison With Chain Coverage

bit 1 支持后追加者签前面所有 append 内容：

```text
B signs base + B
C signs base + B + C
D signs base + B + C + D
```

这适合有序接力构建，但不作为默认规则：

- 前面的签名者仍然没有确认后面的内容；
- 后面的签名者被迫审查并背书前面所有内容；
- 修改前面 segment 会让后面签名全部失效；
- 多方并行提交 contribution 的能力较差。

默认规则仍然是 independent segment model：

```text
B signs base + B
C signs base + C
D signs base + D
```

因此 `coverage_previous_segments` 必须由每个 segment 显式选择。钱包和
off-chain builder 需要把 bit 1 展示为签名覆盖差异，而不是普通 metadata。

## Recommendation

不要把 append segment 作为长期并行 extension 加入 Core v1；它就是当前 OTX
append 语义。

当前实现状态：

1. `Otx` 使用 `append_segments` 替代旧单 append scope；
2. schema、hash、layout、签名规划和测试框架已经按 append segment 迁移；
3. `append_*_cells`、`SealScope` 和整体 `OtxAppend` hash 不再是协议目标；
4. `OtxLayout` 中的 aggregate append ranges 是由 segment layout 派生的运行时缓存，
   不是旧语义残留。

append segment 的价值是真实的，但覆盖面较窄。它不应和旧 append 语义长期并存；
否则钱包、builder 和脚本都需要同时解释两套 append 签名模型。

## Implementation Notes

当前实现的关键约束：

- `message` 由 `OtxBase` 覆盖，append segment hash 通过 `base_hash` 绑定它；
- own-only segment hash 不编码 own segment index；
- previous-coverage segment hash 不编码 `previous_segment_index`，而是通过
  previous segment count 和有序 previous segment 内容绑定位置；
- `append_permissions` 是跨所有 append segments 的实体类型 gate，允许 bit 未使用；
- 业务脚本如果需要枚举同一个 base intent 下的 contribution，应按 segment layout
  和自身 action 语义检查，而不是依赖旧整体 append scope。
