//! Hand-rolled proc-macros for propcheck.
//!
//! This crate depends only on the compiler-provided `proc_macro` crate — no
//! external dependencies such as `syn` or `quote`. The parser is small,
//! tuned for the inputs we actually accept (structs and free functions),
//! and intentionally returns clear errors rather than supporting every Rust
//! syntax detail.
//!
//! ## What's supported
//!
//! ### `#[derive(Arbitrary)]`
//! - Named-field structs:     `struct Foo { a: T, b: U }`
//! - Tuple structs:           `struct Foo(T, U);`
//! - Unit structs:            `struct Foo;`
//! - Generic structs:         `struct Foo<T> { x: T }`
//!   (an `Arbitrary` bound is automatically added to each type parameter)
//!
//! Enums and structs with custom where-clauses are **not** supported; write
//! the `Arbitrary` impl by hand for those.
//!
//! ### `#[propcheck]`
//! Wraps a free function as a `#[test]` driven by `propcheck::run`. Each
//! parameter type must implement `Arbitrary`. The function body is run for
//! each generated case; `prop_assert!`, `prop_assert_eq!`, `prop_assume!`
//! all work inside.

extern crate proc_macro;

use proc_macro::{Delimiter, TokenStream, TokenTree};

// ---------------------------------------------------------------------------
// #[derive(Arbitrary)]
// ---------------------------------------------------------------------------

/// Derives [`propcheck::Arbitrary`](https://docs.rs/propcheck) for a
/// struct or enum.
///
/// Optional per-field attribute:
/// `#[arbitrary(strategy = <expr>)]` — generate this field using the
/// given [`propcheck::Strategy`] instead of the field type's default
/// `Arbitrary` impl. The expression can be a Rust expression or a string
/// literal containing one (proptest-style).
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
    /// Tokens copied from the optional `where` clause, e.g. `T: Send`.
    /// Empty if the struct had no where clause.
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
    /// Field name. Empty string for unnamed (tuple) fields; access them
    /// by index in codegen.
    name: String,
    /// Field type, stringified as the user wrote it.
    ty: String,
    /// Optional `#[arbitrary(strategy = ...)]` expression. When set, the
    /// generated impl uses `Strategy::new_value` / `Strategy::shrink_value`
    /// instead of `Arbitrary::arbitrary` / `Arbitrary::shrink`.
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

/// Like [`skip_attrs_and_visibility`] but extracts the
/// `#[arbitrary(strategy = ...)]` attribute when present.
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
                    // else: discard (other attributes are irrelevant to us)
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

/// Parses `arbitrary ( strategy = <expr> )` and returns the expression
/// tokens as a Rust source string. Returns `None` for any other attribute
/// shape.
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
    // If the value is a string literal, strip surrounding quotes and
    // unescape the common cases. Otherwise emit the tokens as-is.
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
        // enum
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

