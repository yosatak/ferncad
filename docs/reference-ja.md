# ferncad 言語リファレンス

## 型

| 型 | 説明 | 例 |
|----|------|------|
| `int` | 整数 | `42` |
| `float` | 浮動小数点数 | `3.14` |
| `length` | 長さ (mm) | `#u(10 :mm)` |
| `angle` | 角度 (rad) | `#a(90 :deg)` |
| `vec3` | 3Dベクトル | `#v(1 0 0)` |
| `point3` | 3D点 | `#p(0 0 0)` |
| `string` | 文字列 | `"hello"` |
| `keyword` | キーワード | `:radius` |
| `bool` | 真偽値 | `t`, `nil` |
| `shape` | CSG形状 | `(box ...)` |
| `path` | パラメトリック曲線 | `(helix ...)` |
| `list` | リスト | `(list 1 2 3)` |

## 特殊形式

| 形式 | 構文 | 説明 |
|------|------|------|
| `defvar` | `(defvar name value)` | 変数定義 |
| `defun` | `(defun name (params) body...)` | 関数定義 |
| `defmacro` | `(defmacro name (params) body...)` | マクロ定義 |
| `defpart` | `(defpart name "doc" :meta ... :params ... :body ...)` | パーツ定義 |
| `defmeta` | `(defmeta :title ... :version ...)` | ファイルメタデータ |
| `let*` | `(let* ((var val) ...) body...)` | 逐次ローカル束縛 |
| `if` | `(if cond then else)` | 条件分岐 |
| `cond` | `(cond (test1 body1) (test2 body2) ...)` | 多分岐条件 |
| `when` | `(when condition body...)` | 真の場合に本体を実行 |
| `unless` | `(unless condition body...)` | 偽の場合に本体を実行 |
| `and` | `(and expr...)` | 短絡論理積（最後の真値または nil を返す） |
| `or` | `(or expr...)` | 短絡論理和（最初の真値または nil を返す） |
| `setf` | `(setf name value)` | 既存の変数を更新 |
| `lambda` | `(lambda (params) body...)` | 無名関数 |
| `quote` | `(quote expr)` | 未評価で返す |
| `quasiquote` | `` `expr `` | テンプレート（unquote 付き） |
| `progn` | `(progn body...)` | 逐次評価 |
| `dotimes` | `(dotimes (var count) body...)` | var を 0 から count-1 まで繰り返す |
| `dolist` | `(dolist (var list) body...)` | リストの各要素で繰り返す |
| `mapcar` | `(mapcar fn list)` | 各要素に fn を適用 |
| `reduce` | `(reduce fn list [initial])` | リストを畳み込む |
| `remove-if` | `(remove-if predicate list)` | 述語に一致する要素を除去 |
| `apply` | `(apply fn args-list)` | 引数リストに fn を適用 |
| `assembly` | `(assembly "name" body...)` | アセンブリ定義 |
| `require` | `(require "module-name")` | モジュール読み込み（ユーザーファイルまたはビルトイン） |
| `export` | `(export name1 name2 ...)` | シンボルのエクスポート |

## 準クォート構文

| 構文 | 展開 | 説明 |
|------|------|------|
| `` `expr `` | `(quasiquote expr)` | テンプレート |
| `,expr` | `(unquote expr)` | 評価して挿入 |
| `,@expr` | `(unquote-splicing expr)` | 評価してリスト展開 |

## プリミティブ形状

| 関数 | 引数 | 説明 |
|------|------|------|
| `box` | `:width :depth :height` | 直方体 |
| `cube` | `size` | 立方体 |
| `sphere` | `:radius` | 球 |
| `cylinder` | `:radius :height` | 円柱 |
| `cone` | `:radius-bottom :radius-top :height` | 円錐/截頭円錐 |
| `torus` | `:radius-major :radius-minor` | トーラス |
| `prism` | `:sides :radius :height` | 正多角柱 |

## プロファイル操作

| 関数 | 引数 | 説明 |
|------|------|------|
| `polygon` | `(list x y) ...` | 2D点列の作成 |
| `circle` | `:radius :segments` | 円のポリゴン近似 |
| `extrude` | `:profile :height` | 2DプロファイルをZ軸方向に押し出し |
| `revolve` | `:profile :angle :segments` | 2DプロファイルをZ軸周りに回転 |

## パス（経路）

| 関数 | 引数 | 説明 |
|------|------|------|
| `helix` | `:radius :pitch :turns` | 螺旋パス |
| `arc` | `:radius :angle` | XY平面上の円弧 |
| `bezier` | `:points` | ベジェ曲線 |

**パスのパラメータ：**
- `helix` — `:radius` 螺旋の半径、`:pitch` 1回転あたりのZ方向進み量、`:turns` 回転数
- `arc` — `:radius` 円弧の半径、`:angle` 角度（デフォルト: 2π）
- `bezier` — `:points` 制御点のリスト `(list (list x y z) ...)`

## スイープ / ロフト

| 関数 | 引数 | 説明 |
|------|------|------|
| `sweep` | `:profile :path :segments` | 2Dプロファイルを3Dパスに沿って掃引 |
| `loft` | `:profiles :at :segments` | 複数の2D断面間を補間 |

