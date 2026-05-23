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

/// Derives [`propcheck::Arbitrary`](https://docs.rs/propcheck) for a struct.
#[proc_macro_derive(Arbitrary)]
pub fn derive_arbitrary(input: TokenStream) -> TokenStream {
    match parse_struct(input) {
        Ok(s) => generate_arbitrary_impl(&s),
        Err(e) => compile_error(&e),
    }
}

#[derive(Debug)]
struct ParsedStruct {
    name: String,
    /// Generic parameter declarations as they appear inside `<...>`, e.g.
    /// `T, U: Send`. Empty if the struct has no generics.
    generics_decl: String,
    /// Generic parameter usage list, e.g. `T, U`. Lifetimes appear as
    /// `'a`. Empty if the struct has no generics.
    generics_use: String,
    /// Names of type parameters (no lifetimes, no const). Used to synthesize
    /// the `T: Arbitrary` bounds.
    type_params: Vec<String>,
    fields: Fields,
}

#[derive(Debug)]
enum Fields {
    Named(Vec<(String, String)>),
    Unnamed(Vec<String>),
    Unit,
}

fn parse_struct(input: TokenStream) -> Result<ParsedStruct, String> {
    let mut iter = input.into_iter().peekable();

    // Skip attributes (`#[...]`) and visibility.
    skip_attrs_and_visibility(&mut iter);

    // Expect `struct`.
    match iter.next() {
        Some(TokenTree::Ident(id)) if id.to_string() == "struct" => {}
        Some(other) => {
            return Err(format!(
                "expected `struct`, found `{}`",
                tt_display(&other)
            ))
        }
        None => return Err("expected `struct`".to_string()),
    }

    // Struct name.
    let name = match iter.next() {
        Some(TokenTree::Ident(id)) => id.to_string(),
        Some(other) => return Err(format!("expected identifier, found `{}`", tt_display(&other))),
        None => return Err("expected struct name".to_string()),
    };

    // Optional generics: `<T, U: Bound>`.
    let (generics_decl, generics_use, type_params) = parse_generics(&mut iter)?;

    // Optional where clause: not supported — fall through to disallow.
    if let Some(TokenTree::Ident(id)) = iter.peek() {
        if id.to_string() == "where" {
            return Err("`where` clauses are not supported by #[derive(Arbitrary)]; write the impl by hand".to_string());
        }
    }

    // Body: braces, parens, or semicolon.
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

    Ok(ParsedStruct {
        name,
        generics_decl,
        generics_use,
        type_params,
        fields,
    })
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

fn parse_named_fields(stream: TokenStream) -> Result<Vec<(String, String)>, String> {
    let mut iter = stream.into_iter().peekable();
    let mut out: Vec<(String, String)> = Vec::new();

    while iter.peek().is_some() {
        skip_attrs_and_visibility(&mut iter);
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
        let type_str = stream_to_string(type_tokens.into_iter().collect());
        out.push((name, type_str));
    }
    Ok(out)
}

fn parse_tuple_fields(stream: TokenStream) -> Result<Vec<String>, String> {
    let mut iter = stream.into_iter().peekable();
    let mut out: Vec<String> = Vec::new();
    while iter.peek().is_some() {
        skip_attrs_and_visibility(&mut iter);
        if iter.peek().is_none() {
            break;
        }
        let type_tokens = collect_until_top_comma(&mut iter);
        if type_tokens.is_empty() {
            break;
        }
        let type_str = stream_to_string(type_tokens.into_iter().collect());
        out.push(type_str);
    }
    Ok(out)
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
    // Build extra bounds: each type param must be Arbitrary.
    let extra_bounds_pieces: Vec<String> = s
        .type_params
        .iter()
        .map(|p| format!("{p}: ::propcheck::Arbitrary"))
        .collect();
    let where_clause = if extra_bounds_pieces.is_empty() {
        String::new()
    } else {
        format!("where {}", extra_bounds_pieces.join(", "))
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
            for (i, (fname, _)) in fields.iter().enumerate() {
                if i > 0 {
                    init.push_str(", ");
                }
                init.push_str(&format!(
                    "{fname}: <_ as ::propcheck::Arbitrary>::arbitrary(rng, size)"
                ));
            }
            init.push('}');
            let constructor = format!("{name} {init}");

            let mut shrink = String::new();
            shrink.push_str("let mut __out: ::std::vec::Vec<Self> = ::std::vec::Vec::new();\n");
            for (idx, (fname, _)) in fields.iter().enumerate() {
                let other_clones: Vec<String> = fields
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != idx)
                    .map(|(_, (fother, _))| format!("{fother}: ::std::clone::Clone::clone(&self.{fother})"))
                    .collect();
                let other_clones_str = if other_clones.is_empty() {
                    String::new()
                } else {
                    format!(", {}", other_clones.join(", "))
                };
                shrink.push_str(&format!(
                    "for __s in ::propcheck::Arbitrary::shrink(&self.{fname}) {{\n  __out.push(Self {{ {fname}: __s{others} }});\n}}\n",
                    others = other_clones_str
                ));
            }
            shrink.push_str("::std::boxed::Box::new(__out.into_iter())");
            (constructor, shrink)
        }
        Fields::Unnamed(types) => {
            let mut init = String::from("(");
            for i in 0..types.len() {
                if i > 0 {
                    init.push_str(", ");
                }
                init.push_str("<_ as ::propcheck::Arbitrary>::arbitrary(rng, size)");
            }
            init.push(')');
            let constructor = format!("{name}{init}");

            let mut shrink = String::new();
            shrink.push_str("let mut __out: ::std::vec::Vec<Self> = ::std::vec::Vec::new();\n");
            for idx in 0..types.len() {
                let mut args = String::new();
                for j in 0..types.len() {
                    if j > 0 {
                        args.push_str(", ");
                    }
                    if j == idx {
                        args.push_str("__s");
                    } else {
                        args.push_str(&format!("::std::clone::Clone::clone(&self.{j})"));
                    }
                }
                shrink.push_str(&format!(
                    "for __s in ::propcheck::Arbitrary::shrink(&self.{idx}) {{\n  __out.push(Self({args}));\n}}\n"
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
}

impl AttrArgs {
    fn any_set(&self) -> bool {
        self.cases.is_some()
            || self.seed.is_some()
            || self.max_shrinks.is_some()
            || self.max_size.is_some()
            || self.max_discards.is_some()
    }
}

fn parse_attr_args(attr: TokenStream) -> Result<AttrArgs, String> {
    let mut iter = attr.into_iter().peekable();
    let mut args = AttrArgs::default();
    while iter.peek().is_some() {
        let key = match iter.next() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            Some(t) => return Err(format!("expected key identifier, found `{}`", tt_display(&t))),
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
            Some(t) => return Err(format!("expected integer literal for `{key}`, found `{}`", tt_display(&t))),
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
    params: Vec<(String, String)>,
    /// Optional return type as written by the user (e.g. `Result<(), MyErr>`).
    /// Empty string means no return type was declared.
    return_type: String,
    body: String,
}

fn parse_fn(input: TokenStream) -> Result<ParsedFn, String> {
    let mut iter = input.into_iter().peekable();
    skip_attrs_and_visibility(&mut iter);
    // Skip `async`, `unsafe`, `extern "C"` ... none of these make sense for a
    // property test, so just consume any modifier idents before `fn`.
    while let Some(TokenTree::Ident(id)) = iter.peek() {
        let s = id.to_string();
        if s == "fn" {
            break;
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
    })
}

fn generate_test_wrapper(f: &ParsedFn, args: &AttrArgs) -> TokenStream {
    let name = &f.name;

    let (arg_pattern, arg_type, destructure) = match f.params.len() {
        0 => (
            "__arg".to_string(),
            "()".to_string(),
            String::new(),
        ),
        1 => {
            let (pname, pty) = &f.params[0];
            (
                "__arg".to_string(),
                pty.clone(),
                format!("let {pname} = ::std::clone::Clone::clone(__arg);\n"),
            )
        }
        _ => {
            let names: Vec<&str> = f.params.iter().map(|(n, _)| n.as_str()).collect();
            let types: Vec<&str> = f.params.iter().map(|(_, t)| t.as_str()).collect();
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

    let inner_closure = format!(
        "|{arg_pattern}: &{arg_type}| {{
            {destructure}
            let __body = || {body_ret} {{
                {body}
            }};
            ::propcheck::IntoPropResult::into_prop_result(__body())
        }}"
    );

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
