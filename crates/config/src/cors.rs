use ascii::AsciiString;
use duration_str::deserialize_option_duration;
use std::time::Duration;
use url::Url;

/// Configuration for CORS (Cross-Origin Resource Sharing)
#[derive(Clone, Default, Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CorsConfig {
    /// If false (or not defined), credentials are not allowed in requests
    pub allow_credentials: bool,
    /// Origins from which we allow requests
    pub allow_origins: Option<AnyOrUrlArray>,
    /// Maximum time between OPTIONS and the next request
    #[serde(deserialize_with = "deserialize_option_duration")]
    pub max_age: Option<Duration>,
    /// HTTP methods allowed to the endpoint.
    pub allow_methods: Option<AnyOrHttpMethodArray>,
    /// Headers allowed in incoming requests
    pub allow_headers: Option<AnyOrAsciiStringArray>,
    /// Headers exposed from the OPTIONS request
    pub expose_headers: Option<AnyOrAsciiStringArray>,
    /// If set, allows browsers from private network to connect
    pub allow_private_network: bool,
}

/// Represents a standard HTTP method.
#[derive(Debug, PartialEq, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// The GET method requests a representation of the specified resource. Requests using GET should only retrieve data.
    Get,
    /// The POST method submits an entity to the specified resource, often causing a change in state or side effects on the server.
    Post,
    /// The PUT method replaces all current representations of the target resource with the request payload.
    Put,
    /// The DELETE method deletes the specified resource.
    Delete,
    /// The HEAD method asks for a response identical to that of a GET request, but without the response body.
    Head,
    /// The OPTIONS method describes the communication options for the target resource.
    Options,
    /// The CONNECT method establishes a tunnel to the server identified by the target resource.
    Connect,
    /// The PATCH method applies partial modifications to a resource.
    Patch,
    /// The TRACE method performs a message loop-back test along the path to the target resource.
    Trace,
}

impl std::str::FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "HEAD" => Ok(HttpMethod::Head),
            "OPTIONS" => Ok(HttpMethod::Options),
            "CONNECT" => Ok(HttpMethod::Connect),
            "PATCH" => Ok(HttpMethod::Patch),
            "TRACE" => Ok(HttpMethod::Trace),
            _ => Err(format!("Unknown HTTP method: {s}")),
        }
    }
}

impl From<http::Method> for HttpMethod {
    fn from(value: http::Method) -> Self {
        if value == http::Method::GET {
            Self::Get
        } else if value == http::Method::POST {
            Self::Post
        } else if value == http::Method::PUT {
            Self::Put
        } else if value == http::Method::DELETE {
            Self::Delete
        } else if value == http::Method::PATCH {
            Self::Patch
        } else if value == http::Method::HEAD {
            Self::Head
        } else if value == http::Method::OPTIONS {
            Self::Options
        } else if value == http::Method::TRACE {
            Self::Trace
        } else if value == http::Method::CONNECT {
            Self::Connect
        } else {
            todo!("Unsupported HTTP method: {:?}", value);
        }
    }
}

impl From<HttpMethod> for http::Method {
    fn from(value: HttpMethod) -> Self {
        match value {
            HttpMethod::Get => http::Method::GET,
            HttpMethod::Post => http::Method::POST,
            HttpMethod::Put => http::Method::PUT,
            HttpMethod::Delete => http::Method::DELETE,
            HttpMethod::Head => http::Method::HEAD,
            HttpMethod::Options => http::Method::OPTIONS,
            HttpMethod::Connect => http::Method::CONNECT,
            HttpMethod::Patch => http::Method::PATCH,
            HttpMethod::Trace => http::Method::TRACE,
        }
    }
}

/// A type alias for `AnyOrArray` specifically for `Url` types.
pub type AnyOrUrlArray = AnyOrArray<Url>;

/// A type alias for `AnyOrArray` specifically for `HttpMethod` types.
pub type AnyOrHttpMethodArray = AnyOrArray<HttpMethod>;

