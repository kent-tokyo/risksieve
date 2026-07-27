# risksieve

予測や意思決定を、有限サンプルおよびanytime-validなコンフォーマルリスク保証で
ふるいにかけるRustライブラリ。

## 現在の状況

**Milestone 0(語彙)、Milestone 1(古典的単調CRC)、Milestone 2(anytime-valid
単調CRC)、Milestone 3(非単調CRC、部分的)、Milestone 4(SCoRE-MDR、部分的)、
Milestone 5(SCoRE-SDR、部分的)、Milestone 6(分布シフト、部分的)が完了。**
Milestone 7(下流の実例)はまだ未実装。

Milestone 0 で提供するもの:

- 確率的な値のための検証済み数値型(`OpenUnitInterval`, `ClosedUnitInterval`,
  `NonNegative`, `ClosedInterval`);
- 評価時にチェックされる `BoundedLoss` コントラクトと、組み込みの
  `ZeroOneLoss` / `AbsoluteErrorLoss`;
- 今後すべての証明書が使う `GuaranteeKind` / `Assumptions` の分類体系;
- 出力型 `RiskCertificate` / `Diagnostics`;
- エラー分類 `RiskSieveError`。

Milestone 1 では、有界単調損失に対する有限サンプル期待リスクコントローラ
`risksieve::crc::monotone::certify`(Angelopoulos, Bates, Fisch, Lei,
Schuster (2024) の Theorem 1)と、それが利用する誤差補正付き総和
`risksieve::numerics::summation` を追加した。

Milestone 2 では、キャリブレーション観測を1件ずつ取り込み、そのたびに更新
された証明書を返す `risksieve::anytime::AnytimeController` を追加した
(Hultberg, Zachariah, Ribeiro (2026) の Theorem 4.1 および Definition 2.7)。
最小有効サンプル数に達するまではエラーではなく論文が指定する
「非有益(uninformative)」な結果を返し、デプロイされるパラメータは
running minimum によって更新のたびに単調非増加であることが保証される。

Milestone 3 では、非単調・多次元の損失に対する一般的な symmetry +
beta-stability の還元 `risksieve::nonmonotone::stability::certify` を追加した
(Angelopoulos (2026) の Theorem 1)。他の2つのコントローラと異なり、これ自体は
パラメータを探索しない — 呼び出し側が自分のアルゴリズムで既に求めたパラメータ
と、symmetryの宣言・stability evidenceを渡し、この関数はTheorem 1の前提条件を
検証して証明書を組み立てるだけ。今回実装したのはTheorem 1のみで、論文が示す
具体的な安定性の構成(離散化損失、Lipschitz損失、選択的分類、正則化ERM)は
`tasks/todo.md` に記録している。

Milestone 4 では、SCoRE-MDR の直接デプロイ判定
`risksieve::selective::evalue::risk_adjusted_evalue` と
`risksieve::selective::mdr::certify` を追加した(Bai and Jin (2026) の
Definition 3.1、Equation 4.1、Algorithm 1、Theorem 3.2)。探索型の他の
コントローラと異なり、これはリスク調整済みe値から単一のデプロイ/非デプロイ
判定を1回だけ行う。結果として得られる `E[loss * deploy] <= alpha` は
同時分布に対する周辺(marginal)の保証であり、個々の実現された判定について
の性質ではない。

Milestone 5 では、バッチ処理の SCoRE-SDR
`risksieve::selective::sdr::certify` を追加した(Bai and Jin (2026) の
Algorithm 2、Theorem 3.3)。これは汎用的な eBH 選択エンジン
(`risksieve::selective::ebh::select`)と、Milestone 4 のテスト点ごとの
e値構成をバッチ内の各項目に独立に適用したものを組み合わせて構築している
— 論文自身が示す、テスト点間で結合した構成(Equation 5.1)ではない。
Equation 5.1 の効率的計算アルゴリズム(Algorithm 3)を確信を持って抽出
できなかったこと、その正規化関数の閾値に対する単調性が自明でないことから、
今回は見送った(`tasks/todo.md` 参照)。統計的な妥当性は保たれるが、論文
自身の構成より検出力(power)は劣ると見込まれる。選択集合が空であることは
証明書として正当であり、エラーではない。`risksieve::selective::sdr::realized_selective_risk`
はラベルが判明した後の事後的な実現リスクを計算するが、証明書そのものと
混同されないよう単なる数値を返す。

