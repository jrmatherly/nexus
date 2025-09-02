use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

/// A proc macro for marking tests as live provider tests.
/// Preserves spans for better error reporting and LSP support.
///
/// # Usage
/// ```
/// #[live_provider_test(bedrock)]
/// async fn test_name() {
///     // test body
/// }
/// ```
///
/// The test will only run when the environment variable `BEDROCK_LIVE_TESTS=true` is set.
/// All live tests are marked with `#[ignore]` by default.
#[proc_macro_attribute]
pub fn live_provider_test(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse provider name
    let provider = args.to_string().trim().to_string();

    // Parse the input function properly to preserve spans
    let input_fn = parse_macro_input!(input as ItemFn);

    // Extract function components
    let fn_name = input_fn.sig.ident.to_string();
    let fn_body = &input_fn.block;
    let fn_sig = &input_fn.sig;
    let is_async = fn_sig.asyncness.is_some();

    // Generate environment variable name
    let env_var = format!("{}_LIVE_TESTS", provider.to_uppercase());

    // Generate new test name with provider prefix
    let test_name = format!("live_{}_{}", provider, fn_name);
    let test_ident = quote::format_ident!("{}", test_name);

    // Build the new function signature with the test name
    let async_token = if is_async { quote!(async) } else { quote!() };

    // Generate the wrapped test preserving the original body's spans
    let output = quote! {
        #[tokio::test]
        #[ignore]
        #async_token fn #test_ident() {
            // Fast environment check - inline for better performance
            if std::env::var(#env_var).unwrap_or_default() != "true" {
                eprintln!(
                    "Skipping {} live test '{}' - set {}=true to run",
                    #provider,
                    #fn_name,
                    #env_var
                );
                return;
            }

            // Log execution (only when actually running)
            eprintln!("Running live {} test: {}", #provider, #fn_name);

            // Original function body with preserved spans
            #fn_body
        }
    };

    TokenStream::from(output)
}
