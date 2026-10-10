# Proposal 0001: Rust による z/Architecture (z/OS 対応) エミュレータ開発作業計画

- 状態: 提案 (Draft)
- 対象リポジトリ: Zeus
- ライセンス: GPL-3.0 (リポジトリ既定)

## 1. 目的

IBM メインフレーム (System/370 → ESA/390 → z/Architecture) のハードウェアを Rust で
エミュレートし、最終的に z/OS を IPL・稼働させられるエミュレータ **Zeus** を実装する。

開発中の正しさの検証には、以下の 2 種類の「比較対象 (オラクル)」を常に用いる。

1. **Hercules (SDL Hyperion)** — 既存の実績あるエミュレータ。命令単位・システム単位で
   Zeus と同じ入力を与え、結果を突き合わせる差分テストの基準とする。
2. **公開 (パブリックドメイン化) された旧 IBM ホスト OS** — MVS 3.8j、VM/370 などを
   実ワークロードとして IPL させ、Hercules 上での挙動・出力と一致することを確認する。

### 1.1 「z/OS エミュレータ」の定義

本計画では **ハードウェア (マシン) レベルのエミュレータ** を作る。すなわち
z/Architecture Principles of Operation (SA22-7832, 以下 PoO) に定義された CPU・記憶域・
チャネルサブシステム・周辺装置をエミュレートし、その上で OS をそのまま動かす。
z/OS の API (SVC やマクロ) をホスト OS 上で再実装する「OS パーソナリティ型」は本計画の
スコープ外とする (将来の別提案として検討余地は残す)。

### 1.2 スコープ外・前提

- z/OS 本体・IBM のライセンスプログラムはリポジトリに含めない。z/OS での検証は
  正規ライセンス (ADCD 等) を持つ開発者が手元で行う任意検証とし、CI には載せない。
- Coupling Facility / Parallel Sysplex、ハードウェア暗号カード (Crypto Express)、
  zIIP/zAAP の課金上の区別などは初期スコープ外。

## 2. 比較テスト対象 (オラクル) の詳細

| オラクル | アーキテクチャ | 用途 | 入手性 |
|---|---|---|---|
| Hercules SDL Hyperion 4.x | S/370, ESA/390, z/Arch | 命令単位差分・トレース突き合わせ・システム出力比較の基準 | OSS (QPL) |
| MVS 3.8j (TK4- / TK5 配布) | S/370 | 主力のシステムテスト (IPL, JES2, ジョブ実行, TSO) | パブリックドメイン |
| VM/370 Release 6 (VM/370 Community Edition) | S/370 | DAT・仮想マシン・特権命令の網羅的な負荷 | パブリックドメイン |
| DOS/VS Release 34, OS/360 21.8 | S/360/370 | 初期段階の軽量 IPL テスト | パブリックドメイン |
| Linux on IBM Z (Debian s390x 等) | z/Arch (64bit) | z/Architecture モードの OSS 実ワークロード | OSS |
| QEMU s390x (TCG / linux-user) | z/Arch | 問題状態命令の第 2 オラクル (Hercules と食い違う場合の裁定用) | OSS |
| z/OS (ADCD 等) | z/Arch | 最終目標の検証 (ライセンス保持者のみ・任意) | 商用 |

方針:

- **仕様の正は PoO**。Hercules はあくまで「強力な参照実装」であり、Hercules 自体の
  既知の不具合・未実装は `docs/oracle-discrepancies.md` に記録し、PoO に従う。
- MVS 3.8j / VM/370 は S/370 モードしか使わないため、ESA/390・z/Arch モードの
  実ワークロード検証は Linux (s390 / s390x) で補う。
- **ライセンス上の注意**: Hercules は Q Public License であり GPL-3.0 と非互換のため、
  **ソースコードの流用・移植は行わない** (クリーンルーム実装)。Hercules は
  外部プロセスとして実行し、ブラックボックスのオラクルとしてのみ使う。
  一方、Hercules の DASD イメージ形式 (CKD/CCKD/CCKD64) や AWS/HET テープ形式の
  **読み書き互換**は、同じディスクイメージを両者で使えるよう実装する (形式仕様に基づく)。
