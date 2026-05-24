//! ジェネレータコンビネータ (いわゆる「strategy」) です。
//!
//! [`Arbitrary`] は型ごとに 1 つの正規ジェネレータがあれば十分な場合に便利ですが、
//! 実際のテストではアドホックなジェネレータ (「0 から 100 までの `i32`」、
//! 「空でない `Vec`」、「これら 3 つのうちのいずれか」など) がよく必要になります。
//! このモジュールは、そのような目的のために合成可能な [`Strategy`] 値を提供します。
//!
//! # 例
//!
//! ```
//! use propcheck_core::strategy::{any, int_range, vec_of, one_of, just, Strategy, StrategyExt};
//!
//! let small_positives = int_range(1i32..100);
//! let small_vec       = vec_of(small_positives, 1..5);
//! let constants       = one_of(vec![just(0i32), just(42i32)]);
//! let even_ints       = any::<i32>().filter(|n| n % 2 == 0);
//! # let _ = (small_vec, constants, even_ints);
//! ```

use crate::arbitrary::Arbitrary;
use crate::rng::Rng;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Range;

/// 文字列生成 strategy 群 (`ascii_alphanumeric`、`hex_string` など)。
///
/// 完全な一覧については [`strategy_str`](crate::strategy_str) モジュールを参照してください。
pub mod str {
    pub use crate::strategy_str::*;
}

/// [`Strategy::Value`] 型の値を生成し shrink する方法を知る、合成可能なジェネレータです。
pub trait Strategy {
    /// この strategy が生成する値の型です。
    type Value: Clone + Debug + 'static;

    /// 新しい値を生成します。
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> Self::Value;

    /// 与えられた値より厳密に小さい候補のリストを返します。
    /// 空の vec を返すことは「既知の shrink がない」ことを意味します。
    fn shrink_value(&self, value: &Self::Value) -> Vec<Self::Value>;
}

/// 任意の [`Strategy`] の上に重ねるコンビネータヘルパーです。
pub trait StrategyExt: Strategy + Sized {
    /// 生成された値を `f` で変換します。`f` は一般に可逆ではないため、
    /// shrink は失われます。
    fn map<F, U>(self, f: F) -> Map<Self, F, U>
    where
        F: Fn(Self::Value) -> U + Clone + 'static,
        U: Clone + Debug + 'static,
    {
        Map {
            inner: self,
            f,
            _phantom: PhantomData,
        }
    }

    /// `pred` に一致する値のみを残します。この strategy は `new_value`
    /// の呼び出しごとに最大 `max_tries` 回まで再試行し、その後 panic します。
    fn filter<F>(self, pred: F) -> Filter<Self, F>
    where
        F: Fn(&Self::Value) -> bool + 'static,
    {
        Filter {
            inner: self,
            pred,
            max_tries: 256,
        }
    }

    /// 先に生成された値に依存する値を持つ新しい strategy を構築します。
    /// 「まず長さを選び、次にその長さの Vec を生成する」といった依存生成パターンに
    /// 便利です。
    ///
    /// shrink は `flat_map` を越えて保持されません。詳細は [`FlatMap`] の
    /// ドキュメントを参照してください。
    fn flat_map<F, S2>(self, f: F) -> FlatMap<Self, F, S2>
    where
        F: Fn(Self::Value) -> S2 + 'static,
        S2: Strategy,
    {
        FlatMap {
            inner: self,
            f,
            _phantom: PhantomData,
        }
    }

    /// [`one_of`] のような異種コレクション内で利用するために、具体的な型を
    /// 消去します。
    fn boxed(self) -> BoxedStrategy<Self::Value>
    where
        Self: 'static,
    {
        BoxedStrategy {
            inner: Box::new(self),
        }
    }
}

impl<S: Strategy> StrategyExt for S {}

// --- any<T>: Arbitrary に委譲する ----------------------------------------

