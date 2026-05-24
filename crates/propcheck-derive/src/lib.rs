//! propcheck 向けに手書きされた proc-macro 群です。
//!
//! このクレートはコンパイラ提供の `proc_macro` クレートのみに依存しており、`syn`
//! や `quote` といった外部依存は持ちません。パーサーは小さく、実際に受け入れる
//! 入力（struct と自由関数）に合わせて調整されており、Rust 構文のあらゆる詳細を
//! サポートするのではなく、意図的に明確なエラーを返すようになっています。
//!
//! ## サポートしている内容
//!
//! ### `#[derive(Arbitrary)]`
//! - 名前付きフィールドの struct: `struct Foo { a: T, b: U }`
//! - タプル struct:               `struct Foo(T, U);`
//! - unit struct:                 `struct Foo;`
//! - ジェネリック struct:         `struct Foo<T> { x: T }`
//!   （各型パラメータに `Arbitrary` 境界が自動的に追加されます）
//!
//! enum や独自の where 句を持つ struct は **サポートしていません**。それらに
//! ついては `Arbitrary` 実装を手書きしてください。
//!
//! ### `#[propcheck]`
//! 自由関数を `propcheck::run` で駆動される `#[test]` としてラップします。
//! 各パラメータの型は `Arbitrary` を実装している必要があります。関数本体は
//! 生成されたケースごとに実行され、内部では `prop_assert!`、`prop_assert_eq!`、
//! `prop_assume!` がすべて利用できます。

extern crate proc_macro;

use proc_macro::{Delimiter, TokenStream, TokenTree};

// ---------------------------------------------------------------------------
// #[derive(Arbitrary)]
// ---------------------------------------------------------------------------

/// struct または enum に対して [`propcheck::Arbitrary`](https://docs.rs/propcheck)
/// を導出します。
///
/// オプションのフィールド単位の属性として、
/// `#[arbitrary(strategy = <expr>)]` を指定できます。これにより、フィールド型の
/// デフォルトの `Arbitrary` 実装の代わりに、指定した [`propcheck::Strategy`] を
/// 用いてそのフィールドを生成します。式は Rust の式そのもの、または式を含む
/// 文字列リテラル（proptest 風）のいずれかを指定できます。
#[proc_macro_derive(Arbitrary, attributes(arbitrary))]
pub fn derive_arbitrary(input: TokenStream) -> TokenStream {
    match parse_input(input) {
        Ok(ParsedItem::Struct(s)) => generate_arbitrary_impl(&s),
        Ok(ParsedItem::Enum(e)) => generate_arbitrary_impl_enum(&e),
        Err(e) => compile_error(&e),
    }
}

#[derive(Debug)]
enum ParsedItem {
    Struct(ParsedStruct),
    Enum(ParsedEnum),
}

#[derive(Debug)]
struct ParsedStruct {
    name: String,
    generics_decl: String,
    generics_use: String,
    type_params: Vec<String>,
    /// 任意指定の `where` 句からコピーされたトークン群です（例: `T: Send`）。
    /// struct に where 句がなかった場合は空となります。
    where_extra: String,
    fields: Fields,
}

#[derive(Debug)]
struct ParsedEnum {
    name: String,
    generics_decl: String,
    generics_use: String,
    type_params: Vec<String>,
    where_extra: String,
    variants: Vec<Variant>,
}

#[derive(Debug)]
struct Variant {
    name: String,
    fields: Fields,
}

#[derive(Debug)]
struct FieldInfo {
    /// フィールド名。名前なし（タプル）フィールドの場合は空文字列となり、
    /// コード生成ではインデックス経由でアクセスします。
    name: String,
    /// フィールドの型を、ユーザーが記述したとおりに文字列化したものです。
    ty: String,
    /// 任意指定の `#[arbitrary(strategy = ...)]` 式です。設定されている場合、
    /// 生成される実装は `Arbitrary::arbitrary` / `Arbitrary::shrink` ではなく
    /// `Strategy::new_value` / `Strategy::shrink_value` を使用します。
    strategy: Option<String>,
}

#[derive(Debug)]
enum Fields {
    Named(Vec<FieldInfo>),
    Unnamed(Vec<FieldInfo>),
    Unit,
}

#[derive(Default)]
struct FieldAttrs {
    strategy: Option<String>,
}

/// [`skip_attrs_and_visibility`] と同様ですが、`#[arbitrary(strategy = ...)]`
/// 属性が存在する場合はそれを抽出します。
fn collect_field_attrs(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
) -> FieldAttrs {
    let mut attrs = FieldAttrs::default();
    loop {
        match iter.peek() {
            Some(TokenTree::Punct(p)) if p.as_char() == '#' => {
                iter.next();
                if let Some(TokenTree::Group(g)) = iter.peek() {
                    let g_stream = g.stream();
                    iter.next();
                    if let Some(strat) = try_parse_arbitrary_attr(g_stream) {
                        attrs.strategy = Some(strat);
                    }
                    // それ以外の場合は破棄します（他の属性はここでは関係ありません）。
                }
            }
            Some(TokenTree::Ident(id)) if id.to_string() == "pub" => {
                iter.next();
                if let Some(TokenTree::Group(g)) = iter.peek() {
                    if g.delimiter() == Delimiter::Parenthesis {
                        iter.next();
                    }
                }
            }
            _ => break,
        }
    }
    attrs
}