- OS イメージ・Hercules バイナリはリポジトリにコミットせず、`tools/fetch-oracles`
  スクリプトで取得・ハッシュ検証してキャッシュする。

## 3. アーキテクチャ設計 (Rust ワークスペース構成案)

```
zeus/
├─ crates/
│  ├─ zeus-arch      # 命令定義テーブル(データ駆動), ファシリティビット, アーキモード
│  ├─ zeus-cpu       # PSW, GR/AR/CR/FPR/VR, 命令デコード&実行, 割り込み処理
│  ├─ zeus-mem       # 主記憶, 記憶保護キー, プレフィックス, DAT(S/370/ESA/z), TLB
│  ├─ zeus-fp        # HFP / BFP / DFP 演算 (ソフトウェア実装, ビット厳密)
│  ├─ zeus-io        # S/370 チャネル(SIO/TIO/HIO) と CSS(SSCH/TSCH/ORB/IRB/SCHIB), CCW 処理
│  ├─ zeus-devices   # 3215/1052 コンソール, 2540 カード, 1403 プリンタ, 3270(TN3270),
│  │                 # 3420/3480 テープ(AWS/HET), CKD 3330/3350/3380/3390, 3088 CTC, OSA(QDIO)
│  ├─ zeus-system    # TOD/CPU タイマ/クロックコンパレータ, SIGP/MP, SCLP, IPL, 構成管理
│  ├─ zeus-trace     # 共通トレース形式(命令/割り込み/I/O), Hercules トレースのパーサ
│  └─ zeus-cli       # 実行バイナリ, 設定ファイル(Hercules .cnf のサブセット互換), コンソール
├─ tests/
│  ├─ insn/          # 命令単位テスト (PoO の例題・条件コード網羅)
│  ├─ diff/          # Hercules との差分テストハーネス
│  └─ system/        # OS IPL・ジョブ実行の E2E テスト (オラクル取得済み時のみ実行)
├─ tools/            # fetch-oracles, 命令表生成, トレース比較ツール
└─ docs/
```

設計上の主要方針:

1. **データ駆動の命令表**: 命令 (ニーモニック, オペコード, 形式 RR/RX/RS/SI/SS/RIL/RXY/VRR…,
   要求ファシリティ, 特権区分) を TOML/CSV で管理し、`build.rs` でデコードテーブルと
   ディスパッチ用関数ポインタ表を生成する。z/Arch の命令数は 1,000 超のため、
   実装済み/未実装のカバレッジもこの表から自動集計する。
2. **アーキテクチャモードとファシリティ**: S/370・ESA/390・z/Arch をランタイムで切替え、
   命令の可否は STFLE ファシリティビットで制御する (ARCHLVL 相当の切替え)。
3. **決定的実行モード**: TOD・タイマを「実行命令数ベースの仮想時間」で駆動するモードを
   持つ。差分テストと不具合再現の前提条件。通常実行時は実時間モード。
4. **安全性**: `unsafe` は主記憶アクセスの高速パス等に限定し、モジュール単位で
   理由をコメントで明記。ビッグエンディアンアクセスはラッパ型で統一する。
5. **性能は後回しだが設計で塞がない**: まずはインタプリタ (match/テーブルディスパッチ)。
   DAT 変換キャッシュ、命令デコードキャッシュ、将来の JIT を想定した
   「ブロック単位実行」インタフェースを Phase 1 から用意しておく。
6. **MP**: 最初は単一 CPU。SIGP と MP は Phase 3 で導入し、ホストスレッド 1 CPU/1 スレッド、
   記憶域のシリアライズ (CS/CDS/PLO, インターロック更新) を atomics で実装する。

## 4. テスト戦略

テストは 4 層で構成し、すべての層で Hercules をオラクルとして使う。