/// 型の [`Arbitrary`] 実装に委譲する strategy です。
pub struct AnyOf<T>(PhantomData<fn() -> T>);

/// [`AnyOf`] strategy を構築します。
pub fn any<T: Arbitrary + 'static>() -> AnyOf<T> {
    AnyOf(PhantomData)
}

impl<T: Arbitrary + 'static> Strategy for AnyOf<T> {
    type Value = T;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> T {
        T::arbitrary(rng, size)
    }
    fn shrink_value(&self, value: &T) -> Vec<T> {
        value.shrink().collect()
    }
}

// --- just<T>: 定数 -------------------------------------------------

/// 常に同じ値を返す strategy です。
pub struct Just<T>(T);

/// 常に `v` を返す [`Just`] strategy を構築します。
pub fn just<T: Clone + Debug + 'static>(v: T) -> Just<T> {
    Just(v)
}

impl<T: Clone + Debug + 'static> Strategy for Just<T> {
    type Value = T;
    fn new_value<R: Rng + ?Sized>(&self, _rng: &mut R, _size: usize) -> T {
        self.0.clone()
    }
    fn shrink_value(&self, _value: &T) -> Vec<T> {
        Vec::new()
    }
}

// --- int_range: [lo, hi) の数値 ------------------------------------

/// `[lo, hi)` の整数を生成し、`lo` に向けて shrink します。
/// `0 ∈ [lo, hi)` の場合は `0` に向けて shrink します。
pub struct IntRange<T> {
    lo: T,
    hi: T,
}

/// `Range<T>` から [`IntRange`] strategy を構築します。
pub fn int_range<T>(r: Range<T>) -> IntRange<T> {
    IntRange {
        lo: r.start,
        hi: r.end,
    }
}

macro_rules! impl_int_range {
    ($t:ty, $cast_to:ty, $gen_method:ident) => {
        impl Strategy for IntRange<$t> {
            type Value = $t;
            fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, _size: usize) -> $t {
                assert!(self.lo < self.hi, "int_range: lo < hi");
                rng.$gen_method(self.lo as $cast_to, self.hi as $cast_to) as $t
            }
            fn shrink_value(&self, value: &$t) -> Vec<$t> {
                let target: $t = if self.lo <= 0 && (0 as $t) < self.hi {
                    0 as $t
                } else {
                    self.lo
                };
                if *value == target {
                    return Vec::new();
                }
                let mut out: Vec<$t> = vec![target];
                let mut delta: i128 = (*value as i128) - (target as i128);
                loop {
                    delta /= 2;
                    if delta == 0 {
                        break;
                    }
                    let candidate_i = (*value as i128) - delta;
                    if candidate_i >= self.lo as i128
                        && candidate_i < self.hi as i128
                        && candidate_i != *value as i128
                    {
                        let cand = candidate_i as $t;
                        if !out.contains(&cand) {
                            out.push(cand);
                        }
                    }
                }
                out
            }
        }
    };
}

impl_int_range!(i8, i64, gen_range_i64);
impl_int_range!(i16, i64, gen_range_i64);
impl_int_range!(i32, i64, gen_range_i64);
impl_int_range!(i64, i64, gen_range_i64);
impl_int_range!(isize, i64, gen_range_i64);
impl_int_range!(u8, u64, gen_range_u64);
impl_int_range!(u16, u64, gen_range_u64);
impl_int_range!(u32, u64, gen_range_u64);
impl_int_range!(u64, u64, gen_range_u64);
impl_int_range!(usize, u64, gen_range_u64);

// --- char_range: [lo, hi) の `char` -----------------------------------

/// `[lo, hi)` の `char` を生成する strategy です。サロゲートコードポイントは
/// スキップします。shrink は `lo` に向かいます。
pub struct CharRange {
    lo: char,
    hi: char,
}

/// `Range<char>` から [`CharRange`] strategy を構築します。
pub fn char_range(r: Range<char>) -> CharRange {
    assert!(r.start <= r.end, "char_range: lo must be <= hi");
    CharRange {
        lo: r.start,
        hi: r.end,
    }
}