/// `arbitrary ( strategy = <expr> )` をパースし、その式のトークンを Rust の
/// ソース文字列として返します。それ以外の形式の属性については `None` を
/// 返します。
fn try_parse_arbitrary_attr(stream: TokenStream) -> Option<String> {
    let mut it = stream.into_iter();
    match it.next()? {
        TokenTree::Ident(id) if id.to_string() == "arbitrary" => {}
        _ => return None,
    }
    let group = match it.next()? {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => g,
        _ => return None,
    };
    let mut inner = group.stream().into_iter();
    match inner.next()? {
        TokenTree::Ident(id) if id.to_string() == "strategy" => {}
        _ => return None,
    }
    match inner.next()? {
        TokenTree::Punct(p) if p.as_char() == '=' => {}
        _ => return None,
    }
    let rest: TokenStream = inner.collect();
    let s = rest.to_string();
    let trimmed = s.trim();
    // 値が文字列リテラルの場合は、囲み引用符を取り除き、よくあるケースのみ
    // アンエスケープします。それ以外の場合はトークンをそのまま出力します。
    let unquoted = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_string()
    };
    Some(unquoted)
}

fn parse_input(input: TokenStream) -> Result<ParsedItem, String> {
    let mut iter = input.into_iter().peekable();
    skip_attrs_and_visibility(&mut iter);

    let kind = match iter.next() {
        Some(TokenTree::Ident(id)) => id.to_string(),
        Some(other) => {
            return Err(format!(
                "expected `struct` or `enum`, found `{}`",
                tt_display(&other)
            ))
        }
        None => return Err("expected `struct` or `enum`".to_string()),
    };
    if kind != "struct" && kind != "enum" {
        return Err(format!(
            "#[derive(Arbitrary)] only supports `struct` and `enum`, got `{kind}`"
        ));
    }
    let name = match iter.next() {
        Some(TokenTree::Ident(id)) => id.to_string(),
        Some(other) => {
            return Err(format!(
                "expected identifier, found `{}`",
                tt_display(&other)
            ))
        }
        None => return Err("expected type name".to_string()),
    };
    let (generics_decl, generics_use, type_params) = parse_generics(&mut iter)?;
    let where_extra = parse_optional_where(&mut iter)?;

    if kind == "struct" {
        let fields = match iter.next() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                Fields::Named(parse_named_fields(g.stream())?)
            }
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                Fields::Unnamed(parse_tuple_fields(g.stream())?)
            }
            Some(TokenTree::Punct(p)) if p.as_char() == ';' => Fields::Unit,
            Some(other) => {
                return Err(format!(
                    "expected struct body or `;`, found `{}`",
                    tt_display(&other)
                ))
            }
            None => return Err("expected struct body".to_string()),
        };
        Ok(ParsedItem::Struct(ParsedStruct {
            name,
            generics_decl,
            generics_use,
            type_params,
            where_extra,
            fields,
        }))
    } else {
        // enum の場合
        let body = match iter.next() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g.stream(),
            Some(other) => {
                return Err(format!(
                    "expected enum body `{{ ... }}`, found `{}`",
                    tt_display(&other)
                ))
            }
            None => return Err("expected enum body".to_string()),
        };
        let variants = parse_enum_variants(body)?;
        if variants.is_empty() {
            return Err("#[derive(Arbitrary)] needs at least one variant on an enum".to_string());
        }
        Ok(ParsedItem::Enum(ParsedEnum {
            name,
            generics_decl,
            generics_use,
            type_params,
            where_extra,
            variants,
        }))
    }
}

/// 任意指定の `where C1: T1, C2: T2` 句を消費し、そのトークンを文字列化して
/// 返します（`where` キーワードや末尾の `{` / `;` は含みません）。
fn parse_optional_where(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
) -> Result<String, String> {
    if !matches!(iter.peek(), Some(TokenTree::Ident(id)) if id.to_string() == "where") {
        return Ok(String::new());
    }
    iter.next(); // `where` を消費します
    let mut tokens: Vec<TokenTree> = Vec::new();
    while let Some(t) = iter.peek() {
        match t {
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Brace || g.delimiter() == Delimiter::Parenthesis =>
            {
                break;
            }
            TokenTree::Punct(p) if p.as_char() == ';' => break,
            _ => tokens.push(iter.next().unwrap()),
        }
    }
    Ok(stream_to_string(tokens.into_iter().collect()))
}