### 4.1 L1: 命令単位テスト (CI 常時実行)
- 各命令について PoO の定義から期待値を手書きした単体テスト (CC, 例外, 境界値)。
- HFP/BFP/DFP は丸めモード・例外フラグを含むテストベクタを整備
  (BFP は IEEE 754 のテストベクタ集も活用)。

### 4.2 L2: 命令差分ファジング (CI 常時・夜間拡大)
- ランダム命令列 + 初期レジスタ/記憶域状態を生成し、スタンドアロンのテストプログラム
  (IPL 可能なカード/ディスクイメージ) として Zeus と Hercules の両方で実行。
- 終了時 (またはチェックポイント命令ごと) にレジスタ・PSW・記憶域ダンプを比較。
- 不一致は自動で最小化 (delta debugging) してリグレッションテストに追加。
- 問題状態の z/Arch 命令は QEMU linux-user でも実行し、3 者多数決で判定補助。

### 4.3 L3: トレース突き合わせ (ロックステップ比較)
- Hercules の命令トレース出力を `zeus-trace` で共通形式に変換し、Zeus のトレースと
  命令ごとに比較して **最初の乖離点** を報告するツール (`zeus-tracediff`) を作る。
- 非同期割り込み (I/O 完了・タイマ) のタイミングは両者で一致しないため、
  (a) 最初の非同期割り込みまでの完全一致、(b) 以降は SVC・I/O 開始などの
  同期点単位での状態比較、の 2 段階で扱う。
- 主に MVS 3.8j / VM/370 の IPL シーケンスのデバッグに使う。

### 4.4 L4: システム E2E テスト (夜間)
- **MVS 3.8j (TK4-/TK5)**: IPL → JES2 起動 → カードリーダからジョブ投入
  (IEFBR14, IEBGENER, IFOX00 アセンブル, リンケージエディタ, COBOL/PL/I F コンパイル&実行,
  SORT) → プリンタ出力を Hercules 上で得た出力 (ゴールデン) と比較。
  日付・時刻・CPU 時間などの可変部分はマスクして比較。
- **VM/370 CE**: IPL → CP ログオン → CMS で EXEC/コンパイル → 出力比較。
- **Linux s390 / s390x**: カーネル起動 → ログイン → 簡易ベンチ・自己テスト
  (例: kernel の `kselftest` の一部, glibc テスト) を Hercules 上の結果と比較。
- **z/OS (任意)**: ライセンス保持者のみ手元で実行。IPL 到達段階
  (NIP → マスタースケジューラ → JES2 → TSO ログオン) を記録する手順書を整備。

### 4.5 指標
- 命令カバレッジ (命令表から自動算出), L2 の不一致件数推移, 各 OS の到達マイルストーン,
  MIPS (同一ホストでの Hercules 比)。

## 5. フェーズ別作業計画

期間は 1〜2 名 + AI 支援体制を想定した目安。各フェーズは「完了条件」を満たすまで次へ進まない。

### Phase 0: 基盤整備 (2〜3 週)
- Cargo ワークスペース、CI (fmt / clippy / test)、ドキュメント構成。
- Hercules を CI コンテナでビルド・実行可能にする。`tools/fetch-oracles` で
  TK4-/TK5, VM/370 CE 等を取得・ハッシュ検証。
- 命令表フォーマット確定と `build.rs` によるデコーダ生成の雛形。
- 共通トレース形式の定義と Hercules トレースパーサの最初の版。
- **完了条件**: 空の CPU で「Hercules を起動しトレースを取得 → パース」が CI で動く。

### Phase 1: S/370 CPU コア (6〜8 週)
- PSW (BC/EC モード), 汎用・制御・浮動小数点レジスタ, 主記憶, 記憶保護キー, プレフィックス。
- S/370 の一般命令・10 進命令・HFP・特権命令, プログラム/SVC/外部/マシンチェック割り込み。
- スタンドアロンテストプログラムを IPL する最小 IPL 経路 (カードリーダのみ)。
- L1・L2 ハーネス稼働開始。
- **完了条件**: S/370 命令の L2 差分ファジングで 1,000 万命令規模の不一致ゼロ。

