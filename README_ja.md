# risksieve

予測や意思決定を、有限サンプルおよびanytime-validなコンフォーマルリスク保証で
ふるいにかけるRustライブラリ。

## 現在の状況

**Milestone 0(語彙)、Milestone 1(古典的単調CRC)、Milestone 2(anytime-valid
単調CRC)、Milestone 3(非単調CRC、部分的)、Milestone 4(SCoRE-MDR、部分的)、
Milestone 5(SCoRE-SDR)、Milestone 6(分布シフト、部分的)が完了。**
Milestone 5には論文のオプション機能であるrandomized pruning(公式実装の
`prune='hete'` / `'homo'`)とweighted SDRはまだ含まれていない。Milestone 6は
重要度重み付きanytime-valid CRCと重み付きSCoRE-MDRをカバーするが、weighted
SDR(`SCoRE_SDR_w`)はまだ含まれていない — 詳細は下記のMilestone 5・6の段落と
`docs/roadmap.md`を参照。Milestone 7(下流の実例)はまだ未実装。

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
`docs/roadmap.md` に記録している。

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
Algorithm 2、Theorem 3.3)。論文自身が示す、テスト点間で結合したe値構成
(Equation 5.1、Theorem 5.1、新しい`risksieve::selective::coupled`モジュール)
を使い、汎用的な eBH 選択エンジン(`risksieve::selective::ebh::select`)
と組み合わせて構築している。以前の構成 — Milestone 4 のe値構成をバッチ内の
各項目に独立に適用したもの、他のテスト点を一切考慮しない — は
`risksieve::selective::sdr::certify_independent` として引き続き利用できる:
これもTheorem 3.3の正当なインスタンス化である(同定理の前提条件は各e値が
個別にDefinition 3.1を満たすことのみを要求する)。比較用・後方互換用として
残している。両者は常に同じ選択集合を返すわけではない — `docs/references.md`
に、両者が異なる選択をするfixtureと、対称性により一致するfixtureの両方を
記録している。論文もこのcrateも、一方が他方を常に上回ることは証明していない。
結合構成は`Tian-Bai/SCoRE`自身の`SCoRE_SDR`との照合(30件のfixture、
`tests/score_sdr_oracle.rs`)と、SDR保証そのもののモンテカルロ・シミュレーション
(`tests/statistical_validity.rs`、このcrateのtier 4の最初のエントリ)で
検証している。選択集合が空であることは証明書として正当であり、エラーでは
ない。`risksieve::selective::sdr::realized_selective_risk`はラベルが判明した
後の事後的な実現リスクを計算するが、証明書そのものと混同されないよう単なる
数値を返す。randomized pruning(公式実装ではオプションの検出力向上策)と
weighted SDRは未実装 — `docs/roadmap.md`を参照。

Milestone 6 では、分布シフト下での重要度重み付き anytime-valid CRC
`risksieve::anytime::AnytimeShiftedController`(Hultberg, Zachariah,
Ribeiro (2026) の Theorem 4.7)と、非負かつ有限な重みの検証・診断
(合計、二乗和、実効サンプルサイズ、最小値、最大値)を担う
`risksieve::shift::importance::WeightAccumulator` を追加した。ここでの
m* は Milestone 2 のように事前計算できない — Theorem 4.7 の m* を定める
条件が実現された重みに依存するためで、代わりに実行時に「停止時刻」として
発見し、条件が最初に成立した時点で固定する。`weight_source` は必須で
暗黙のデフォルトを持たないフィールドであり、`KnownDensityRatio` は
有限サンプルの保証をフルに与えるが、`Estimated` は無条件に
`EmpiricalOnly`(経験的診断のみ)へ格下げされる — 論文がそもそも
推定された重みについて一切論じていない(既知のomegaを前提条件として
扱うのみで、定理自身がそれを緩めることはない)ため、頼れる漸近的論拠が
存在しない。

Milestone 6 ではさらに、分布シフト下での重み付き SCoRE-MDR
`risksieve::selective::evalue_weighted::weighted_risk_adjusted_evalue` と
`risksieve::selective::mdr::certify_weighted` を追加した(Bai and Jin
(2026) の Equation 6.1、Theorem 6.2・6.4)。`KnownDensityRatio` は
`MarginalDeploymentRisk`(unweighted MDRと同じ有限サンプル保証)を返す。
`Estimated` は、Theorem 6.4の4条件——calibrationから独立に学習された
weight estimator、L2(P_X)一致性(`WeightConsistencyEvidence`)、論文の
閾値関数の正則性(`ThresholdRegularityEvidence`)、そして`gamma == alpha`
の厳密な一致——が**すべて**宣言された場合に限り`Asymptotic`を返し、
一つでも欠ければ`EmpiricalOnly`へ格下げする。これは
`risksieve::anytime::AnytimeShiftedController`(上記、無条件に
`EmpiricalOnly`)よりも厳格な、定理ごとの個別判定である——根拠となる
定理(Theorem 4.7)自体が推定weightの漸近論拠を持たないanytimeの場合と
異なり、weighted MDRのTheorem 6.4には実際に(条件付きの)漸近論拠が
存在するため。両コントローラとも
`ExchangeabilityAssumption::CovariateShiftIid`(calibrationはP、testは
異なるQからそれぞれi.i.d.)を記録する——両者とも実際には成立しない
同一分布Iidとは異なる主張である。キャリブレーション点はテスト点だけで
なく個別に重み付けされ(`w(X_i)`)、重みには正規化の要件がない — 全ての
重み(キャリブレーションとテスト点)を同じ正の定数で一律にリスケール
してもe値は不変だが、不均一な再重み付けに対しては不変ではなく、また
巨大だが有限な重み(例えばf64::MAX付近)がオーバーフローしないよう、
計算前に全重みをその最大値で正規化している。e値は、狭く非退化な特定の
ケースで`f64::INFINITY`になり得る(oracle fixtureを生成する過程で
見つかった具体的な事例であり、仮説上のものではない —
`docs/references.md`の「Equation 6.1 audit」を参照)。これは大きな
有限値へクランプするのではなく、専用の`EValue`型(`Finite(NonNegative)`
/ `PositiveInfinity`)で表現している——この型は`certificate.rs`に
定義されており、`Diagnostics::risk_adjusted_evalue`がserde機能下で
`Finite`/`PositiveInfinity`/`None`を区別して往復できる。`Tian-Bai/SCoRE`
自身の`SCoRE_MDR_w`との照合(38件のfixture、109テスト点、
`tests/score_mdr_w_oracle.rs` — 公式パッケージには重み付きe値関数自体が
存在しないため、1件につき2種類の独立した比較を実施。公式判定は
gammaとalphaの大小に関わらず全ケースで厳密一致を検証しており、これは
仮定ではなく30万試行のランダム探索で確認済み)と、重み付きMDR保証その
もののモンテカルロ・シミュレーション
(`tests/statistical_validity_weighted_mdr.rs`)で検証している。weighted
SDR は見送った(`docs/roadmap.md` 参照)。

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
