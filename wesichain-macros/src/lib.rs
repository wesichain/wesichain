use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat};

#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attr_metas = syn::parse_macro_input!(attr with syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated);

    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let pascal_name: String = fn_name_str
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect();
    let struct_name = format_ident!("{}Tool", pascal_name);
    let args_struct_name = format_ident!("{}Args", struct_name);

    let mut tool_name = fn_name_str.clone();
    let mut tool_desc = String::new();

    for meta in attr_metas {
        if let syn::Meta::NameValue(nv) = meta {
            if nv.path.is_ident("name") {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = nv.value {
                    tool_name = lit.value();
                }
            } else if nv.path.is_ident("description") {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = nv.value {
                    tool_desc = lit.value();
                }
            }
        }
    }

    if tool_desc.is_empty() {
        for attr in &input_fn.attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = &nv.value {
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

    let mut param_names = Vec::new();
    let mut param_types = Vec::new();

    for input in &input_fn.sig.inputs {
        if let FnArg::Typed(pat_type) = input {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                param_names.push(pat_ident.ident.clone());
                param_types.push((*pat_type.ty).clone());
            }
        }
    }

    let args_struct_fields: Vec<_> = param_names.iter().zip(param_types.iter()).map(|(name, ty)| {
        quote! { pub #name: #ty }
    }).collect();

    let typed_invoke_args: Vec<_> = param_names.iter().map(|arg_name| {
        quote! { args.#arg_name }
    }).collect();

    let invoke_args: Vec<_> = param_names.iter().map(|arg_name| {
        let arg_name_str = arg_name.to_string();
        quote! {
            serde_json::from_value(legacy_args.get(#arg_name_str).cloned().unwrap_or(serde_json::Value::Null))
               .map_err(|e| wesichain_core::ToolError::InvalidInput(format!("Failed to parse argument '{}': {}", #arg_name_str, e)))?
        }
    }).collect();

    let expanded = quote! {
        #input_fn

        #[derive(serde::Deserialize, schemars::JsonSchema)]
        pub struct #args_struct_name {
            #(#args_struct_fields),*
        }

        pub struct #struct_name;

        #[async_trait::async_trait]
        impl wesichain_core::TypedTool for #struct_name {
            type Args = #args_struct_name;
            type Output = serde_json::Value;
            const NAME: &'static str = #tool_name;

            async fn run(&self, args: Self::Args, _ctx: wesichain_core::ToolContext)
                -> Result<Self::Output, wesichain_core::ToolError>
            {
                let result = #fn_name(#(#typed_invoke_args),*).await;
                match result {
                    Ok(val) => serde_json::to_value(val)
                        .map_err(|e| wesichain_core::ToolError::ExecutionFailed(e.to_string())),
                    Err(e) => Err(wesichain_core::ToolError::ExecutionFailed(e.to_string())),
                }
            }
        }

        #[async_trait::async_trait]
        impl wesichain_core::Tool for #struct_name {
            fn name(&self) -> &str { #tool_name }
            fn description(&self) -> &str { #tool_desc }
            fn schema(&self) -> wesichain_core::Value {
                serde_json::to_value(schemars::schema_for!(#args_struct_name))
                    .unwrap_or(serde_json::Value::Null)
            }
            async fn invoke(&self, legacy_args: wesichain_core::Value)
                -> Result<wesichain_core::Value, wesichain_core::ToolError>
            {
                let result = #fn_name(
                    #(#invoke_args),*
                ).await;
                match result {
                    Ok(val) => serde_json::to_value(val)
                        .map_err(|e| wesichain_core::ToolError::ExecutionFailed(e.to_string())),
                    Err(e) => Err(wesichain_core::ToolError::ExecutionFailed(e.to_string())),
                }
            }
        }
    };

    TokenStream::from(expanded)
}
