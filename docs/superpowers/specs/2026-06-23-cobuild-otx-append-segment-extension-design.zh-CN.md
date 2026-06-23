# Cobuild OTX Append Segment Extension Design

## Status

本文是一个探索性设计草案，用于评估在 Cobuild Core v1 之外增加
`append segment` 扩展是否值得。

本文不修改 `2026-05-28-cobuild-core-community-redraft-design.md` 定义的
Core v1 baseline。当前建议是：

- Core v1 继续保持一个 OTX 内只有 `base scope + append scope`；
- 多人独立拼接优先使用多个 OTX；
- 只有当应用确实需要“一个 base intent 下的多段 append contribution”时，
  才考虑本扩展。

## Motivation

现有 Core v1 的 `OtxAppend` 是一个整体 append scope。只要某个 lock 出现在
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
- 不在 Core v1 中增加新的 witness variant。
- 不提供通用业务约束语言。
- 不让 segment 签名隐式保证最终交易的全局经济正确性。
- 不支持第一版中的复杂 coverage mode，例如签后续 segment count 或
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

如果应用安全性要求每个 contributor 都确认最终完整 append 内容，则不应使用
本扩展，应继续使用 Core v1 的整体 append scope。

## Data Model

扩展不新增 `WitnessLayout` variant，而是定义一个新的 OTX-compatible witness
schema。具体接入方式可以是 Core v2，或一个明确标记的 extension OTX variant。

这意味着当前 Core v1 实现不能只靠解释现有 `Otx` 字段来获得 segment 语义。
落地时必须二选一：

- 在 Core v2 中替换或扩展 `Otx` schema；
- 在标准扩展中定义新的 witness envelope，并让支持该扩展的脚本显式识别它。

本文只评估链上语义和 hash 规则，不要求当前 Core v1 代码立即支持该 schema。

概念 schema：

```text
table SegmentSealPair {
  script_hash: Byte32,
  seal: Bytes,
}

vector SegmentSealPairVec <SegmentSealPair>;

table OtxAppendSegment {
  segment_flags: byte,

  input_cells: Uint32,
  output_cells: Uint32,
  cell_deps: Uint32,
  header_deps: Uint32,

  seals: SegmentSealPairVec,
}

vector OtxAppendSegmentVec <OtxAppendSegment>;

table SegmentedOtx {
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

  base_seals: SealPairVec,
}
```

`base_*` 字段沿用 Core v1 的 base coverage 规则。`append_segments` 替代 Core
v1 的单组 `append_*_cells`。

## Segment Flags

第一版只定义一个 flag：

```text
bit 0: allow_more_segments_after
bits 1..7: reserved, MUST be zero
```

语义：

- `0x01`: 当前 segment 签名者允许后续继续追加 segment；
- `0x00`: 当前 segment 必须是最后一个 append segment；
- 其他 bit 非零时 witness 无效。

这条规则给 contributor 一个最低限度的 finality 控制，同时避免把 Core 扩展成
复杂的协作策略语言。

## Transaction Layout

最终交易中的实体仍然只出现一次。segment 只提供 witness 中的分段边界。

对一个 `SegmentedOtx`，实体排列为：

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
每个 segment 在最终交易中的实体范围。

## Signing Domains

扩展引入一个新的 signing domain：

```text
OtxAppendSegment
```

`OtxBase` 仍按 Core v1 计算。

每个 segment 的签名 hash 只覆盖同一个 base commitment 和该 segment 自身：

```text
OtxAppendSegmentHash =
  hash(
    message,
    base_hash,
    segment_index,
    segment_flags,
    segment input count and full segment inputs,
    segment output count and full segment outputs,
    segment cell dep count and full segment cell deps,
    segment header dep count and full segment header deps
  )
```

`segment_index` 必须进入 hash，避免两个相同内容的 segment 签名可互换。
`base_hash` 必须进入 hash，避免 segment 被搬到另一个 base intent 下复用。

## Lock Signature Requirements

对每个 relevant segment：

- 如果当前 lock script hash 出现在该 segment 的 input scope 中，必须找到且只
  找到一个 `SegmentSealPair`；
- seal 使用 `OtxAppendSegmentHash` 验证；
- 一个 lock 同时出现在 base 和一个或多个 segment 中时，需要分别提供 base
  seal 和对应 segment seal；
- segment action 本身不创建 lock 签名要求，签名要求仍由 input ownership 决定。

## Validation Rules

基础规则：

- `append_segments` 可以为空；为空时该 witness 等价于只有 base scope 的 OTX；
- 每个 segment 的 reserved flag bits 必须为零；
- 如果 segment `i` 的 `allow_more_segments_after` 为零，则 `i` 必须是最后一个
  segment；
- 如果 segment `i` 不是最后一个 segment，则 `allow_more_segments_after` 必须
  为一；
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

另一种设计是后追加者签前面所有 append 内容：

```text
B signs base + B
C signs base + B + C
D signs base + B + C + D
```

这适合有序接力构建，但不适合作为本扩展的默认规则：

- 前面的签名者仍然没有确认后面的内容；
- 后面的签名者被迫审查并背书前面所有内容；
- 修改前面 segment 会让后面签名全部失效；
- 多方并行提交 contribution 的能力较差。

因此第一版选择 independent segment model：

```text
B signs base + B
C signs base + C
D signs base + D
```

如果未来需要链式覆盖，应作为单独 coverage extension 设计，而不是混入最小
append segment 规则。

## Recommendation

不要把 append segment 加入 Core v1。

当前推荐路线：

1. Core v1 继续保留单 append scope；
2. 多人独立聚合继续使用多个 OTX；
3. 只有当真实应用反复需要“一个 base intent 下的多个独立 contribution”时，
   再把本文设计推进为标准扩展或 Core v2 proposal。

append segment 的价值是真实的，但覆盖面较窄。它更像 shared-base fulfillment
扩展，而不是 Cobuild Core 的基础能力。

## Open Design Questions

如果该扩展继续推进，下一轮设计需要回答：

- 是否采用 Core v2 schema，还是定义独立 extension envelope；
- `message` 是否由所有 segment hash 直接覆盖，还是改为覆盖 message
  commitment；
- 业务脚本如何枚举同一个 base intent 下的 segment actions；
- 是否需要一个 off-chain packet 格式帮助参与者检查 segment flags 和最终顺序；
- 是否需要未来的 optional coverage mode，例如 chain coverage 或 following
  segment commitment。