fn parse_enum_variants(stream: TokenStream) -> Result<Vec<Variant>, String> {
    let mut iter = stream.into_iter().peekable();
    let mut out: Vec<Variant> = Vec::new();
    while iter.peek().is_some() {
        skip_attrs_and_visibility(&mut iter);
        let name = match iter.peek() {
            Some(TokenTree::Ident(_)) => match iter.next().unwrap() {
                TokenTree::Ident(id) => id.to_string(),
                _ => unreachable!(),
            },
            None => break,
            Some(other) => {
                return Err(format!(
                    "expected variant name, found `{}`",
                    tt_display(other)
                ))
            }
        };
        // バリアント名のあとには、任意の `(...)`、`{...}`、あるいはそのまま `,` または末尾が続きます。
        let fields = match iter.peek() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                let g = match iter.next().unwrap() {
                    TokenTree::Group(g) => g,
                    _ => unreachable!(),
                };
                Fields::Unnamed(parse_tuple_fields(g.stream())?)
            }
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                let g = match iter.next().unwrap() {
                    TokenTree::Group(g) => g,
                    _ => unreachable!(),
                };
                Fields::Named(parse_named_fields(g.stream())?)
            }
            _ => Fields::Unit,
        };
        out.push(Variant { name, fields });
        // 末尾の任意のカンマを消費します。
        if matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
            iter.next();
        }
    }
    Ok(out)
}

fn skip_attrs_and_visibility(iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>) {
    loop {
        match iter.peek() {
            Some(TokenTree::Punct(p)) if p.as_char() == '#' => {
                iter.next(); // '#'
                             // 続くのは角括弧グループ `[...]` のはずです。
                if let Some(TokenTree::Group(_)) = iter.peek() {
                    iter.next();
                }
            }
            Some(TokenTree::Ident(id)) if id.to_string() == "pub" => {
                iter.next();
                // 可視性スコープが続く可能性があります: `(crate)`、`(super)`、`(in ...)`。
                if let Some(TokenTree::Group(g)) = iter.peek() {
                    if g.delimiter() == Delimiter::Parenthesis {
                        iter.next();
                    }
                }
            }
            _ => break,
        }
    }
}

/// `< ... >` が存在すればパースし、(decl_string, use_string, type_param_names) を返します。
fn parse_generics(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
) -> Result<(String, String, Vec<String>), String> {
    let opens_with_angle = matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '<');
    if !opens_with_angle {
        return Ok((String::new(), String::new(), Vec::new()));
    }
    iter.next(); // '<' を消費します

    let mut decl = String::new();
    let mut depth = 1i32; // <...> の内側に入った状態です
    let mut current_param_tokens: Vec<TokenTree> = Vec::new();
    let mut params: Vec<Vec<TokenTree>> = Vec::new();

    loop {
        match iter.next() {
            None => return Err("unterminated generics `<...>`".to_string()),
            Some(TokenTree::Punct(p)) if p.as_char() == '<' => {
                depth += 1;
                current_param_tokens.push(TokenTree::Punct(p));
            }
            Some(TokenTree::Punct(p)) if p.as_char() == '>' => {
                depth -= 1;
                if depth == 0 {
                    if !current_param_tokens.is_empty() {
                        params.push(std::mem::take(&mut current_param_tokens));
                    }
                    break;
                } else {
                    current_param_tokens.push(TokenTree::Punct(p));
                }
            }
            Some(TokenTree::Punct(p)) if p.as_char() == ',' && depth == 1 => {
                if !current_param_tokens.is_empty() {
                    params.push(std::mem::take(&mut current_param_tokens));
                }
            }
            Some(t) => current_param_tokens.push(t),
        }
    }

    // 宣言部、使用リスト、型パラメータ名を構築します。
    let mut use_list: Vec<String> = Vec::new();
    let mut type_params: Vec<String> = Vec::new();
    for (i, param_tokens) in params.iter().enumerate() {
        if i > 0 {
            decl.push_str(", ");
        }
        // 宣言: 記述されたとおりに全トークンを含めます（境界を保持します）。
        let decl_str = stream_to_string(param_tokens.iter().cloned().collect());
        decl.push_str(&decl_str);

        // 使用リストおよび型パラメータ名の抽出。
        // ケース:
        //   `T`            -> 型パラメータ "T"、use は "T"
        //   `T: Bound`     -> 型パラメータ "T"、use は "T"
        //   `'a`           -> ライフタイム "'a"、use は "'a"
        //   `'a: 'b`       -> ライフタイム "'a"、use は "'a"
        //   `const N: T`   -> const ジェネリクス、use は "N"
        let first = &param_tokens[0];
        match first {
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                // ライフタイム: 次の識別子がその名前です。
                if let Some(TokenTree::Ident(name)) = param_tokens.get(1) {
                    use_list.push(format!("'{}", name));
                }
            }
            TokenTree::Ident(id) if id.to_string() == "const" => {
                // `const N: T` — use は "N" となります。
                if let Some(TokenTree::Ident(name)) = param_tokens.get(1) {
                    use_list.push(name.to_string());
                }
            }
            TokenTree::Ident(id) => {
                let name = id.to_string();
                use_list.push(name.clone());
                type_params.push(name);
            }
            _ => {}
        }
    }

    let use_string = use_list.join(", ");
    Ok((decl, use_string, type_params))
}