### Phase 2: S/370 I/O とデバイス (6〜8 週)
- S/370 チャネル (SIO/SIOF/TIO/HIO/TCH, CSW, チャネルプログラム, データチェイン/コマンドチェイン)。
- 3215 コンソール, 2540 カードリーダ/パンチ, 1403 プリンタ, 3420 テープ (AWS/HET),
  CKD DASD (2314/3330/3350/3380, Hercules CKD/CCKD 形式互換)。
- 3270 端末 (TN3270 サーバ) — TSO/CMS 用。
- **完了条件**: DOS/VS または OS/360 がコンソールに IPL メッセージを出し、
  スタンドアロンユーティリティ (DASD ダンプ/リストア等) が Hercules と同一出力。

### Phase 3: S/370 DAT・タイマ・MP → MVS 3.8j / VM/370 稼働 (8〜10 週)
- S/370 DAT (セグメント/ページテーブル, 2K/4K ページ, TLB, PTLB/IPTE/RRB)。
- TOD クロック, CPU タイマ, クロックコンパレータ, インターバルタイマ。
- SIGP と 2 CPU 構成 (MVS は MP 構成でも検証)。
- L3 トレース突き合わせツールで IPL シーケンスの乖離を潰す。
- **完了条件**: TK4- の MVS 3.8j が IPL し、L4 のジョブ群の出力が Hercules と一致。
  VM/370 CE で CMS が起動し、CMS 上のジョブ出力が一致。TSO に TN3270 でログオン可能。

### Phase 4: ESA/390 (8〜10 週)
- チャネルサブシステム (SSCH/TSCH/MSCH/STSCH/CSCH/HSCH, ORB/IRB/SCHIB, I/O 割り込みサブクラス)。
- 31bit アドレッシング, アクセスレジスタ, ESA DAT, ホーム/セカンダリ空間,
  プログラム呼び出し (PC/PT/PR), リンケージスタック, BFP。
- 3390 DASD, 3480/3490 テープ, ECKD (Define Extent / Locate Record)。
- **完了条件**: ESA/390 命令の L2 差分で不一致ゼロ。31bit 版 Linux (s390) が
  Hercules と同様にブートしシェルに到達。

### Phase 5: z/Architecture 基本 (10〜12 週)
- 64bit レジスタ・PSW, z/Arch DAT (リージョン/セグメント/ページ, 大ページ 1M/2G),
  SCLP (Read SCP Info 等), STFLE, 64bit 新命令群 (RXY/RIL/RIE 形式等)。
- Long-Displacement, Extended-Immediate, General-Instructions-Extension,
  Miscellaneous-Instruction-Extensions など z10〜z13 世代の一般命令。
- DFP (decimal floating point)。
- **完了条件**: Debian s390x がブートしログイン・パッケージ操作が可能。
  z/Arch 一般命令の L2 差分 (Hercules + QEMU) で不一致ゼロ。

### Phase 6: z/OS 稼働に必要な機能 (12〜16 週)
- 対象 z/OS 版が要求するアーキテクチャレベル (例: z/OS 3.1 は z14 以上) の
  ファシリティを命令表上で洗い出し、ギャップリストを作成して順次実装。
  - Vector Facility (z13) と Vector Enhancements (z14/z15)
  - CPACF 暗号命令 (KM/KMC/KIMD/KLMD/KMAC/PRNO 等)
  - 圧縮 (CMPSC), DFLTCC (z15) は優先度を判断
  - Interlocked-Access, Load/Store-on-Condition, Execution-Hint 等
- OSA-Express (QDIO) による TCP/IP 接続, 3390 大容量 (EAV) 対応。
- z/OS は IPL 時に特定のモデル情報 (STIDP/STSI) やファシリティを検査するため、
  STSI 応答・機種情報の構成可能化。