impl Strategy for CharRange {
    type Value = char;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, _size: usize) -> char {
        let lo = self.lo as u32;
        let hi = self.hi as u32;
        if hi <= lo {
            return self.lo;
        }
        // サロゲートコードポイント (0xD800..=0xDFFF) は char として無効。
        // 無限ループを避けるため、約 32 回まで再試行してから lo にフォールバックする。
        for _ in 0..32 {
            let code = rng.gen_range_u64(lo as u64, hi as u64) as u32;
            if let Some(c) = char::from_u32(code) {
                return c;
            }
        }
        self.lo
    }
    fn shrink_value(&self, value: &char) -> Vec<char> {
        if *value == self.lo {
            return Vec::new();
        }
        let mut out: Vec<char> = vec![self.lo];
        let v = *value as u32;
        let lo = self.lo as u32;
        let mut delta = v.saturating_sub(lo);
        loop {
            delta /= 2;
            if delta == 0 {
                break;
            }
            let cand = v - delta;
            if let Some(c) = char::from_u32(cand) {
                if c != *value && c != self.lo && !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }
}

// --- bytes: 可変長バイトスライス -------------------------------

/// 長さが `len_range` に収まる `Vec<u8>` を生成する strategy です。
/// `vec_of(any::<u8>(), len_range)` のシュガーです。
pub fn bytes(len_range: Range<usize>) -> VecOf<AnyOf<u8>> {
    vec_of(any::<u8>(), len_range)
}

// --- 浮動小数点範囲 ----------------------------------------------------

/// `[lo, hi)` の範囲で一様に浮動小数点数を生成する strategy です。
/// 範囲が含む場合は `0.0` に向けて、そうでなければ `lo` に向けて shrink します。
pub struct FloatRange<T> {
    lo: T,
    hi: T,
}

/// [`f32`] の範囲 strategy を構築します。
pub fn f32_range(r: Range<f32>) -> FloatRange<f32> {
    assert!(r.start <= r.end, "f32_range: lo must be <= hi");
    FloatRange {
        lo: r.start,
        hi: r.end,
    }
}

/// [`f64`] の範囲 strategy を構築します。
pub fn f64_range(r: Range<f64>) -> FloatRange<f64> {
    assert!(r.start <= r.end, "f64_range: lo must be <= hi");
    FloatRange {
        lo: r.start,
        hi: r.end,
    }
}

macro_rules! impl_float_range {
    ($t:ty) => {
        impl Strategy for FloatRange<$t> {
            type Value = $t;
            fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, _size: usize) -> $t {
                if self.lo >= self.hi {
                    return self.lo;
                }
                // u64 → float 変換を経て [0, 1) の一様分布。
                let bits = rng.next_u64();
                let frac = (bits as $t) / (u64::MAX as $t);
                self.lo + frac * (self.hi - self.lo)
            }
            fn shrink_value(&self, value: &$t) -> Vec<$t> {
                let target: $t = if self.lo <= 0.0 && 0.0 < self.hi {
                    0.0
                } else {
                    self.lo
                };
                if *value == target {
                    return Vec::new();
                }
                let mut out: Vec<$t> = vec![target];
                let mut delta = *value - target;
                loop {
                    delta /= (2.0 as $t);
                    // shrink 結果が入力と区別できなくなるほど delta が
                    // 小さくなったら停止する。
                    if delta.abs() < <$t>::EPSILON * value.abs().max(1.0) {
                        break;
                    }
                    let cand = *value - delta;
                    if cand >= self.lo
                        && cand < self.hi
                        && cand != *value
                        && !out.iter().any(|x: &$t| (*x - cand).abs() < <$t>::EPSILON)
                    {
                        out.push(cand);
                    }
                }
                out
            }
        }
    };
}

impl_float_range!(f32);
impl_float_range!(f64);