fn parse_named_fields(stream: TokenStream) -> Result<Vec<FieldInfo>, String> {
    let mut iter = stream.into_iter().peekable();
    let mut out: Vec<FieldInfo> = Vec::new();

    while iter.peek().is_some() {
        let attrs = collect_field_attrs(&mut iter);
        let name = match iter.next() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            None => break,
            Some(other) => {
                return Err(format!(
                    "expected field name, found `{}`",
                    tt_display(&other)
                ))
            }
        };
        match iter.next() {
            Some(TokenTree::Punct(p)) if p.as_char() == ':' => {}
            Some(other) => {
                return Err(format!(
                    "expected `:` after field name, found `{}`",
                    tt_display(&other)
                ))
            }
            None => return Err("expected `:` after field name".to_string()),
        }
        let type_tokens = collect_until_top_comma(&mut iter);
        let ty = stream_to_string(type_tokens.into_iter().collect());
        out.push(FieldInfo {
            name,
            ty,
            strategy: attrs.strategy,
        });
    }
    Ok(out)
}

fn parse_tuple_fields(stream: TokenStream) -> Result<Vec<FieldInfo>, String> {
    let mut iter = stream.into_iter().peekable();
    let mut out: Vec<FieldInfo> = Vec::new();
    while iter.peek().is_some() {
        let attrs = collect_field_attrs(&mut iter);
        if iter.peek().is_none() {
            break;
        }
        let type_tokens = collect_until_top_comma(&mut iter);
        if type_tokens.is_empty() {
            break;
        }
        let ty = stream_to_string(type_tokens.into_iter().collect());
        out.push(FieldInfo {
            name: String::new(),
            ty,
            strategy: attrs.strategy,
        });
    }
    Ok(out)
}

/// `Arbitrary::arbitrary` の際に単一フィールドの値を生成する Rust の式を
/// 返します。フィールドに `#[arbitrary(strategy = ...)]` 式があればそれを
/// 使用し、なければフィールド型のデフォルトの `Arbitrary` を使用します。
fn gen_field_value(strategy: &Option<String>) -> String {
    match strategy {
        Some(expr) => format!(
            "{{ let __strat = ({expr}); ::propcheck::Strategy::new_value(&__strat, rng, size) }}"
        ),
        None => "<_ as ::propcheck::Arbitrary>::arbitrary(rng, size)".to_string(),
    }
}

/// 単一フィールドの shrink 候補を列挙するイテレータを生成する Rust の式を
/// 返します。`field_access` は現在の値の取得元です（例: `self.foo` や
/// `__f0`）。
fn gen_field_shrink_iter(strategy: &Option<String>, field_access: &str) -> String {
    match strategy {
        Some(expr) => format!(
            "{{ let __strat = ({expr}); ::propcheck::Strategy::shrink_value(&__strat, &{field_access}) }}"
        ),
        None => format!("::propcheck::Arbitrary::shrink(&{field_access})"),
    }
}

/// 次のトップレベルの `,` までのトークン（カンマ自体は含みません）を集めます。
/// `<...>` のネストや、その他の区切りグループのネストを考慮します。
fn collect_until_top_comma(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
) -> Vec<TokenTree> {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut depth = 0i32;
    while let Some(t) = iter.peek() {
        match t {
            TokenTree::Punct(p) if p.as_char() == ',' && depth == 0 => {
                iter.next();
                break;
            }
            TokenTree::Punct(p) if p.as_char() == '<' => {
                depth += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                depth -= 1;
            }
            _ => {}
        }
        out.push(iter.next().unwrap());
    }
    out
}

