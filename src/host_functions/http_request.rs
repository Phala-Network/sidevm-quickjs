use anyhow::{anyhow, Context};
use log::info;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use crate::{runtime::time::sleep, service::OwnedJsValue};
use js::{AsBytes, Error as ValueError, FromJsValue, ToJsValue};

use super::*;

#[derive(Debug, Default)]
pub struct Headers {
    pairs: Vec<(String, String)>,
}

impl FromJsValue for Headers {
    fn from_js_value(value: js::Value) -> Result<Self, ValueError> {
        Ok(if value.is_array() {
            Vec::<(String, String)>::from_js_value(value)?.into()
        } else {
            BTreeMap::<String, String>::from_js_value(value)?.into()
        })
    }
}

impl ToJsValue for Headers {
    fn to_js_value(&self, ctx: &js::Context) -> Result<js::Value, ValueError> {
        self.pairs.to_js_value(ctx)
    }
}

impl From<Vec<(String, String)>> for Headers {
    fn from(pairs: Vec<(String, String)>) -> Self {
        Self { pairs }
    }
}

impl From<BTreeMap<String, String>> for Headers {
    fn from(headers: BTreeMap<String, String>) -> Self {
        Self {
            pairs: headers.into_iter().collect(),
        }
    }
}

impl From<Headers> for Vec<(String, String)> {
    fn from(headers: Headers) -> Self {
        headers.pairs
    }
}

impl FromIterator<(String, String)> for Headers {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        Self {
            pairs: iter.into_iter().collect(),
        }
    }
}

#[derive(FromJsValue, Debug)]
#[qjsbind(rename_all = "camelCase")]
pub struct HttpRequest {
    url: String,
    #[qjsbind(default = "default_method")]
    method: String,
    #[qjsbind(default)]
    headers: Headers,
    #[qjsbind(default, as_bytes)]
    body: Vec<u8>,
    text_body: Option<String>,
    #[qjsbind(default = "default_timeout")]
    timeout_ms: u64,
}

#[derive(ToJsValue, Debug)]
#[qjsbind(rename_all = "camelCase")]
struct HttpResponseHead {
    status: u16,
    status_text: String,
    version: String,
    headers: Headers,
}

#[derive(ToJsValue, Debug)]
struct Event<'a, Data> {
    name: &'a str,
    data: Data,
}

pub fn setup(ns: &js::Value) -> Result<()> {
    ns.define_property_fn("httpRequest", http_request)?;
    Ok(())
}

#[js::host_call(with_context)]
fn http_request(
    service: ServiceRef,
    _this: js::Value,
    req: HttpRequest,
    callback: OwnedJsValue,
) -> Result<u64> {
    Ok(service.spawn(callback, do_http_request, req))
}

fn default_method() -> String {
    "GET".into()
}

fn default_timeout() -> u64 {
    30_000
}

async fn do_http_request(weak_service: ServiceWeakRef, id: u64, req: HttpRequest) {
    let url = req.url.clone();
    let result = tokio::select! {
        _ = sleep(Duration::from_millis(req.timeout_ms)) => {
            Err(anyhow!("Timed out"))
        }
        result = do_http_request_inner(weak_service.clone(), id, req) => result,
    };
    if let Err(err) = result {
        invoke_callback(
            &weak_service,
            id,
            "error",
            &format!("Failed to request `{url}`: {err:?}"),
        );
    }
}

#[cfg(not(feature = "web"))]
async fn do_http_request_inner(
    weak_service: ServiceWeakRef,
    id: u64,
    req: HttpRequest,
) -> Result<()> {
    use crate::runtime::{http_connector, HyperExecutor};
    use core::pin::pin;
    use hyper::{body::HttpBody, Body};
    let connector = http_connector();
    let client = hyper::Client::builder()
        .executor(HyperExecutor)
        .build::<_, Body>(connector);
    let uri: hyper::Uri = req
        .url
        .parse()
        .with_context(|| format!("Failed to parse url: {}", req.url))?;
    let mut builder = hyper::Request::builder()
        .method(req.method.as_str())
        .uri(&uri);
    let body: Vec<u8> = if let Some(text_body) = req.text_body {
        text_body.into_bytes()
    } else {
        req.body
    };
    for (k, v) in req.headers.pairs.iter() {
        builder = builder.header(k.as_str(), v.as_str());
    }
    let headers: BTreeSet<&str> = req.headers.pairs.iter().map(|(k, _v)| k.as_str()).collect();
    // Append Host, Content-Length and Content-Length if not present
    if !headers.contains("Host") {
        builder = builder.header("Host", uri.host().unwrap_or_default());
    }
    if !headers.contains("Content-Length") {
        builder = builder.header("Content-Length", body.len());
    }
    if !headers.contains("User-Agent") {
        builder = builder.header("User-Agent", "PhatContract/0.1.0");
    }
    let request = builder
        .body(Body::from(body))
        .context("Failed to build request")?;
    let response = client.request(request).await?;
    {
        let head = {
            let headers = response
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().into(), v.to_str().unwrap_or_default().into()))
                .collect();
            let status = response.status().as_u16();
            let status_text = response
                .status()
                .canonical_reason()
                .unwrap_or_default()
                .into();
            let version = format!("{:?}", response.version());
            HttpResponseHead {
                status,
                status_text,
                version,
                headers,
            }
        };
        invoke_callback(&weak_service, id, "head", &head);
    }
    let mut response = pin!(response);
    while let Some(chunk) = response.data().await {
        let chunk = chunk.context("Failed to read response body")?;
        invoke_callback(&weak_service, id, "data", &AsBytes(chunk));
    }
    invoke_callback(&weak_service, id, "end", &());
    Ok(())
}

#[cfg(feature = "web")]
async fn do_http_request_inner(
    weak_service: ServiceWeakRef,
    id: u64,
    req: HttpRequest,
) -> Result<()> {
    use reqwest::{Client, Method};
    let method = Method::from_bytes(req.method.as_bytes()).context("Invalid method")?;
    let mut builder = Client::new().request(method, req.url);
    for (k, v) in req.headers.pairs.iter() {
        builder = builder.header(k, v);
    }
    let headers: BTreeSet<&str> = req.headers.pairs.iter().map(|(k, _v)| k.as_str()).collect();
    // Append User-Agent if not present
    if !headers.contains("User-Agent") {
        builder = builder.header("User-Agent", "PhatContract/0.1.0");
    }
    let body: Vec<u8> = if let Some(text_body) = req.text_body {
        text_body.into_bytes()
    } else {
        req.body
    };
    builder = builder.body(body);
    let response = builder.send().await?;
    let head = {
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.to_str().unwrap_or_default().into()))
            .collect();
        let status = response.status().as_u16();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or_default()
            .into();
        HttpResponseHead {
            status,
            status_text,
            version: "HTTP/1.1".into(),
            headers,
        }
    };
    invoke_callback(&weak_service, id, "head", &head);
    let body = response.bytes().await?;
    invoke_callback(&weak_service, id, "data", &AsBytes(body));
    invoke_callback(&weak_service, id, "end", &());
    Ok(())
}

fn invoke_callback(weak_service: &Weak<Service>, id: u64, name: &str, data: &dyn ToJsValue) {
    let Some(service) = weak_service.upgrade() else {
        info!("http_request {id} exited because the service has been dropped");
        return;
    };
    let Some(callback) = service.get_resource_value(id) else {
        info!("http_request {id} exited because the resource has been dropped");
        return;
    };
    if let Err(err) = service.call_function(callback, (name, data)) {
        error!("[{id}] Failed to report http_request event {name}: {err:?}");
    }
}
