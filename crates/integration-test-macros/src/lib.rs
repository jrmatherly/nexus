use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// A fast proc macro for marking tests as live provider tests.
/// Optimized for minimal compilation overhead by avoiding full AST parsing.
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
    // Fast path: just grab provider name as string, no parsing
    let provider = args.to_string().trim().to_string();

    // Extract just the function name using minimal string operations
    let input_str = input.to_string();
    let fn_name = extract_function_name(&input_str);

    // Generate environment variable name
    let env_var = format!("{}_LIVE_TESTS", provider.to_uppercase());

    // Generate new test name with provider prefix
    let test_name = format!("live_{}_{}", provider, fn_name);
    let test_ident = quote::format_ident!("{}", test_name);

    // Extract the async keyword if present (simple string check)
    let is_async = input_str.trim_start().starts_with("async ");
    let async_token = if is_async { quote!(async) } else { quote!() };

    // Find where the function body starts (after the first {)
    let body_start = input_str.find('{').unwrap_or(0);

    // Extract just the function body
    let body = &input_str[body_start..];
    let body_tokens: TokenStream2 = body.parse().unwrap_or_else(|_| quote!({}));

    // Generate the wrapped test with minimal overhead
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

            // Original function body
            #body_tokens
        }
    };

    TokenStream::from(output)
}

/// Extract function name from source using simple string operations.
/// Avoids full syn parsing for better compilation performance.
fn extract_function_name(input: &str) -> String {
    // Look for "fn " pattern
    if let Some(fn_pos) = input.find("fn ") {
        let after_fn = &input[fn_pos + 3..];
        // Take characters until we hit a non-identifier character
        let name_end = after_fn
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after_fn.len());
        after_fn[..name_end].to_string()
    } else {
        "unknown".to_string()
    }
}