fn generate_arbitrary_impl(s: &ParsedStruct) -> TokenStream {
    // where 句を構築します。各型パラメータには Arbitrary 境界が必要で、
    // ユーザー自身の where トークンがあればそれも追加します。
    let mut where_pieces: Vec<String> = s
        .type_params
        .iter()
        .map(|p| format!("{p}: ::propcheck::Arbitrary"))
        .collect();
    if !s.where_extra.is_empty() {
        where_pieces.push(s.where_extra.clone());
    }
    let where_clause = if where_pieces.is_empty() {
        String::new()
    } else {
        format!("where {}", where_pieces.join(", "))
    };

    let generics_decl = if s.generics_decl.is_empty() {
        String::new()
    } else {
        format!("<{}>", s.generics_decl)
    };
    let generics_use = if s.generics_use.is_empty() {
        String::new()
    } else {
        format!("<{}>", s.generics_use)
    };

    let name = &s.name;
    let self_ty = format!("{name}{generics_use}");

    let (constructor_arb, shrink_body) = match &s.fields {
        Fields::Unit => (
            name.clone(),
            "::std::boxed::Box::new(::std::iter::empty())".to_string(),
        ),
        Fields::Named(fields) => {
            let mut init = String::from("{");
            for (i, fi) in fields.iter().enumerate() {
                if i > 0 {
                    init.push_str(", ");
                }
                init.push_str(&format!(
                    "{fname}: {gen}",
                    fname = fi.name,
                    gen = gen_field_value(&fi.strategy)
                ));
            }
            init.push('}');
            let constructor = format!("{name} {init}");

            let mut shrink = String::new();
            shrink.push_str("let mut __out: ::std::vec::Vec<Self> = ::std::vec::Vec::new();\n");
            for (idx, fi) in fields.iter().enumerate() {
                let other_clones: Vec<String> = fields
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != idx)
                    .map(|(_, fo)| {
                        format!("{}: ::std::clone::Clone::clone(&self.{})", fo.name, fo.name)
                    })
                    .collect();
                let other_part = if other_clones.is_empty() {
                    String::new()
                } else {
                    format!(", {}", other_clones.join(", "))
                };
                let field_access = format!("self.{}", fi.name);
                let shrink_iter = gen_field_shrink_iter(&fi.strategy, &field_access);
                shrink.push_str(&format!(
                    "for __s in {shrink_iter} {{\n  __out.push(Self {{ {fname}: __s{other_part} }});\n}}\n",
                    fname = fi.name
                ));
            }
            shrink.push_str("::std::boxed::Box::new(__out.into_iter())");
            (constructor, shrink)
        }
        Fields::Unnamed(fields) => {
            let mut init = String::from("(");
            for (i, fi) in fields.iter().enumerate() {
                if i > 0 {
                    init.push_str(", ");
                }
                init.push_str(&gen_field_value(&fi.strategy));
            }
            init.push(')');
            let constructor = format!("{name}{init}");

            let mut shrink = String::new();
            shrink.push_str("let mut __out: ::std::vec::Vec<Self> = ::std::vec::Vec::new();\n");
            for (idx, fi) in fields.iter().enumerate() {
                let mut args = String::new();
                for j in 0..fields.len() {
                    if j > 0 {
                        args.push_str(", ");
                    }
                    if j == idx {
                        args.push_str("__s");
                    } else {
                        args.push_str(&format!("::std::clone::Clone::clone(&self.{j})"));
                    }
                }
                let field_access = format!("self.{idx}");
                let shrink_iter = gen_field_shrink_iter(&fi.strategy, &field_access);
                shrink.push_str(&format!(
                    "for __s in {shrink_iter} {{\n  __out.push(Self({args}));\n}}\n"
                ));
            }
            shrink.push_str("::std::boxed::Box::new(__out.into_iter())");
            (constructor, shrink)
        }
    };

    let code = format!(
        "impl{generics_decl} ::propcheck::Arbitrary for {self_ty} {where_clause} {{
            fn arbitrary<__R: ::propcheck::Rng + ?Sized>(rng: &mut __R, size: usize) -> Self {{
                {constructor_arb}
            }}
            fn shrink(&self) -> ::std::boxed::Box<dyn ::std::iter::Iterator<Item = Self> + '_> {{
                {shrink_body}
            }}
        }}",
        generics_decl = generics_decl,
        self_ty = self_ty,
        where_clause = where_clause,
        constructor_arb = constructor_arb,
        shrink_body = shrink_body,
    );

    code.parse().unwrap_or_else(|e| {
        compile_error(&format!(
            "internal error: generated code failed to parse: {e}\n--- generated ---\n{code}"
        ))
    })
}

// --- enum のコード生成 ------------------------------------------------

