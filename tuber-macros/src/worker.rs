use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::Parser;
use syn::token::{Colon, Underscore};
use syn::{FnArg, Ident, PatIdent, PatType, Type, TypeInfer};

use super::common::*;

struct FinalWorkerConfig {
    name: String,
    tube_pat: PatType,
}

struct WorkerConfig {
    name: Option<(String, Span)>,
    tube_pat: PatType,
}

impl WorkerConfig {
    fn new(tube_pat: PatType) -> Self {
        WorkerConfig {
            name: None,
            tube_pat,
        }
    }

    fn set_name(&mut self, name: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.name.is_some() {
            return Err(syn::Error::new(span, "`name` set multiple times."));
        }
        let name = parse_string(name, span, "name")?;
        self.name = Some((name, span));
        Ok(())
    }

    fn macro_name(&self) -> &'static str {
        "tuber::worker"
    }

    fn build(&self) -> Result<FinalWorkerConfig, syn::Error> {
        let name = match self.name.as_ref() {
            Some((name, _span)) => name.to_string(),
            None => {
                let msg = format!(
                    "A `name` is required to wire workers up to the main thread. Use `#[{}(name = \"rồi\")]`",
                    self.macro_name(),
                );
                return Err(syn::Error::new(Span::call_site(), msg));
            }
        };

        Ok(FinalWorkerConfig {
            name,
            tube_pat: self.tube_pat.clone(),
        })
    }
}

pub(crate) fn worker(args: TokenStream, item: TokenStream) -> TokenStream {
    let input: ItemFn = match syn::parse2(item.clone()) {
        Ok(it) => it,
        Err(e) => return token_stream_with_error(item, e),
    };

    let config = match input.sig.inputs.first() {
        Some(FnArg::Typed(pat)) => AttributeArgs::parse_terminated
            .parse2(args)
            .and_then(|args| build_worker_config(&input, args, pat.clone())),

        _ => {
            let msg = "tuber::worker entrypoint must accept a single `Tube<In, Out>` as argument";
            Err(syn::Error::new_spanned(&input.sig.ident, msg))
        }
    };

    match config {
        Ok(config) => parse_knobs(input, config),
        Err(e) => token_stream_with_error(
            parse_knobs(
                input,
                FinalWorkerConfig {
                    name: "rồi".to_string(),
                    tube_pat: PatType {
                        attrs: vec![],
                        pat: Box::new(syn::Pat::Ident(PatIdent {
                            attrs: vec![],
                            by_ref: None,
                            mutability: None,
                            ident: Ident::new("tube", Span::call_site()),
                            subpat: None,
                        })),
                        colon_token: Colon(Span::call_site()),
                        ty: Box::new(Type::Infer(TypeInfer {
                            underscore_token: Underscore(Span::call_site()),
                        })),
                    },
                },
            ),
            e,
        ),
    }
}

fn build_worker_config(
    input: &ItemFn,
    args: AttributeArgs,
    tube_pat: PatType,
) -> Result<FinalWorkerConfig, syn::Error> {
    if input.sig.asyncness.is_none() {
        let msg = "the `async` keyword is missing from the function declaration";
        return Err(syn::Error::new_spanned(input.sig.fn_token, msg));
    }

    let mut config = WorkerConfig::new(tube_pat);

    for arg in args {
        match arg {
            syn::Meta::NameValue(namevalue) => {
                let ident = namevalue
                    .path
                    .get_ident()
                    .ok_or_else(|| {
                        syn::Error::new_spanned(&namevalue, "Must have specified ident")
                    })?
                    .to_string()
                    .to_lowercase();
                let lit = match &namevalue.value {
                    syn::Expr::Lit(syn::ExprLit { lit, .. }) => lit,
                    expr => return Err(syn::Error::new_spanned(expr, "Must be a literal")),
                };
                match ident.as_str() {
                    "name" => {
                        config.set_name(lit.clone(), syn::spanned::Spanned::span(lit))?;
                    }
                    name => {
                        let msg = format!(
                            "Unknown attribute {} is specified; expected one of: `name`",
                            name,
                        );
                        return Err(syn::Error::new_spanned(namevalue, msg));
                    }
                }
            }
            syn::Meta::Path(path) => {
                let name = path
                    .get_ident()
                    .ok_or_else(|| syn::Error::new_spanned(&path, "Must have specified ident"))?
                    .to_string()
                    .to_lowercase();
                let msg = match name.as_str() {
                    "name" => {
                        format!("The `{}` attribute requires an argument.", name)
                    }
                    name => {
                        format!(
                            "Unknown attribute {} is specified; expected one of: `name`",
                            name
                        )
                    }
                };
                return Err(syn::Error::new_spanned(path, msg));
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "Unknown attribute inside the macro",
                ));
            }
        }
    }

    config.build()
}

fn parse_knobs(mut input: ItemFn, config: FinalWorkerConfig) -> TokenStream {
    input.sig.asyncness = None;

    // If type mismatch occurs, the current rustc points to the last statement.
    let (last_stmt_start_span, last_stmt_end_span) = {
        let mut last_stmt = input.stmts.last().cloned().unwrap_or_default().into_iter();

        // `Span` on stable Rust has a limitation that only points to the first
        // token, not the whole tokens. We can work around this limitation by
        // using the first/last span of the tokens like
        // `syn::Error::new_spanned` does.
        let start = last_stmt.next().map_or_else(Span::call_site, |t| t.span());
        let end = last_stmt.last().map_or(start, |t| t.span());
        (start, end)
    };

    let crate_path = Ident::new("tuber", last_stmt_start_span).into_token_stream();

    let builder = quote_spanned! {last_stmt_start_span=>
        #crate_path::worker::Builder
    };

    let name = config.name;
    let entrypoint = format!("__tuber_worker_entrypoint_{}", name);
    let tube_pat = config.tube_pat;

    let header = quote! {
        #[wasm_bindgen(js_name = #entrypoint)]
    };

    let body = input.body();
    let body_ident = quote! { body };
    let last_block = quote_spanned! {last_stmt_end_span=>
        #[allow(clippy::expect_used, clippy::diverging_sub_expression)]
        {
            let #tube_pat = #builder::with_name((#name).to_string())
                .build()
                .expect("Failed to build the worker");

            let body = async move #body;

            #crate_path::task::spawn_local(#body_ident).expect("Failed to spawn worker body");
        }
    };

    input.sig.inputs.clear();
    input.into_tokens(header, last_block)
}
