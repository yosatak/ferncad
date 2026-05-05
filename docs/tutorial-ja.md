# ferncad チュートリアル

Lisp風構文で3D CADモデリングを行うための入門ガイドです。

## はじめに

```bash
# ビルド
cargo build --workspace

# Web ビューアの起動
cd crates/ferncad-wasm && wasm-pack build --target web
cd web && npm install && npm run dev
# ランディングページ: http://localhost:5173/
# エディタ:           http://localhost:5173/app/
```

## 基本形状

すべてのプリミティブは原点中心に生成されます。

```lisp
;; 直方体
(box :width 20 :depth 15 :height 10)

;; 球
(sphere :radius 10)

;; 円柱
(cylinder :radius 5 :height 20)

;; 円錐
(cone :radius-bottom 8 :radius-top 0 :height 15)

;; トーラス
(torus :radius-major 15 :radius-minor 3)

;; 正多角柱（六角柱）
(prism :sides 6 :radius 10 :height 5)
```

## CSG（集合演算）

ブーリアン演算で形状を組み合わせます：

```lisp
;; 和集合：結合
(union
  (box :width 20 :depth 20 :height 10)
  (cylinder :radius 5 :height 20))

;; 差集合：切り取り
(difference
  (box :width 20 :depth 20 :height 20)
  (sphere :radius 12))

;; 積集合：重なる部分のみ
(intersection
  (box :width 15 :depth 15 :height 15)
  (sphere :radius 10))
```

## 変換

```lisp
;; 移動
(translate :shape (box :width 10 :depth 10 :height 10)
           :by (list 20 0 0))

;; 回転（角度はラジアン、#a で度数指定も可）
(rotate :shape (box :width 10 :depth 10 :height 10)
        :axis :z
        :angle #a(45 :deg))

;; 拡大縮小
(scale :shape (sphere :radius 5)
       :factor 2.0)
```

## 変数と関数

```lisp
;; 定数（Common Lisp 慣例：+name+）
(defvar +plate-width+ 50)
(defvar +plate-height+ 5)

;; 関数定義
(defun make-plate (w d h)
  (box :width w :depth d :height h))

(make-plate +plate-width+ +plate-width+ +plate-height+)
```

## パーツ定義 (defpart)

再利用可能なパラメトリックパーツを定義します：

```lisp
(defpart bracket
  "L字ブラケット"
  :meta (:category :structural
         :material :aluminum
         :description "L型ブラケット")
  :params ((width     :: length :default 30.0 :doc "幅")
           (height    :: length :default 40.0 :doc "高さ")
           (thickness :: length :default 3.0  :doc "板厚"))
  :body
  (let* ((base (box :width width :depth width :height thickness))
         (wall (box :width thickness :depth width :height height))
         (wall-placed (translate :shape wall
                                 :by (list (/ (- width thickness) 2) 0 (/ height 2)))))
    (union base wall-placed)))

;; デフォルト値で使用
(bracket)

;; パラメータ指定
(bracket :width 50 :height 60 :thickness 5)
```

## プロファイルと押し出し

2Dプロファイルから3D形状を生成します：

```lisp
;; ポリゴンをZ軸方向に押し出し
(extrude
  :profile (polygon (list 0 0) (list 20 0) (list 20 10)
                    (list 10 10) (list 10 5) (list 0 5))
  :height 15)

;; プロファイルをZ軸周りに回転（花瓶形状）
(revolve
  :profile (polygon (list 5 0) (list 10 0) (list 12 10)
                    (list 8 20) (list 5 20))
  :angle #a(360 :deg))

;; 円のプロファイル（polygon の便利関数）
(circle :radius 5 :segments 32)
```

## パス（経路）とスイープ

2Dプロファイルを3Dパスに沿って掃引（スイープ）します。
ネジ山、パイプ、スプリングなど複雑な形状を作れます。

### パスの種類

```lisp
;; 螺旋（ヘリックス）
(helix :radius 5 :pitch 2 :turns 3)

;; 円弧
(arc :radius 10 :angle (/ pi 2))

;; ベジェ曲線
(bezier :points (list (list 0 0 0) (list 10 10 0) (list 20 0 10)))
```

### スイープの基本

```lisp
;; 円断面を螺旋に沿ってスイープ → スプリング
(sweep :profile (circle :radius 0.5 :segments 12)
       :path    (helix :radius 5 :pitch 3 :turns 4)
       :segments 128)

;; 三角断面を円弧に沿ってスイープ → 曲がったパイプ
(sweep :profile (polygon (list -1 0) (list 0 1) (list 1 0))
       :path    (arc :radius 10 :angle (/ pi 2))
       :segments 32)
```

### ネジ山の実例

ISO メトリック M3 ネジを作成します：

```lisp
(defvar +pitch+   0.5)   ; ピッチ (mm)
(defvar +major-r+ 1.5)   ; おねじ外径
(defvar +minor-r+ 1.221) ; 谷径

(let* ((length 10)
       (turns (/ length +pitch+))
       (tooth-h (- +major-r+ +minor-r+))

       ;; ネジ山断面（三角形） — ISO 60°
       (thread-profile (polygon
         (list 0.0       (- 0 (/ +pitch+ 4)))
         (list tooth-h   0.0)
         (list 0.0       (/ +pitch+ 4))))

       ;; 螺旋パスに沿ってスイープ
       (thread (sweep :profile thread-profile
                      :path (helix :radius +minor-r+
                                   :pitch +pitch+
                                   :turns turns)
                      :segments (* turns 24)))

       ;; シャフト（谷径の円柱）
       (shaft (cylinder :radius +minor-r+ :height length))
       (shaft-up (translate :shape shaft
                            :by (list 0 0 (/ length 2)))))

  (union shaft-up thread))
```