fn generate_arbitrary_impl_enum(e: &ParsedEnum) -> TokenStream {
    let mut where_pieces: Vec<String> = e
        .type_params
        .iter()
        .map(|p| format!("{p}: ::propcheck::Arbitrary"))
        .collect();
    if !e.where_extra.is_empty() {
        where_pieces.push(e.where_extra.clone());
    }
    let where_clause = if where_pieces.is_empty() {
        String::new()
    } else {
        format!("where {}", where_pieces.join(", "))
    };

    let generics_decl = if e.generics_decl.is_empty() {
        String::new()
    } else {
        format!("<{}>", e.generics_decl)
    };
    let generics_use = if e.generics_use.is_empty() {
        String::new()
    } else {
        format!("<{}>", e.generics_use)
    };

    let name = &e.name;
    let self_ty = format!("{name}{generics_use}");
    let n_variants = e.variants.len();

    // 最もシンプルなバリアントを見つけます（unit を優先し、次にアリティの
    // 小さいものを選びます）。これは shrink 時の「collapse」対象として使用されます。
    let simplest_idx = e
        .variants
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| match &v.fields {
            Fields::Unit => 0usize,
            Fields::Named(fs) => 1 + fs.len() * 2,
            Fields::Unnamed(fs) => 1 + fs.len() * 2,
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    // --- arbitrary(): バリアントを一様に選び、そのフィールドを埋めます ---
    let mut arms_gen = String::new();
    for (i, v) in e.variants.iter().enumerate() {
        let vname = &v.name;
        let body = match &v.fields {
            Fields::Unit => format!("{name}::{vname}"),
            Fields::Unnamed(fs) => {
                let args: Vec<String> = fs.iter().map(|fi| gen_field_value(&fi.strategy)).collect();
                format!("{name}::{vname}({})", args.join(", "))
            }
            Fields::Named(fs) => {
                let inits: Vec<String> = fs
                    .iter()
                    .map(|fi| {
                        format!(
                            "{fname}: {gen}",
                            fname = fi.name,
                            gen = gen_field_value(&fi.strategy)
                        )
                    })
                    .collect();
                format!("{name}::{vname} {{ {} }}", inits.join(", "))
            }
        };
        arms_gen.push_str(&format!("            {i}u64 => {body},\n"));
    }

    // --- shrink(): バリアントごとのフィールド shrink と任意の collapse ---
    let mut arms_shrink = String::new();
    for v in e.variants.iter() {
        let vname = &v.name;
        let (pat, body) = match &v.fields {
            Fields::Unit => (
                format!("{name}::{vname}"),
                String::from("/* unit バリアントには shrink がありません */"),
            ),
            Fields::Unnamed(fs) => {
                let pat_args: Vec<String> = (0..fs.len()).map(|i| format!("__f{i}")).collect();
                let pat = format!("{name}::{vname}({})", pat_args.join(", "));
                let mut body = String::new();
                for (shrink_idx, fi) in fs.iter().enumerate() {
                    let mut ctor_args = String::new();
                    for j in 0..fs.len() {
                        if j > 0 {
                            ctor_args.push_str(", ");
                        }
                        if j == shrink_idx {
                            ctor_args.push_str("__s");
                        } else {
                            ctor_args.push_str(&format!("::std::clone::Clone::clone(__f{j})"));
                        }
                    }
                    let field_access = format!("__f{shrink_idx}");
                    // match による束縛はすでに参照になっているため、strategy
                    // パスではその束縛をそのまま渡します（追加の `&` は不要です）。
                    let shrink_iter = match &fi.strategy {
                        Some(expr) => format!(
                            "{{ let __strat = ({expr}); ::propcheck::Strategy::shrink_value(&__strat, {field_access}) }}"
                        ),
                        None => format!("::propcheck::Arbitrary::shrink({field_access})"),
                    };
                    body.push_str(&format!(
                        "for __s in {shrink_iter} {{\n            __out.push({name}::{vname}({ctor_args}));\n        }}\n"
                    ));
                }
                (pat, body)
            }
            Fields::Named(fs) => {
                let pat_args: Vec<String> = fs.iter().map(|fi| fi.name.clone()).collect();
                let pat = format!("{name}::{vname} {{ {} }}", pat_args.join(", "));
                let mut body = String::new();
                for (shrink_idx, fi) in fs.iter().enumerate() {
                    let sname = &fi.name;
                    let mut ctor_args = String::new();
                    for (j, fo) in fs.iter().enumerate() {
                        if j > 0 {
                            ctor_args.push_str(", ");
                        }
                        if j == shrink_idx {
                            ctor_args.push_str(&format!("{}: __s", fo.name));
                        } else {
                            ctor_args.push_str(&format!(
                                "{n}: ::std::clone::Clone::clone({n})",
                                n = fo.name
                            ));
                        }
                    }
                    // 上記と同様、enum の named バリアントについても `&&` に
                    // 関する考慮が必要です。フィールドの束縛はすでに参照です。
                    let shrink_iter = if let Some(expr) = &fi.strategy {
                        format!(
                            "{{ let __strat = ({expr}); ::propcheck::Strategy::shrink_value(&__strat, {sname}) }}"
                        )
                    } else {
                        format!("::propcheck::Arbitrary::shrink({sname})")
                    };
                    body.push_str(&format!(
                        "for __s in {shrink_iter} {{\n            __out.push({name}::{vname} {{ {ctor_args} }});\n        }}\n"
                    ));
                }
                (pat, body)
            }
        };
        arms_shrink.push_str(&format!(
            "        {pat} => {{\n            {body}        }}\n"
        ));
    }

    // 最もシンプルなバリアントへ collapse します（そのバリアントが unit
    // バリアントであり、かつ現在のバリアントがそれと異なる場合に限ります）。
    let collapse = if matches!(e.variants[simplest_idx].fields, Fields::Unit) {
        let target = &e.variants[simplest_idx].name;
        format!(
            "match self {{\n            {name}::{target} => {{}}\n            _ => __out.push({name}::{target}),\n        }}\n"
        )
    } else {
        String::new()
    };

    let code = format!(
        "impl{generics_decl} ::propcheck::Arbitrary for {self_ty} {where_clause} {{
            fn arbitrary<__R: ::propcheck::Rng + ?Sized>(rng: &mut __R, size: usize) -> Self {{
                let __pick = ::propcheck::Rng::gen_range_u64(rng, 0, {n_variants}u64);
                match __pick {{
{arms_gen}                    _ => unreachable!(),
                }}
            }}
            fn shrink(&self) -> ::std::boxed::Box<dyn ::std::iter::Iterator<Item = Self> + '_> {{
                let mut __out: ::std::vec::Vec<Self> = ::std::vec::Vec::new();
                {collapse}
                match self {{
{arms_shrink}                }}
                ::std::boxed::Box::new(__out.into_iter())
            }}
        }}",
    );

    code.parse().unwrap_or_else(|err| {
        compile_error(&format!(
            "internal error: generated enum code failed to parse: {err}\n--- generated ---\n{code}"
        ))
    })
}

