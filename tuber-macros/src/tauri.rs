use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::Parser;
use syn::token::{Colon, Underscore};
use syn::{FnArg, Ident, PatIdent, PatType, Type, TypeInfer};

use super::common::*;

struct FinalWorkerConfig {
    tube_pat: PatType,
}

struct WorkerConfig {
    tube_pat: PatType,
}

impl WorkerConfig {
    fn new(tube_pat: PatType) -> Self {
        WorkerConfig { tube_pat }
    }

    fn build(&self) -> Result<FinalWorkerConfig, syn::Error> {
        Ok(FinalWorkerConfig {
            tube_pat: self.tube_pat.clone(),
        })
    }
}

pub(crate) fn tauri_web(args: TokenStream, item: TokenStream) -> TokenStream {
    let input: ItemFn = match syn::parse2(item.clone()) {
        Ok(it) => it,
        Err(e) => return token_stream_with_error(item, e),
    };

    let config = match input.sig.inputs.first() {
        Some(FnArg::Typed(pat)) => AttributeArgs::parse_terminated
            .parse2(args)
            .and_then(|args| build_worker_config(&input, args, pat.clone())),

        _ => {
            let msg =
                "tuber::tauri_web entrypoint must accept a single `Tube<In, Out>` as argument";
            Err(syn::Error::new_spanned(&input.sig.ident, msg))
        }
    };

    match config {
        Ok(config) => parse_knobs(input, config),
        Err(e) => token_stream_with_error(
            parse_knobs(
                input,
                FinalWorkerConfig {
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
    _input: &ItemFn,
    args: AttributeArgs,
    tube_pat: PatType,
) -> Result<FinalWorkerConfig, syn::Error> {
    let config = WorkerConfig::new(tube_pat);

    for arg in args {
        match arg {
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

    let tauri_path = quote_spanned! {last_stmt_start_span=>
        #crate_path::tauri
    };

    let tube_pat = config.tube_pat;

    let header = quote! {
        #[wasm_bindgen]
    };

    let body = input.body();
    let last_block = quote_spanned! {last_stmt_end_span=>
        #[allow(clippy::expect_used, clippy::diverging_sub_expression)]
        {
            let #tube_pat = #tauri_path::connect().expect("Failed to connect to tauri");

            #body
        }
    };

    input.sig.inputs.clear();
    input.into_tokens(header, last_block)
}
