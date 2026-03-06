# nest-crdt

Nest VCS の CRDT (Conflict-free Replicated Data Type) エンジン。分散環境での衝突のないデータ収束を提供します。

## CRDT 型一覧

| 型 | 説明 | 主な用途 |
|----|------|----------|
| `HLC` | Hybrid Logical Clock。物理時刻+論理カウンタ+ノードIDで因果順序を追跡 | 全CRDT型のタイムスタンプ |
| `TextCrdt` | RGA ベースのテキスト CRDT。文字レベルの協調編集 | ファイル内容、Issue/Review/Document の本文 |
| `LwwRegister<T>` | Last-Writer-Wins レジスタ。HLC タイムスタンプで最新値が勝つ | タイトル、ステータス等のスカラー値 |
| `MapCrdt<V>` | Observed-Remove セマンティクスのマップ | ファイルツリー |
| `SetCrdt<T>` | Observed-Remove セット。同時追加/削除が衝突しない | ラベル、アサイニー、承認 |
| `SequenceCrdt<T>` | 追記型の順序付きリスト | コメント、履歴エントリ |

## 使い方

```rust
use nest_crdt::text::TextCrdt;
use nest_crdt::hlc::HLC;
use nest_crdt::lww::LwwRegister;

// テキスト CRDT
let mut text = TextCrdt::from_text(1, "Hello World");
let ops = text.apply_diff("Hello World", "Hello Nest!");
assert_eq!(text.to_string(), "Hello Nest!");

// 2つのノードで同時編集しても自動収束
let base = TextCrdt::from_text(1, "Hello");
let mut a = base.fork(1);
let mut b = base.fork(2);
a.insert(5, '!');
b.insert(5, '?');
a.merge(&b);
b.merge(&a);
assert_eq!(a.to_string(), b.to_string()); // 常に一致

// LWW レジスタ
let mut clock = HLC::new(1);
let mut reg = LwwRegister::new("initial".to_string(), &mut clock);
reg.set("updated".to_string(), &mut clock);
assert_eq!(reg.get(), "updated");
```

## CrdtValue トレイト

全 CRDT 値は `CrdtValue` トレイトを実装しており、`MapCrdt` 等でのネスト構成が可能です。

```rust
pub trait CrdtValue: Clone + Serialize + DeserializeOwned {
    type Op: Clone + Serialize + DeserializeOwned;
    fn apply(&mut self, op: &Self::Op);
    fn merge(&mut self, other: &Self);
}
```

## テスト

```bash
cargo test -p nest-crdt   # 13 tests
```