- **完了条件**: Linux s390x を `-march=z14`/`z15` 相当でビルドしたテスト群が通過。
  (任意) ライセンス保持者の環境で z/OS が NIP を通過しマスタースケジューラ到達 →
  最終的に TSO/ISPF ログオン。

### Phase 7: 性能・運用性 (継続)
- プロファイリングに基づく最適化 (デコードキャッシュ, DAT/TLB 高速化, ブロック実行,
  将来の JIT 検討)。目標: 同一ホストで Hercules と同等以上の MIPS。
- Hercules 設定ファイル・コンソールコマンドの互換サブセット拡充、HTTP/GUI 管理画面。
- ユーザ向けドキュメント (TK4-/Linux を動かすクイックスタート)。

## 6. 成果物とマイルストーン一覧

| マイルストーン | 到達内容 | 目安 (累積) |
|---|---|---|
| M0 | CI + オラクル環境 + トレース基盤 | 〜3 週 |
| M1 | S/370 CPU, 差分ファジング稼働 | 〜3 ヶ月 |
| M2 | S/370 I/O, 旧 OS の IPL メッセージ | 〜5 ヶ月 |
| M3 | **MVS 3.8j / VM/370 稼働, 出力一致** | 〜7.5 ヶ月 |
| M4 | ESA/390, Linux s390 (31bit) ブート | 〜10 ヶ月 |
| M5 | z/Arch, Debian s390x ブート | 〜13 ヶ月 |
| M6 | z/OS 要求機能, (任意) z/OS IPL | 〜17 ヶ月 |

## 7. リスクと対策

| リスク | 影響 | 対策 |
|---|---|---|
| オラクル (Hercules) 自体の誤り | 誤った挙動を「正」としてしまう | PoO を正とし、食い違いは QEMU・実機文献で裁定し記録 |
| ライセンス (QPL と GPL の非互換) | コード汚染 | Hercules ソースを参照・移植しない運用ルールをCONTRIBUTING に明記 |
| z/OS が検証に使えない (ライセンス) | 最終目標の検証が CI 不可 | Linux s390x で命令/ファシリティを最大限網羅し、z/OS 検証は任意の手動手順に |
| 非同期イベントの非決定性 | 差分テストが不安定 | 決定的実行モード + 同期点比較 |
| 命令数・仕様量の膨大さ | 工期超過 | 命令表駆動のカバレッジ管理, OS が実際に使う命令から優先 (トレースで頻度集計) |
| DFP・暗号の実装難度 | 正確性不良 | 既存の検証済みテストベクタ (IEEE 754-2008, NIST CAVP) を使用 |
| 性能不足 | z/OS IPL に非現実的な時間 | Phase 1 からベンチを計測、ボトルネックを継続監視 |

## 8. 直近の作業 (Phase 0 の着手タスク)

1. Cargo ワークスペースと上記クレート雛形の作成、CI (GitHub Actions) 設定。
2. `CONTRIBUTING.md` にクリーンルーム方針 (Hercules ソース非参照) と PoO 準拠を明記。
3. `tools/fetch-oracles`: Hercules SDL Hyperion のビルド, TK4-/TK5・VM/370 CE の取得とハッシュ検証。
4. 命令表 (`crates/zeus-arch/data/instructions.toml`) のスキーマ確定と S/370 命令の登録。
5. 共通トレース形式の仕様書 (`docs/design/trace-format.md`) と Hercules トレースパーサ。
6. 最初の差分テスト: 「LR/AR/BCR だけのスタンドアロンプログラム」を両者で実行・比較。

## 9. 決定が必要な事項 (レビュー依頼)

- 最終目標とする z/OS のバージョン (→ 必要なアーキテクチャレベル z14/z15/z16 が決まる)。
- 主に使う MVS 3.8j 配布 (TK4- か TK5 か)。両方サポートするかどうか。
- Hercules 設定ファイル互換をどこまで目指すか (サブセットか完全互換か)。
- 開発体制 (人数) とマイルストーン期間の妥当性。