// ---------------------------------------------------------------------------
// #[propcheck]
// ---------------------------------------------------------------------------

/// 自由関数を `propcheck::run` で駆動されるプロパティベースのテストとして
/// ラップします。
///
/// 任意指定の `key = literal` 引数を受け付けます:
/// - `cases = N`         — 成功させるケース総数（デフォルト 100）
/// - `seed = N`          — 固定の PRNG シード
/// - `max_shrinks = N`   — shrink ステップの上限
/// - `max_size = N`      — ジェネレータサイズの上限
/// - `max_discards = N`  — abort 前の discard 上限
/// - `max_skips = N`     — abort 前の skip 上限
#[proc_macro_attribute]
pub fn propcheck(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match parse_attr_args(attr) {
        Ok(a) => a,
        Err(e) => return compile_error(&e),
    };
    match parse_fn(item) {
        Ok(f) => generate_test_wrapper(&f, &args),
        Err(e) => compile_error(&e),
    }
}

#[derive(Debug, Default)]
struct AttrArgs {
    cases: Option<u64>,
    seed: Option<u64>,
    max_shrinks: Option<u64>,
    max_size: Option<u64>,
    max_discards: Option<u64>,
    max_skips: Option<u64>,
}

impl AttrArgs {
    fn any_set(&self) -> bool {
        self.cases.is_some()
            || self.seed.is_some()
            || self.max_shrinks.is_some()
            || self.max_size.is_some()
            || self.max_discards.is_some()
            || self.max_skips.is_some()
    }
}

fn parse_attr_args(attr: TokenStream) -> Result<AttrArgs, String> {
    let mut iter = attr.into_iter().peekable();
    let mut args = AttrArgs::default();
    while iter.peek().is_some() {
        let key = match iter.next() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            Some(t) => {
                return Err(format!(
                    "expected key identifier, found `{}`",
                    tt_display(&t)
                ))
            }
            None => break,
        };
        match iter.next() {
            Some(TokenTree::Punct(p)) if p.as_char() == '=' => {}
            other => {
                return Err(format!(
                    "expected `=` after `{key}`, found `{}`",
                    other.as_ref().map(tt_display).unwrap_or_default()
                ))
            }
        }
        let value_str = match iter.next() {
            Some(TokenTree::Literal(lit)) => lit.to_string(),
            Some(t) => {
                return Err(format!(
                    "expected integer literal for `{key}`, found `{}`",
                    tt_display(&t)
                ))
            }
            None => return Err(format!("expected value for `{key}`")),
        };
        let value: u64 = value_str
            .parse()
            .map_err(|e| format!("invalid integer literal for `{key}` ({value_str:?}): {e}"))?;
        match key.as_str() {
            "cases" => args.cases = Some(value),
            "seed" => args.seed = Some(value),
            "max_shrinks" => args.max_shrinks = Some(value),
            "max_size" => args.max_size = Some(value),
            "max_discards" => args.max_discards = Some(value),
            "max_skips" => args.max_skips = Some(value),
            other => return Err(format!("unknown #[propcheck] argument: `{other}`")),
        }
        if matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
            iter.next();
        }
    }
    Ok(args)
}

#[derive(Debug)]
struct ParsedFn {
    name: String,
    params: Vec<FieldInfo>,
    /// 任意指定の戻り値型を、ユーザーが記述したとおりに保持します
    /// （例: `Result<(), MyErr>`）。空文字列は戻り値型が宣言されていないことを意味します。
    return_type: String,
    body: String,
    /// 関数が `async fn` として宣言されていた場合に `true` となります。
    is_async: bool,
}

fn parse_fn(input: TokenStream) -> Result<ParsedFn, String> {
    let mut iter = input.into_iter().peekable();
    skip_attrs_and_visibility(&mut iter);
    let mut is_async = false;
    // `fn` の前にある修飾子をスキップ／記録します。
    while let Some(TokenTree::Ident(id)) = iter.peek() {
        let s = id.to_string();
        if s == "fn" {
            break;
        }
        if s == "async" {
            is_async = true;
        }
        iter.next();
        if s == "extern" {
            if let Some(TokenTree::Literal(_)) = iter.peek() {
                iter.next();
            }
        }
    }
    match iter.next() {
        Some(TokenTree::Ident(id)) if id.to_string() == "fn" => {}
        other => {
            return Err(format!(
                "expected `fn`, found `{}`",
                other.as_ref().map(tt_display).unwrap_or_default()
            ))
        }
    }
    let name = match iter.next() {
        Some(TokenTree::Ident(id)) => id.to_string(),
        _ => return Err("expected function name".to_string()),
    };
    if matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '<') {
        return Err("generic test functions are not supported by #[propcheck]".to_string());
    }
    let param_group = match iter.next() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => g,
        _ => return Err("expected `(...)` parameter list".to_string()),
    };
    let params = parse_named_fields(param_group.stream())?;

    // 任意指定の戻り値型: 存在する場合はそのまま取得します。
    let mut return_type = String::new();
    if matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '-') {
        iter.next();
        match iter.next() {
            Some(TokenTree::Punct(p)) if p.as_char() == '>' => {}
            _ => return Err("expected `->` in return type".to_string()),
        }
        let mut ret_tokens: Vec<TokenTree> = Vec::new();
        while let Some(t) = iter.peek() {
            if matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace) {
                break;
            }
            ret_tokens.push(iter.next().unwrap());
        }
        return_type = stream_to_string(ret_tokens.into_iter().collect());
    }
    let body_group = match iter.next() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g,
        _ => return Err("expected function body `{ ... }`".to_string()),
    };
    Ok(ParsedFn {
        name,
        params,
        return_type,
        body: body_group.stream().to_string(),
        is_async,
    })
}