Milestone 6 では、分布シフト下での重要度重み付き anytime-valid CRC
`risksieve::anytime::AnytimeShiftedController`(Hultberg, Zachariah,
Ribeiro (2026) の Theorem 4.7)と、非負かつ有限な重みの検証・診断
(合計、二乗和、実効サンプルサイズ、最小値、最大値)を担う
`risksieve::shift::importance::WeightAccumulator` を追加した。ここでの
m* は Milestone 2 のように事前計算できない — Theorem 4.7 の m* を定める
条件が実現された重みに依存するためで、代わりに実行時に「停止時刻」として
発見し、条件が最初に成立した時点で固定する。`weight_source` は必須で
暗黙のデフォルトを持たないフィールドであり、`KnownDensityRatio` は
有限サンプルの保証をフルに与えるが、`Estimated` は漸近的な保証に格下げ
される — 論文が推定された重みについて有限サンプルの妥当性を示していない
ため。重み付き SCoRE は見送った(`tasks/todo.md` 参照)。

実装の全体シーケンスは `AGENTS.md` の第7章、現時点でのテスト範囲は
`docs/validation.md` を参照。

## 学術的根拠

`risksieve` は4つの層を統合する。各層は特定の論文に対応している:

1. **有界単調損失に対する古典的コンフォーマルリスク制御** — Angelopoulos,
   Bates, Fisch, Lei, Schuster (2024), *Conformal Risk Control*, ICLR 2024,
   [arXiv:2208.02814](https://arxiv.org/abs/2208.02814)。
2. **増大するキャリブレーションデータに対するanytime-validリスク制御** —
   Hultberg, Zachariah, Ribeiro (2026), *Anytime-Valid Conformal Risk
   Control*, [arXiv:2602.04364](https://arxiv.org/abs/2602.04364)。
3. **非単調損失と多次元パラメータに対する、アルゴリズム安定性を用いたリスク
   制御** — Angelopoulos (2026), *Conformal Risk Control for Non-Monotonic
   Losses*, [arXiv:2602.20151](https://arxiv.org/abs/2602.20151)。
4. **リスク調整済みe値、MDR、SDRによる選択的デプロイ** — Bai and Jin (2026),
   *Conformal Selective Prediction with General Risk Control*,
   [arXiv:2603.24704](https://arxiv.org/abs/2603.24704)。

分布シフトへの対応は各層の明示的な拡張であり、暗黙のデフォルトではない。
完全な参考文献一覧と各定理と実装の対応表は `docs/references.md` を参照。

## スコープ

対象内: 有界スカラー損失、有限サンプルおよびanytime-validなリスク制御、
非単調損失、多次元パラメータ、選択的デプロイ(MDR/SDR)、リスク調整済みe値、
既知および推定された重要度重み(明示的にラベル付け)、決定的で検証可能な
証明書。

最初の安定版で明示的にスコープ外とするもの: 機械学習モデルの学習、生の
特徴量からの密度比の自動推定、汎用的な非線形最適化フレームワーク、
最適化アルゴリズムの安定性の自動証明、任意の概念ドリフトに対する保証、
因果推論の主張、医療機器・規制上の認証、Rust API安定化前の他言語バインディング。
完全な一覧は `AGENTS.md` 第3章を参照。

## すべての証明書が答えること

- どのリスク量が制御されているか?
- 保証は期待値か、高確率か?
- 有限サンプルか、anytime-validか?
- すべての予測に関するものか、選択・デプロイされた予測のみか?
- どの仮定が必要だったか?
- 重要度重みは既知か推定か?
- 最適化アルゴリズムの安定性は証明済みか、与えられたものか、単に推定されたものか?
- 結果は定理に基づく証明書か、漸近的な主張か、経験的な診断か?

## 使い方

```bash
cargo build
cargo test --all-features
```

```rust
use risksieve::{BoundedLoss, OpenUnitInterval, ZeroOneLoss};

let alpha = OpenUnitInterval::new("alpha", 0.1)?;
let observed = ZeroOneLoss.evaluate_checked(&"cat", &"dog")?;
assert_eq!(observed, 1.0);
```

## ライセンス

[MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) のデュアルライセンス
(いずれかを選択可能)。外部リポジトリから移入したコードやフィクスチャの
ライセンス状況は `THIRD_PARTY_NOTICES.md`、クリーンルーム実装の方針は
`AGENTS.md` 第11章を参照。

## 開発に参加する

まず `AGENTS.md` を読むこと。このクレートを統べるエンジニアリング方針
(スコープ、API原則、マイルストーンの順序、数値計算要件、テスト戦略、
引用方針、プルリクエスト要件)がすべてそこに書かれている。

英語版は [README.md](README.md) を参照。
