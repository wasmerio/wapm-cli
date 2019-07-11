//! Code for dealing with proxies

#[derive(Debug, Fail)]
pub enum ProxyError {
    #[fail(
        display = "Failed to parse URL from {}: {}",
        url_location, error_message
    )]
    UrlParseError {
        url_location: String,
        error_message: String,
    },

    #[fail(display = "Could not connect to proxy: {}", _0)]
    ConnectionError(String),
}

/// Tries to set up a proxy
///
/// This function reads from wapm config's `proxy.url` first, then checks
/// `ALL_PROXY`, `HTTPS_PROXY`, and `http_proxy` environment variables
/// in that order.
///
/// If a proxy is specified in wapm config's `proxy.url`, it is assumed
/// to be a general proxy
pub fn maybe_set_up_proxy() -> Result<Option<reqwest::Proxy>, failure::Error> {
    let maybe_proxy_url = crate::config::Config::from_file()
        .ok()
        .and_then(|config| config.proxy.url);
    let proxy = if let Some(proxy_url) = maybe_proxy_url {
        reqwest::Proxy::all(&proxy_url).map(|proxy| (proxy_url, proxy, "`proxy.url` config key"))
    } else if let Ok(proxy_url) = std::env::var("ALL_PROXY") {
        reqwest::Proxy::all(&proxy_url).map(|proxy| (proxy_url, proxy, "ALL_PROXY"))
    } else if let Ok(https_proxy_url) = std::env::var("HTTPS_PROXY") {
        reqwest::Proxy::https(&https_proxy_url).map(|proxy| (https_proxy_url, proxy, "HTTPS_PROXY"))
    } else if let Ok(http_proxy_url) = std::env::var("http_proxy") {
        reqwest::Proxy::http(&http_proxy_url).map(|proxy| (http_proxy_url, proxy, "http_proxy"))
    } else {
        return Ok(None);
    }
    .map_err(|e| ProxyError::ConnectionError(e.to_string()))
    .and_then(
        |(proxy_url_str, proxy, url_location): (String, _, &'static str)| {
            url::Url::parse(&proxy_url_str)
                .map_err(|e| ProxyError::UrlParseError {
                    url_location: url_location.to_string(),
                    error_message: e.to_string(),
                })
                .map(|url| proxy.basic_auth(url.username(), url.password().unwrap_or_default()))
        },
    )?;

    Ok(Some(proxy))
}