// --- one_of: strategy リストからの一様選択 ------------------

/// サブ strategy 間で一様にいずれかを選択する strategy です。
/// すべてのサブ strategy は同じ値型を生成する必要があります。
pub struct OneOf<S> {
    choices: Vec<S>,
}

/// [`OneOf`] strategy を構築します。
pub fn one_of<S: Strategy>(choices: Vec<S>) -> OneOf<S> {
    assert!(!choices.is_empty(), "one_of: choices must be non-empty");
    OneOf { choices }
}

impl<S: Strategy> Strategy for OneOf<S> {
    type Value = S::Value;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> S::Value {
        let idx = rng.gen_range_usize(0, self.choices.len());
        self.choices[idx].new_value(rng, size)
    }
    fn shrink_value(&self, value: &S::Value) -> Vec<S::Value> {
        // どの分岐が `value` を生成したかを記憶していないため、できる最善は
        // すべての分岐の shrink を収集することである。
        let mut out = Vec::new();
        for c in &self.choices {
            out.extend(c.shrink_value(value));
        }
        out
    }
}

// --- weighted_one_of: one_of に整数の重みを付けたもの -------------

/// 与えられた整数の重みでサブ strategy 間を選択する strategy です。
pub struct WeightedOneOf<S> {
    choices: Vec<(u32, S)>,
    total_weight: u64,
}

/// [`WeightedOneOf`] strategy を構築します。各ペアは `(weight, strategy)` です。
pub fn weighted_one_of<S: Strategy>(choices: Vec<(u32, S)>) -> WeightedOneOf<S> {
    assert!(
        !choices.is_empty(),
        "weighted_one_of: choices must be non-empty"
    );
    let total = choices.iter().map(|(w, _)| *w as u64).sum::<u64>();
    assert!(total > 0, "weighted_one_of: total weight must be > 0");
    WeightedOneOf {
        choices,
        total_weight: total,
    }
}

impl<S: Strategy> Strategy for WeightedOneOf<S> {
    type Value = S::Value;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> S::Value {
        let mut pick = rng.gen_range_u64(0, self.total_weight);
        for (w, s) in &self.choices {
            let w = *w as u64;
            if pick < w {
                return s.new_value(rng, size);
            }
            pick -= w;
        }
        unreachable!("weights sum to total_weight")
    }
    fn shrink_value(&self, value: &S::Value) -> Vec<S::Value> {
        let mut out = Vec::new();
        for (_, s) in &self.choices {
            out.extend(s.shrink_value(value));
        }
        out
    }
}

// --- vec_of: 要素 strategy の可変長 Vec -------------------

/// 長さが `len_range` に収まる `Vec<S::Value>` を生成する strategy です。
/// ベクタは長さを優先して shrink し、その後要素を shrink します。
/// 長さは少なくとも `min_len` を保ちます。
pub struct VecOf<S> {
    element: S,
    min_len: usize,
    max_len: usize,
}

