use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat};

#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    // AttributeArgs is not available in basic syn features or has changed path.
    // Instead of parse_macro_input!(attr as AttributeArgs), we can parse as a comma-separated list of Metas.
    let attr_metas = syn::parse_macro_input!(attr with syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated);

    let fn_name = &input_fn.sig.ident;
    let struct_name = format_ident!("{}Tool", fn_name.to_string().to_uppercase());
    let fn_name_str = fn_name.to_string();

    // Parse attributes for name and description overrides
    let mut tool_name = fn_name_str.clone();
    let mut tool_desc = String::new();

    for meta in attr_metas {
        if let syn::Meta::NameValue(nv) = meta {
            if nv.path.is_ident("name") {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = nv.value
                {
                    tool_name = lit.value();
                }
            } else if nv.path.is_ident("description") {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = nv.value
                {
                    tool_desc = lit.value();
                }
            }
        }
    }

    // If description is missing, try to get from doc comments
    if tool_desc.is_empty() {
        for attr in &input_fn.attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit),
                        ..
                    }) = &nv.value
                    {
                        let doc = lit.value().trim().to_string();
                        if !tool_desc.is_empty() {
                            tool_desc.push('\n');
                        }
                        tool_desc.push_str(&doc);
                    }
                }
            }
        }
    }

    // Parse arguments to build schema and invocation
    let mut args_schema_fields = Vec::new();
    let mut invoke_args = Vec::new();

    for input in &input_fn.sig.inputs {
        if let FnArg::Typed(pat_type) = input {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let arg_name = &pat_ident.ident;
                let arg_name_str = arg_name.to_string();
                let ty = &pat_type.ty;

                // Simple type mapping for schema (incomplete, just string/int/bool for now)
                let type_str = quote!(#ty).to_string();
                let json_type = match type_str.as_str() {
                    "String" | "& str" => "string",
                    "i32" | "i64" | "f32" | "f64" | "usize" => "number",
                    "bool" => "boolean",
                    _ => "string", // Default/fallback
                };

                args_schema_fields.push(quote! {
                    #arg_name_str: { "type": #json_type }
                });

                invoke_args.push(quote! {
                     serde_json::from_value(args.get(#arg_name_str).cloned().unwrap_or(serde_json::Value::Null))
                        .map_err(|e| wesichain_core::ToolError::InvalidInput(format!("Failed to parse argument '{}': {}", #arg_name_str, e)))?
                });
            }
        }
    }

    let expanded = quote! {
        #input_fn

        pub struct #struct_name;

        #[async_trait::async_trait]
        impl wesichain_core::Tool for #struct_name {
            fn name(&self) -> &str {
                #tool_name
            }

            fn description(&self) -> &str {
                #tool_desc
            }

            fn schema(&self) -> wesichain_core::Value {
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        #(#args_schema_fields),*
                    }
                })
            }

            async fn invoke(&self, args: wesichain_core::Value) -> Result<wesichain_core::Value, wesichain_core::ToolError> {
                let result = #fn_name(
                    #(#invoke_args),*
                ).await;

                // Assuming result can be converted to Value. Use helper trait or serde.
                // For simplicity, using serde_json::to_value
                match result {
                    Ok(val) => serde_json::to_value(val).map_err(|e| wesichain_core::ToolError::ExecutionFailed(e.to_string())),
                    Err(e) => Err(wesichain_core::ToolError::ExecutionFailed(e.to_string())),
                }
            }
        }
    };

    TokenStream::from(expanded)
}