fn generate_test_wrapper(f: &ParsedFn, args: &AttrArgs) -> TokenStream {
    let name = &f.name;

    let (arg_pattern, arg_type, destructure) = match f.params.len() {
        0 => ("__arg".to_string(), "()".to_string(), String::new()),
        1 => {
            let fi = &f.params[0];
            (
                "__arg".to_string(),
                fi.ty.clone(),
                format!("let {} = ::std::clone::Clone::clone(__arg);\n", fi.name),
            )
        }
        _ => {
            let names: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
            let types: Vec<&str> = f.params.iter().map(|p| p.ty.as_str()).collect();
            let pattern = format!("({})", names.join(", "));
            let ty = format!("({})", types.join(", "));
            (
                "__arg".to_string(),
                ty,
                format!("let {pattern} = ::std::clone::Clone::clone(__arg);\n"),
            )
        }
    };

    let body = &f.body;
    let name_str = format!("{name:?}");
    // ユーザーが戻り値型を宣言している場合は、内側のクロージャでもそれを
    // 保持します。これにより `?` 演算子や、Result を返すその他の本体について
    // 型推論が一意に決まるようになります。
    let body_ret = if f.return_type.is_empty() {
        String::new()
    } else {
        format!(" -> {}", f.return_type)
    };

    // async fn の場合、本体を `async move { ... }` でラップし、propcheck の
    // 最小限の block_on で駆動します。async ブロックの戻り値型は推論されます。
    // ユーザーが Result<(), E> な本体を必要とする場合、型は末尾の `Ok(())` や
    // `?` による伝播から決定されます。
    let inner_closure = if f.is_async {
        let out_ty = if f.return_type.is_empty() {
            "_".to_string()
        } else {
            f.return_type.clone()
        };
        format!(
            "|{arg_pattern}: &{arg_type}| {{
                {destructure}
                let __out: {out_ty} = ::propcheck::block_on(async move {{
                    {body}
                }});
                ::propcheck::IntoPropResult::into_prop_result(__out)
            }}"
        )
    } else {
        format!(
            "|{arg_pattern}: &{arg_type}| {{
                {destructure}
                let __body = || {body_ret} {{
                    {body}
                }};
                ::propcheck::IntoPropResult::into_prop_result(__body())
            }}"
        )
    };

    // エントリポイントを選択します。デフォルト設定では `run` を、属性引数が
    // いずれか設定されている場合は `run_with` を使用します。
    let invocation = if args.any_set() {
        let mut overrides = String::new();
        if let Some(v) = args.cases {
            overrides.push_str(&format!("            cases: {v}usize,\n"));
        }
        if let Some(v) = args.seed {
            overrides.push_str(&format!("            seed: {v}u64,\n"));
        }
        if let Some(v) = args.max_shrinks {
            overrides.push_str(&format!("            max_shrinks: {v}usize,\n"));
        }
        if let Some(v) = args.max_size {
            overrides.push_str(&format!("            max_size: {v}usize,\n"));
        }
        if let Some(v) = args.max_discards {
            overrides.push_str(&format!("            max_discards: {v}usize,\n"));
        }
        if let Some(v) = args.max_skips {
            overrides.push_str(&format!("            max_skips: {v}usize,\n"));
        }
        format!(
            "::propcheck::run_with::<{arg_type}, _, _>(
                {name_str},
                ::propcheck::Config {{
{overrides}                    ..::propcheck::Config::default()
                }},
                {inner_closure},
            )"
        )
    } else {
        format!(
            "::propcheck::run::<{arg_type}, _, _>(
                {name_str},
                {inner_closure},
            )"
        )
    };

    let code = format!(
        "#[test]
        fn {name}() {{
            {invocation};
        }}"
    );

    code.parse().unwrap_or_else(|e| {
        compile_error(&format!(
            "internal error: generated code failed to parse: {e}\n--- generated ---\n{code}"
        ))
    })
}

// ---------------------------------------------------------------------------
// ヘルパー
// ---------------------------------------------------------------------------

fn tt_display(tt: &TokenTree) -> String {
    tt.to_string()
}

fn stream_to_string(ts: TokenStream) -> String {
    ts.to_string()
}

fn compile_error(msg: &str) -> TokenStream {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    format!("::std::compile_error!(\"{escaped}\");")
        .parse()
        .unwrap()
}