/// [`VecOf`] strategy を構築します。
pub fn vec_of<S: Strategy>(element: S, len_range: Range<usize>) -> VecOf<S> {
    assert!(
        len_range.start <= len_range.end,
        "vec_of: len_range must be non-decreasing"
    );
    VecOf {
        element,
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

impl<S: Strategy> Strategy for VecOf<S> {
    type Value = Vec<S::Value>;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> Vec<S::Value> {
        let len = if self.max_len > self.min_len {
            rng.gen_range_usize(self.min_len, self.max_len)
        } else {
            self.min_len
        };
        (0..len)
            .map(|_| self.element.new_value(rng, size))
            .collect()
    }
    fn shrink_value(&self, value: &Vec<S::Value>) -> Vec<Vec<S::Value>> {
        let mut out = Vec::new();
        // min_len を尊重しつつ短い接頭辞を生成する。
        if value.len() > self.min_len {
            if self.min_len == 0 {
                out.push(Vec::new());
            }
            for i in 0..value.len() {
                if value.len() > self.min_len {
                    let mut v = value.clone();
                    v.remove(i);
                    out.push(v);
                }
            }
        }
        // 内部 strategy を使って各要素を shrink する。
        for i in 0..value.len() {
            for s in self.element.shrink_value(&value[i]) {
                let mut v = value.clone();
                v[i] = s;
                out.push(v);
            }
        }
        out
    }
}

// --- strategy のタプル -----------------------------------------------

/// 2 つの strategy を結合し、その直積を生成する strategy です。
pub struct Tuple2<A, B>(pub A, pub B);

impl<A: Strategy, B: Strategy> Strategy for Tuple2<A, B> {
    type Value = (A::Value, B::Value);
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> Self::Value {
        (self.0.new_value(rng, size), self.1.new_value(rng, size))
    }
    fn shrink_value(&self, value: &Self::Value) -> Vec<Self::Value> {
        let mut out = Vec::new();
        for sa in self.0.shrink_value(&value.0) {
            out.push((sa, value.1.clone()));
        }
        for sb in self.1.shrink_value(&value.1) {
            out.push((value.0.clone(), sb));
        }
        out
    }
}

/// [`Tuple2`] strategy を構築します。
pub fn tuple<A: Strategy, B: Strategy>(a: A, b: B) -> Tuple2<A, B> {
    Tuple2(a, b)
}

// --- Map コンビネータ ----------------------------------------------------

/// [`StrategyExt::map`] を参照してください。
pub struct Map<S, F, U> {
    inner: S,
    f: F,
    _phantom: PhantomData<fn() -> U>,
}

impl<S, F, U> Strategy for Map<S, F, U>
where
    S: Strategy,
    F: Fn(S::Value) -> U + Clone + 'static,
    U: Clone + Debug + 'static,
{
    type Value = U;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> U {
        (self.f)(self.inner.new_value(rng, size))
    }
    fn shrink_value(&self, _value: &U) -> Vec<U> {
        // f の逆関数がないため、利用可能な shrink はない。
        Vec::new()
    }
}

// --- FlatMap コンビネータ -----------------------------------------------

/// [`StrategyExt::flat_map`] を参照してください。
///
/// `FlatMap` は shrink できません。なぜなら、保持される値は `f` に渡された
/// 「外側」の値とのつながりを失っているからです。依存生成に対する shrink が
/// 必要であれば、shrink に必要な状態を保持するカスタムの [`Strategy`] を
/// 手動で定義してください。
pub struct FlatMap<S, F, S2> {
    inner: S,
    f: F,
    _phantom: PhantomData<fn() -> S2>,
}

impl<S, F, S2> Strategy for FlatMap<S, F, S2>
where
    S: Strategy,
    F: Fn(S::Value) -> S2 + 'static,
    S2: Strategy,
{
    type Value = S2::Value;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> S2::Value {
        let outer = self.inner.new_value(rng, size);
        let inner_strategy = (self.f)(outer);
        inner_strategy.new_value(rng, size)
    }
    fn shrink_value(&self, _value: &S2::Value) -> Vec<S2::Value> {
        Vec::new()
    }
}

// --- Filter コンビネータ -------------------------------------------------

/// [`StrategyExt::filter`] を参照してください。
pub struct Filter<S, F> {
    inner: S,
    pred: F,
    max_tries: usize,
}

impl<S, F> Filter<S, F> {
    /// 述語を満たす値を生成できないときに panic するまでに、フィルタが
    /// 何回再試行するかを調整します。
    pub fn max_tries(mut self, n: usize) -> Self {
        self.max_tries = n;
        self
    }
}

impl<S, F> Strategy for Filter<S, F>
where
    S: Strategy,
    F: Fn(&S::Value) -> bool + 'static,
{
    type Value = S::Value;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> S::Value {
        for _ in 0..self.max_tries {
            let v = self.inner.new_value(rng, size);
            if (self.pred)(&v) {
                return v;
            }
        }
        panic!(
            "Filter: predicate rejected every candidate after {} tries",
            self.max_tries
        );
    }
    fn shrink_value(&self, value: &S::Value) -> Vec<S::Value> {
        self.inner
            .shrink_value(value)
            .into_iter()
            .filter(|v| (self.pred)(v))
            .collect()
    }
}

// --- recursive: 深さ制限付き再帰 strategy --------------------

/// `leaf` の上に `inner` を `max_depth` 回スタックして再帰 strategy を
/// 構築します。[`crate::strategy::recursive`] および `propcheck` クレートの
/// `prop_recursive!` マクロで使用されます。
///
/// `inner` は各階層で、これまでに構築された strategy を引数に呼び出されます。
/// 最上位では、既に内部ケースと葉ケースを葉まで混在させた strategy を受け取ります。
pub fn recursive<L, I, R, T>(leaf: L, inner: I, max_depth: usize) -> BoxedStrategy<T>
where
    L: Strategy<Value = T> + 'static,
    R: Strategy<Value = T> + 'static,
    I: Fn(BoxedStrategy<T>) -> R,
    T: Clone + Debug + 'static,
{
    let mut current: BoxedStrategy<T> = leaf.boxed();
    for _ in 0..max_depth {
        current = inner(current).boxed();
    }
    current
}

// --- BoxedStrategy: 型消去 ---------------------------------------

/// ヒープ確保された、型消去された strategy です。[`one_of`] 用に異種の
/// strategy を 1 つの [`Vec`] にまとめるのに便利です。
pub struct BoxedStrategy<T: Clone + Debug + 'static> {
    inner: Box<dyn ErasedStrategy<Value = T>>,
}

impl<T: Clone + Debug + 'static> Strategy for BoxedStrategy<T> {
    type Value = T;
    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> T {
        // (サイズが不定かもしれない) `R` を sized なアダプタでラップし、
        // オブジェクト安全な呼び出しのために結果を `&mut dyn Rng` へ強制変換できるようにする。
        let mut adapter = SizedRngRef { inner: rng };
        self.inner.dyn_new_value(&mut adapter, size)
    }
    fn shrink_value(&self, value: &T) -> Vec<T> {
        self.inner.dyn_shrink_value(value)
    }
}

