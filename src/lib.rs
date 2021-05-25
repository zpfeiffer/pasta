extern crate cfg_if;
extern crate wasm_bindgen;

mod utils;
mod templates;

use cfg_if::cfg_if;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use js_sys::{Array, Error, Function, Promise, Reflect, JsString};
use arrayvec::ArrayString;
use url::{Url, ParseError as UrlParseError};
// use std::time::SystemTime;
use wasm_bindgen_futures::JsFuture;
use serde::{Serialize, Deserialize};
use web_sys::{FetchEvent, FormData, Headers, Request, Response, ResponseInit};
//use templates::index;

// TODO: Use array buffers for keys instead of strings

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen]
extern "C" {
    type PasteNs;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn get(key: &str, data_type: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn put(key: &str, val: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn delete(key: &str) -> Promise;
}

#[wasm_bindgen]
pub async fn main(req: Request) -> Promise {
    match render_main(req).await {
        Ok(promise) => promise,
        Err(e) => Promise::reject(&e.as_js_value())
    }
}

async fn render_main(req: Request) -> Result<Promise, RenderError> {
    let url = Url::parse(&req.url())?;

    // Note that `Url::path` returns a percent-encoded ASCII string
    let path = url.path().to_ascii_lowercase();

    let method = req.method();
    match path.split("/").nth(1) {
        Some("") => {
            match method.as_ref() {
                "GET" => {
                    // let body = "<h1>hello!</h1>";
                    let body = templates::index().into_string();
                    let headers = Headers::new()?;
                    headers.append("content-type", "text/html")?;
                    let resp = generate_response(&body, 200, &headers)?;
                    Ok(Promise::from(JsValue::from(resp)))
                }
                _ => todo!()
            }
        }
        Some("paste") => {
            match method.as_ref() {
                "GET" => {
                    render_paste(req).await
                }
                _ => todo!()
            }
        }
        _ => {
            todo!()
        }
    }
}

fn generate_response(body: &str, status: u16, headers: &Headers) -> Result<Response, JsValue> {
    let mut init = ResponseInit::new();
    init.status(status);
    init.headers(&JsValue::from(headers));
    Response::new_with_opt_str_and_init(Some(body), &init)
}

async fn render_paste(req: Request) -> Result<Promise, RenderError> {
    let body = Paste::get("ABCDEFGH".to_string())
        .await?
        .render_html();
    let headers = Headers::new()?;
    headers.append("content-type", "text/html")?;
    let resp = generate_response(&body, 200, &headers)?;
    Ok(Promise::from(JsValue::from(resp)))
}

#[derive(Debug, Error)]
enum RenderError {
    #[error("url parse error: {0}")]
    UrlParseError(#[from] UrlParseError),

    #[error("js error: {0:?}")]
    JsError(JsValue),

    #[error("kv error: {0}")]
    KvError(#[from] KvError)
}

impl From<JsValue> for RenderError {
    #[inline]
    fn from(value: JsValue) -> Self {
        Self::JsError(value)
    }
}

impl RenderError {
    fn as_js_value(self) -> JsValue {
        let error_string = self.to_string();
        JsValue::from_str(error_string.as_str())
    }
}

/* 
 * Request:
 * POST /new
 *
 * Response:
 * 201 (Created) w/ Location header
 * Content TBD possibly Refresh header or HTML redirect or JS redirect to new resource
 */

//type PasteId = ArrayString::<8>;
type PasteId = String;

#[derive(Serialize, Deserialize, Debug)]
struct Paste {
    id: PasteId,
    title: Option<String>,
    content: String,
    author: String,
}

impl Paste {
    async fn get(paste_id: PasteId) -> Result<Paste, KvError> {
        let promise = PasteNs::get(paste_id.as_str(), "json");
        JsFuture::from(promise)
            .await
            .map_err(KvError::from)?
            .into_serde::<Paste>()
            .map_err(KvError::from)
    }

    #[inline]
    fn id_str(&self) -> &str {
        self.id.as_str()
    }

    fn render_html(self) -> String {
        let title = self.title.unwrap_or(self.id);
        format!(
            "<html>
            <h1>{0}</h1>
            <p>author: {1}</p>
            <pre>{2}</pre>
            </html>",
            title,
            self.author,
            self.content
        )
    }
}

#[derive(Debug, Error)]
enum KvError {
    #[error("js error: {0:?}")]
    JsError(JsValue),

    // #[error("expected kv get result to be a string")]
    // ResultNotString,

    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::error::Error),
}

impl From<JsValue> for KvError {
    #[inline]
    fn from(value: JsValue) -> Self {
        Self::JsError(value)
    }
}