**sweep の詳細：**
- `:profile` — `polygon` または `circle` で作った2D点列
- `:path` — `helix`、`arc`、`bezier` で作ったパス
- `:segments` — パスの分割数（省略時: `*resolution*` の値）

**loft の詳細：**
- `:profiles` — 2Dプロファイルのリスト `(list profile1 profile2 ...)`
- `:at` — 各断面のZ位置 `(list z1 z2 ...)`（プロファイル数と同数）
- `:segments` — 断面間の補間分割数（省略時: `*resolution*` の値）

## CSG 演算

| 関数 | 引数 | 説明 |
|------|------|------|
| `union` | `shape1 shape2 ...` | 和集合 |
| `difference` | `base cutter1 ...` | 差集合 |
| `intersection` | `shape1 shape2 ...` | 積集合 |

## 変換

| 関数 | 引数 | 説明 |
|------|------|------|
| `translate` | `:shape :by` | 平行移動 |
| `rotate` | `:shape :axis :angle` | 回転 |
| `scale` | `:shape :factor` | 拡大縮小 |

## エッジ操作

| 関数 | 状態 | 説明 |
|------|------|------|
| `chamfer` | 部分対応 | `:shape :distance`（プレースホルダ） |
| `fillet` | 未対応 | truck 0.6 の制約 |
| `shell` | 未対応 | truck 0.6 の制約 |

## 算術・数学関数

`+`, `-`, `*`, `/`, `cos`, `sin`, `tan`, `acos`, `atan`, `atan2`, `sqrt`, `abs`, `mod`, `expt`, `floor`, `ceil`, `min`, `max`, `pi`

## 角度コンストラクタ

`#a(N :deg)` リテラルの実行時版。実行時に計算した角度値を `:angle` 等に渡すときに使う。

| 関数 | 引数 | 説明 |
|------|------|------|
| `deg` | `(deg n)` | `n` を度として解釈し、ラジアンを返す |
| `rad` | `(rad n)` | `n` をラジアンとしてそのまま返す |

## 比較・論理演算

`=`, `<`, `>`, `<=`, `>=`, `not`

## リスト操作

| 関数 | 引数 | 説明 |
|------|------|------|
| `list` | `(list a b ...)` | リストを作成 |
| `cons` | `(cons item list)` | リストの先頭に追加 |
| `car` | `(car list)` | 最初の要素 |
| `cdr` | `(cdr list)` | 先頭を除いた残り |
| `append` | `(append list1 list2 ...)` | リストを連結 |
| `nth` | `(nth n list)` | n番目の要素（0始まり） |
| `length` | `(length list)` | 要素数 |
| `reverse` | `(reverse list)` | リストを逆順に |
| `last` | `(last list)` | 最後の要素 |
| `iota` | `(iota n)` | リスト `(0 1 2 ... n-1)` |
| `assoc` | `(assoc key alist)` | alist の中で car が `key` と一致する最初の対（無ければ `nil`） |
| `getf` | `(getf plist key [default])` | plist 内で `key` の次の値（無ければ `default` か `nil`） |
| `find-if` | `(find-if pred list)` | `pred` が真になる最初の要素（無ければ `nil`） |
| `every` | `(every pred list)` | 全要素が `pred` を満たせば `t` |

## 関数ユーティリティ

| 関数 | 引数 | 説明 |
|------|------|------|
| `memoize` | `(memoize fn)` | 同じ引数なら同じ `Arc` を返すラッパを作る。再帰形状ビルダが realize 段階のポインタキャッシュに自動的に乗り、テッセレーションを共有できる |

## 型述語

`numberp`, `listp`, `nilp`, `stringp`

## 文字列操作

| 関数 | 引数 | 説明 |
|------|------|------|
| `format` | `(format template args...)` | 文字列フォーマット（`~a` 任意, `~d` 整数, `~f` 浮動小数点, `~%` 改行） |

## アセンブリ

| 関数 | 引数 | 説明 |
|------|------|------|
| `place` | `:part :as :at` | パーツを配置 |
| `mate` | `face1 face2` | 2つの面を合わせる |
| `align-axis` | `axis1 axis2` | 2つの軸を揃える |
| `face` | `:part-name :face-name` | 面の参照 |
| `axis` | `:part-name :axis-name` | 軸の参照 |

## 単位リテラル

```lisp
#u(10 :mm)    ; 10 mm
#u(1 :inch)   ; 25.4 mm
#u(2 :cm)     ; 20 mm
#a(90 :deg)   ; π/2 rad
#a(1 :turn)   ; 2π rad
#v(1 0 0)     ; 3Dベクトル
#p(10 20 30)  ; 3D点
```

## グローバル変数

| 変数 | デフォルト | 説明 |
|------|-----------|------|
| `pi` | 3.14159... | 円周率 |
| `*resolution*` | `16` | 曲面のデフォルトセグメント数 |

メッシュ品質の制御：

```lisp
(defvar *resolution* 64)  ; 高い値 = 滑らか・遅い
```

## CLI

```bash
ferncad <input.fern>                         # 評価して結果表示
ferncad <input.fern> --stl <out.stl>         # STL 出力（アセンブリ→パーツ別）
ferncad <input.fern> --step <out.step>       # STEP 出力
ferncad <input.fern> --segments 64 --stl ... # メッシュ解像度を指定
```