## ロフト

複数の2D断面間を補間して3D形状を生成します：

```lisp
;; 円から四角へ遷移
(loft :profiles (list (circle :radius 5 :segments 32)
                      (polygon (list -3 -3) (list 3 -3)
                               (list 3 3) (list -3 3)))
      :at (list 0 20)
      :segments 16)

;; ボトル形状：3つの円断面
(loft :profiles (list (circle :radius 4 :segments 24)
                      (circle :radius 5 :segments 24)
                      (circle :radius 2 :segments 24))
      :at (list 0 10 30)
      :segments 16)
```

## メッシュ解像度

曲面の細かさを制御します。

```lisp
;; .fern ファイルの先頭で設定
(defvar *resolution* 64)  ; デフォルトは 16
```

CLI から指定することもできます：
```bash
ferncad model.fern --segments 64 --stl output.stl
```

| 値 | 用途 |
|----|------|
| `16` | 高速プレビュー |
| `32` | 標準品質 |
| `64` 以上 | 3Dプリント向け |

## `memoize` で再帰形状を共有

ferncad は realize 段階で `Arc<ShapeNode>` のポインタ同一性をキーにメッシュをキャッシュします。
同じ shape 値を複数の `translate` に渡すと内部の三角形分割は 1 度で済みますが、構造的に
同じでも別の `Arc` だと別計算になります。`memoize` は形状ビルダをラップして「同じ引数なら同じ
`Arc`」を返すので、再帰サンプルが共有を意識せずに書けます。

```lisp
;; pinna(size) は同じ size に対して 1 度だけテッセレーションされる
(defvar pinna
  (memoize
    (lambda (len)
      (extrude :profile (polygon (list 0 0) (list len 0) (list 0 len))
               :height (* len 0.1)))))

;; 同じサイズで何度呼んでも同じ Arc が返る
(union (pinna 1.0) (translate :shape (pinna 1.0) :by #v(2 0 0)))
```

`memoize` の対象はラムダ・`defun` の結果・別の memoize 結果（callable 全般）です。
ラップする関数は副作用を持たないこと（最初の呼び出し以外では実行されない）。

`iota` と `mapcar` の組み合わせは「インデックス駆動」の配置を書くイディオムです：

```lisp
(mapcar (lambda (i)
          (translate :shape (sphere :radius 1)
                     :by (list (* i 3) 0 0)))
        (iota 6))   ; → X 軸に 6 個の球を並べる
```

## マクロ

`defmacro` とクォーズクォートでコード変換を定義します：

```lisp
;; "when" マクロ（Common Lisp 風）
(defmacro when (condition &rest body)
  `(if ,condition (progn ,@body)))

;; オフセットマクロ
(defmacro with-offset (shape x y z)
  `(translate :shape ,shape :by (list ,x ,y ,z)))
```

## アセンブリ

複数のパーツを組み合わせます。Web UI でパーツごとに色分け表示されます。

```lisp
(assembly "plate-with-bolts"
  (place :part (box :width 40 :depth 40 :height 5) :as :plate)
  (place :part (m3-bolt :length 12) :as :bolt-1
         :at #p(10 10 5))
  (place :part (m3-bolt :length 12) :as :bolt-2
         :at #p(-10 -10 5)))
```

## マルチファイルプロジェクト

Web UI はマルチファイルプロジェクトに対応しています。左サイドバーのファイルエクスプローラで
`.fern` ファイルの作成・リネーム・削除が可能です。

ファイル間の参照には `require` を使います：

```lisp
;; utils.fern
(defun plate (w d h)
  (box :width w :depth d :height h))

;; main.fern（エントリーポイント — 常に最初に評価）
(require "utils")
(plate 40 40 5)
```

`require` は以下の順に名前を解決します：
1. プロジェクトファイル（完全一致、次に `.fern` 拡張子付き）
2. ビルトイン標準ライブラリモジュール

プロジェクトはブラウザの IndexedDB に保存されます。ツールバーのボタンで
プロジェクトの新規作成・保存・読込が可能です。

## 標準ライブラリ

`require` で読み込み可能なモジュール：

| モジュール | 説明 |
|-----------|------|
| `ferncad-std/m3-bolt` | M3 六角穴付きボルト (JIS B 1176) |
| `ferncad-std/m3-nut` | M3 六角ナット (JIS B 1181) |
| `ferncad-std/m4-bolt` | M4 六角穴付きボルト |
| `ferncad-std/m5-bolt` | M5 六角穴付きボルト |
| `ferncad-std/flat-washer` | 平ワッシャー (JIS B 1256) |
| `ferncad-std/spring-washer` | スプリングワッシャー (JIS B 1251) |

## エクスポート

- **STL**: 三角メッシュ（3Dプリント用）
- **STEP**: 正確な BREP 形状（CADソフト交換用）

```bash
ferncad input.fern --stl output.stl              # STL 出力
ferncad input.fern --step output.step             # STEP 出力
ferncad input.fern --segments 64 --stl output.stl # 高品質 STL
```

アセンブリはパーツ別にSTLが出力されます：
```bash
ferncad assembly.fern --stl output.stl
# → output-bolt.stl, output-nut.stl, ...
```