/// Consumes an optional `where C1: T1, C2: T2` clause and returns its
/// tokens stringified (excluding the `where` keyword and any trailing
/// `{` / `;`).
fn parse_optional_where(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
) -> Result<String, String> {
    if !matches!(iter.peek(), Some(TokenTree::Ident(id)) if id.to_string() == "where") {
        return Ok(String::new());
    }
    iter.next(); // consume `where`
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
        // After variant name: optional `(...)`, `{...}`, or directly `,`/end.
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
        // Consume optional trailing comma.
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
                             // Expect a bracket group: `[...]`.
                if let Some(TokenTree::Group(_)) = iter.peek() {
                    iter.next();
                }
            }
            Some(TokenTree::Ident(id)) if id.to_string() == "pub" => {
                iter.next();
                // Possible visibility scope: `(crate)`, `(super)`, `(in ...)`.
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

/// Parses `< ... >` if present, returning (decl_string, use_string, type_param_names).
fn parse_generics(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
) -> Result<(String, String, Vec<String>), String> {
    let opens_with_angle = matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '<');
    if !opens_with_angle {
        return Ok((String::new(), String::new(), Vec::new()));
    }
    iter.next(); // consume '<'

    let mut decl = String::new();
    let mut depth = 1i32; // we are inside <...>
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

    // Build decl, use list, and extract type-param names.
    let mut use_list: Vec<String> = Vec::new();
    let mut type_params: Vec<String> = Vec::new();
    for (i, param_tokens) in params.iter().enumerate() {
        if i > 0 {
            decl.push_str(", ");
        }
        // Decl: include all tokens as written (preserving bounds).
        let decl_str = stream_to_string(param_tokens.iter().cloned().collect());
        decl.push_str(&decl_str);

        // Use list and type-param extraction.
        // Cases:
        //   `T`            -> type param "T", use "T"
        //   `T: Bound`     -> type param "T", use "T"
        //   `'a`           -> lifetime "'a", use "'a"
        //   `'a: 'b`       -> lifetime "'a",  use "'a"
        //   `const N: T`   -> const generic, use "N"
        let first = &param_tokens[0];
        match first {
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                // Lifetime: next ident is its name.
                if let Some(TokenTree::Ident(name)) = param_tokens.get(1) {
                    use_list.push(format!("'{}", name));
                }
            }
            TokenTree::Ident(id) if id.to_string() == "const" => {
                // `const N: T` — use "N".
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

/// Returns the Rust expression that generates a value for a single field
/// during `Arbitrary::arbitrary`. Uses the field's `#[arbitrary(strategy = ...)]`
/// expression when present, otherwise the field type's default `Arbitrary`.
fn gen_field_value(strategy: &Option<String>) -> String {
    match strategy {
        Some(expr) => format!(
            "{{ let __strat = ({expr}); ::propcheck::Strategy::new_value(&__strat, rng, size) }}"
        ),
        None => "<_ as ::propcheck::Arbitrary>::arbitrary(rng, size)".to_string(),
    }
}

/// Returns the Rust expression that yields an iterator over shrink
/// candidates for a single field. `field_access` is the source for the
/// current value (e.g. `self.foo` or `__f0`).
fn gen_field_shrink_iter(strategy: &Option<String>, field_access: &str) -> String {
    match strategy {
        Some(expr) => format!(
            "{{ let __strat = ({expr}); ::propcheck::Strategy::shrink_value(&__strat, &{field_access}) }}"
        ),
        None => format!("::propcheck::Arbitrary::shrink(&{field_access})"),
    }
}

/// Collects tokens up to (but not including) the next top-level `,`, respecting
/// nesting of `<...>` and any delimited groups.
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
    // Build the where clause: each type param needs Arbitrary, plus the
    // user's own where tokens if any.
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

// --- enum codegen ------------------------------------------------------

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

    // Find the simplest variant (preferring unit, then smallest arity).
    // Used as the "collapse" target during shrinking.
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

    // --- arbitrary(): pick a variant uniformly, fill its fields ---
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

    // --- shrink(): per-variant field shrinks + optional collapse ---
    let mut arms_shrink = String::new();
    for v in e.variants.iter() {
        let vname = &v.name;
        let (pat, body) = match &v.fields {
            Fields::Unit => (
                format!("{name}::{vname}"),
                String::from("/* no shrinks for a unit variant */"),
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
                    // The match binding is already a reference, so the
                    // strategy path takes the binding directly (no extra `&`).
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
                    // Same `&&` consideration as above for the enum named
                    // case: the field binding is already a reference.
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

    // Collapse to simplest variant (only if it is a unit variant and the
    // current is not already that variant).
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

/// Wraps a free function as a property-based test driven by `propcheck::run`.
///
/// Accepts optional `key = literal` arguments:
/// - `cases = N`         — total passing cases (default 100)
/// - `seed = N`          — fixed PRNG seed
/// - `max_shrinks = N`   — shrink-step budget
/// - `max_size = N`      — generator size cap
/// - `max_discards = N`  — discard budget before abort
/// - `max_skips = N`     — skip budget before abort
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
    /// Optional return type as written by the user (e.g. `Result<(), MyErr>`).
    /// Empty string means no return type was declared.
    return_type: String,
    body: String,
    /// `true` if the function was declared `async fn`.
    is_async: bool,
}

fn parse_fn(input: TokenStream) -> Result<ParsedFn, String> {
    let mut iter = input.into_iter().peekable();
    skip_attrs_and_visibility(&mut iter);
    let mut is_async = false;
    // Skip / record modifiers before `fn`.
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

    // Optional return type: capture verbatim if present.
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
    // If the user declared a return type, preserve it on the inner closure
    // so the `?` operator and other Result-returning bodies type-infer
    // unambiguously.
    let body_ret = if f.return_type.is_empty() {
        String::new()
    } else {
        format!(" -> {}", f.return_type)
    };

    // For async fn, wrap the body in `async move { ... }` and drive it
    // through propcheck's minimal block_on. Async block return types are
    // inferred — if the user needs a Result<(), E> body, the type comes
    // from the trailing `Ok(())` and any `?` propagations.
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

    // Choose entry point: `run` for default config, `run_with` when any
    // attribute argument is set.
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
// helpers
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