/// [`BoxedStrategy`] が使用する [`Strategy`] のオブジェクト安全な兄弟トレイトです。
/// メソッド名は意図的に `Strategy` のものと異なるようにしてあります。
/// これは、下のブランケット実装がユーザーコード内の通常の [`Strategy`] 呼び出しを
/// 隠蔽しないようにするためです。
trait ErasedStrategy {
    type Value: Clone + Debug + 'static;
    fn dyn_new_value(&self, rng: &mut dyn Rng, size: usize) -> Self::Value;
    fn dyn_shrink_value(&self, value: &Self::Value) -> Vec<Self::Value>;
}

impl<S: Strategy> ErasedStrategy for S {
    type Value = S::Value;
    fn dyn_new_value(&self, rng: &mut dyn Rng, size: usize) -> Self::Value {
        self.new_value(rng, size)
    }
    fn dyn_shrink_value(&self, value: &Self::Value) -> Vec<Self::Value> {
        Strategy::shrink_value(self, value)
    }
}

/// `R: ?Sized` の場合でも `&mut dyn Rng` に強制変換できるよう、`&mut R` を
/// 囲む sized なラッパーです。ラッパー自体は常にポインタサイズです。
struct SizedRngRef<'a, R: Rng + ?Sized> {
    inner: &'a mut R,
}