/// A type alias for `AnyOrArray` specifically for `AsciiString` types.
pub type AnyOrAsciiStringArray = AnyOrArray<AsciiString>;

/// Represents a configuration option that can either allow "any" value
/// (e.g., signified by a wildcard string `*`) or a specific
/// explicit list of values.
#[derive(Clone, Debug, PartialEq)]
pub enum AnyOrArray<T> {
    /// Indicates that any value is allowed (e.g., `*`).
    Any,
    /// A specific, explicit list of allowed values.
    Explicit(Vec<T>),
}

impl<'de, T> serde::Deserialize<'de> for AnyOrArray<T>
where
    T: serde::Deserialize<'de> + std::str::FromStr<Err: std::fmt::Display>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AnyOrArrayVisitor<T> {
            _marker: std::marker::PhantomData<T>,
        }

        impl<'de, T> serde::de::Visitor<'de> for AnyOrArrayVisitor<T>
        where
            T: serde::Deserialize<'de> + std::str::FromStr<Err: std::fmt::Display>,
        {
            type Value = AnyOrArray<T>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("expecting string \"*\", or an array of values")
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value == "*" {
                    Ok(AnyOrArray::Any)
                } else {
                    value
                        .parse::<T>()
                        .map_err(|err| E::custom(err))
                        .map(|value| AnyOrArray::Explicit(vec![value]))
                }
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut array = Vec::new();
                while let Some(value) = seq.next_element()? {
                    array.push(value);
                }
                Ok(AnyOrArray::Explicit(array))
            }
        }

        deserializer.deserialize_any(AnyOrArrayVisitor {
            _marker: std::marker::PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use ascii::AsciiString;
    use indoc::indoc;
    use insta::assert_snapshot;
    use std::time::Duration;

    #[test]
    fn cors_allow_credentials() {
        let input = indoc! {r#"
            [server.cors]
            allow_credentials = true
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert!(cors.allow_credentials);
    }

    #[test]
    fn cors_allow_credentials_default() {
        let input = indoc! {r#"
            [server.cors]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert!(!cors.allow_credentials);
    }

    #[test]
    fn cors_max_age() {
        let input = indoc! {r#"
           [server.cors]
           max_age = "60s"
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(Some(Duration::from_secs(60)), cors.max_age);
    }

    #[test]
    fn cors_allow_origins_default() {
        let input = indoc! {r#"
            [server.cors]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(None, cors.allow_origins)
    }

    #[test]
    fn cors_allow_origins_any() {
        let input = indoc! {r#"
            [server.cors]
            allow_origins = "*"
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(Some(AnyOrUrlArray::Any), cors.allow_origins)
    }

    #[test]
    fn cors_allow_origins_explicit() {
        let input = indoc! {r#"
            [server.cors]
            allow_origins = ["https://app.grafbase.com"]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();
        let expected = AnyOrUrlArray::Explicit(vec!["https://app.grafbase.com".parse().unwrap()]);

        assert_eq!(Some(expected), cors.allow_origins)
    }

    #[test]
    fn cors_allow_origins_invalid_url() {
        let input = indoc! {r#"
            [server.cors]
            allow_origins = ["foo"]
        "#};

        let error = toml::from_str::<Config>(input).unwrap_err();

        assert_snapshot!(&error.to_string(), @r#"
        TOML parse error at line 2, column 18
          |
        2 | allow_origins = ["foo"]
          |                  ^^^^^
        relative URL without a base: "foo"
        "#);
    }

    #[test]
    fn cors_allow_methods_default() {
        let input = indoc! {r#"
            [server.cors]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(None, cors.allow_methods)
    }

    #[test]
    fn cors_allow_methods_any() {
        let input = indoc! {r#"
            [server.cors]
            allow_methods = "*"
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(Some(AnyOrHttpMethodArray::Any), cors.allow_methods)
    }

    #[test]
    fn cors_allow_methods_explicit() {
        let input = indoc! {r#"
            [server.cors]
            allow_methods = ["POST"]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();
        let expected = AnyOrHttpMethodArray::Explicit(vec![HttpMethod::Post]);

        assert_eq!(Some(expected), cors.allow_methods)
    }

    #[test]
    fn cors_allow_methods_invalid_method() {
        let input = indoc! {r#"
            [server.cors]
            allow_methods = ["MEOW"]
        "#};

        let error = toml::from_str::<Config>(input).unwrap_err();

        assert_snapshot!(&error.to_string(), @r#"
        TOML parse error at line 2, column 18
          |
        2 | allow_methods = ["MEOW"]
          |                  ^^^^^^
        unknown variant `MEOW`, expected one of `GET`, `POST`, `PUT`, `DELETE`, `HEAD`, `OPTIONS`, `CONNECT`, `PATCH`, `TRACE`
        "#);
    }

    #[test]
    fn cors_allow_headers_default() {
        let input = indoc! {r#"
            [server.cors]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(None, cors.allow_headers)
    }

    #[test]
    fn cors_allow_headers_any() {
        let input = indoc! {r#"
            [server.cors]
            allow_headers = "*"
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(Some(AnyOrAsciiStringArray::Any), cors.allow_headers)
    }

    #[test]
    fn cors_allow_headers_explicit() {
        let input = indoc! {r#"
            [server.cors]
            allow_headers = ["Content-Type"]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        let expected = AnyOrAsciiStringArray::Explicit(vec![AsciiString::from_ascii(b"Content-Type").unwrap()]);

        assert_eq!(Some(expected), cors.allow_headers)
    }

    #[test]
    fn cors_allow_headers_invalid() {
        let input = indoc! {r#"
            [server.cors]
            allow_headers = ["😂😂😂"]
        "#};

        let error = toml::from_str::<Config>(input).unwrap_err();

        assert_snapshot!(&error.to_string(), @r#"
        TOML parse error at line 2, column 18
          |
        2 | allow_headers = ["😂😂😂"]
          |                  ^^^^^^^^^^^^^^
        invalid value: string "😂😂😂", expected an ascii string
        "#);
    }

    #[test]
    fn cors_expose_headers_default() {
        let input = indoc! {r#"
            [server.cors]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(None, cors.expose_headers);
    }

    #[test]
    fn cors_expose_headers_any() {
        let input = indoc! {r#"
            [server.cors]
            expose_headers = "*"
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert_eq!(Some(AnyOrAsciiStringArray::Any), cors.expose_headers);
    }

    #[test]
    fn cors_expose_headers_explicit() {
        let input = indoc! {r#"
            [server.cors]
            expose_headers = ["Content-Type"]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        let expected = AnyOrAsciiStringArray::Explicit(vec![AsciiString::from_ascii(b"Content-Type").unwrap()]);

        assert_eq!(Some(expected), cors.expose_headers);
    }

    #[test]
    fn cors_expose_headers_invalid() {
        let input = indoc! {r#"
            [server.cors]
            expose_headers = ["😂😂😂"]
        "#};

        let error = toml::from_str::<Config>(input).unwrap_err();

        assert_snapshot!(&error.to_string(), @r#"
        TOML parse error at line 2, column 19
          |
        2 | expose_headers = ["😂😂😂"]
          |                   ^^^^^^^^^^^^^^
        invalid value: string "😂😂😂", expected an ascii string
        "#);
    }

    #[test]
    fn cors_allow_private_network_default() {
        let input = indoc! {r#"
            [server.cors]
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert!(!cors.allow_private_network);
    }

    #[test]
    fn cors_allow_private_network_explicit() {
        let input = indoc! {r#"
            [server.cors]
            allow_private_network = true
        "#};

        let config: Config = toml::from_str(input).unwrap();
        let cors = config.server.cors.unwrap();

        assert!(cors.allow_private_network);
    }
}