impl<R: Rng + ?Sized> Rng for SizedRngRef<'_, R> {
    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }
    fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }
    fn gen_bool(&mut self) -> bool {
        self.inner.gen_bool()
    }
    fn gen_range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        self.inner.gen_range_u64(lo, hi)
    }
    fn gen_range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        self.inner.gen_range_i64(lo, hi)
    }
    fn gen_range_usize(&mut self, lo: usize, hi: usize) -> usize {
        self.inner.gen_range_usize(lo, hi)
    }
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        self.inner.fill_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::XorShift64;

    fn r() -> XorShift64 {
        XorShift64::seed_from_u64(0xDEADBEEF)
    }

    #[test]
    fn int_range_stays_in_bounds() {
        let s = int_range(10i32..20);
        let mut rng = r();
        for _ in 0..1000 {
            let v = s.new_value(&mut rng, 50);
            assert!((10..20).contains(&v));
        }
    }

    #[test]
    fn int_range_shrinks_toward_zero_inside_range() {
        let s = int_range(-100i32..100);
        let shrinks = s.shrink_value(&50);
        assert!(shrinks.contains(&0));
        assert!(shrinks.iter().all(|&v| (-100..100).contains(&v)));
    }

    #[test]
    fn int_range_shrinks_toward_lo_outside_zero() {
        let s = int_range(10i32..20);
        let shrinks = s.shrink_value(&18);
        assert_eq!(shrinks[0], 10);
        assert!(shrinks.iter().all(|&v| (10..20).contains(&v)));
    }

    #[test]
    fn vec_of_respects_min_len() {
        let s = vec_of(int_range(0i32..10), 2..5);
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 10);
            assert!(v.len() >= 2 && v.len() < 5);
        }
        let shrinks = s.shrink_value(&vec![1, 2, 3, 4]);
        assert!(shrinks.iter().all(|v| v.len() >= 2));
    }

    #[test]
    fn one_of_picks_among_choices() {
        let s = one_of(vec![just(1i32), just(2), just(3)]);
        let mut rng = r();
        let values: Vec<i32> = (0..200).map(|_| s.new_value(&mut rng, 0)).collect();
        assert!(values.iter().all(|v| [1, 2, 3].contains(v)));
        // 3 つすべてが高い確率で出現するはず。
        assert!(values.contains(&1));
        assert!(values.contains(&2));
        assert!(values.contains(&3));
    }

    #[test]
    fn weighted_skews_distribution() {
        let s = weighted_one_of(vec![(99, just(0i32)), (1, just(1))]);
        let mut rng = r();
        let zeros = (0..1000).filter(|_| s.new_value(&mut rng, 0) == 0).count();
        assert!(zeros > 900, "expected ~99% zeros, got {zeros}");
    }

    #[test]
    fn filter_rejects_predicate_failures() {
        let s = any::<i32>().filter(|n| *n % 2 == 0);
        let mut rng = r();
        for _ in 0..100 {
            let v = s.new_value(&mut rng, 50);
            assert_eq!(v % 2, 0);
        }
    }

    #[test]
    fn map_transforms_values() {
        let s = int_range(0i32..10).map(|n| n * n);
        let mut rng = r();
        for _ in 0..100 {
            let v = s.new_value(&mut rng, 0);
            assert!((0..100).contains(&v));
            // 0..10 の何らかの完全平方数であるはず。
            let root = (v as f64).sqrt() as i32;
            assert!(root * root == v);
        }
    }

    #[test]
    fn boxed_strategy_allows_heterogeneous_one_of() {
        let s = one_of(vec![
            int_range(0i32..10).boxed(),
            just(100i32).boxed(),
            any::<i32>().filter(|n| *n < -1000).boxed(),
        ]);
        let mut rng = r();
        // API を動作確認する。値は 3 つの互いに素な領域のいずれかに入りうる。
        for _ in 0..50 {
            let _ = s.new_value(&mut rng, 50);
        }
    }

    #[test]
    fn tuple_combines_two_strategies() {
        let s = tuple(int_range(0i32..10), int_range(100i32..200));
        let mut rng = r();
        let (a, b) = s.new_value(&mut rng, 0);
        assert!((0..10).contains(&a));
        assert!((100..200).contains(&b));
    }
}
