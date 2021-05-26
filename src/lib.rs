mod utils;
mod templates;
mod kv;

use cfg_if::cfg_if;
use kv::put_paste_with_ttl;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use js_sys::{Array, Error, Function, Promise, Reflect, JsString};
use url::{Url, ParseError as UrlParseError};
// use std::time::SystemTime;
use wasm_bindgen_futures::JsFuture;
use serde::{Serialize, Deserialize};
use web_sys::{FetchEvent, FormData, Headers, Request, Response, ResponseInit};
//use templates::index;

// TODO: Use array buffers for keys instead of strings

pub(crate) const BASE_URL: &str = "pasta.zpfeiffer.com";

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

    // #[wasm_bindgen(static_method_of = PasteNs)]
    // fn put(key: &str, val: &str) -> Promise;

    //#[wasm_bindgen(static_method_of = PasteNs)]
    //fn put(key: &str, val: &str, named: Option<&PasteNsPutConfig>) -> Promise;

    //#[wasm_bindgen]
    fn put_paste_ttl(key: &str, val: &str, ttl: u64) -> Promise;

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
    let path = url.path();

    let method = req.method();
    let mut path_segments = path.split("/");
    let first_segment = path_segments.nth(1);
    let paste_id = path_segments.next();
    match (first_segment, paste_id, method.as_ref()) {
        (Some("paste"), None, "GET") => {
            // TODO: Redirect to index
            todo!()
        }
        (Some("paste"), None, "POST") => {
            // TODO: Create new paste

            let new = Paste {
                id: "AAAAAAAA".to_string(),
                title: Some("ahh".to_string()),
                content: "nope".to_string(),
                author: "hha".to_string(),
                unlisted: true,
            };
            let (result, created) = put_paste_with_ttl("AAAAAAAA", new, 600).await?;
            let body = templates::paste_created(created).into_string();
            let headers = Headers::new()?;
            headers.append("content-type", "text/html")?;
            let resp = generate_response(&body, 201, &headers)?;
            Ok(Promise::from(JsValue::from(resp)))
        },
        (Some("paste"), Some(requested_id), "GET") => {
            // TODO: Get paste
            render_paste(requested_id).await
            //todo!()
        },
        (Some("paste"), _, _) => Err(RenderError::InvalidMethod),
        (_, _, _) => Err(RenderError::RouteError),
    }
}

fn generate_response(body: &str, status: u16, headers: &Headers) -> Result<Response, JsValue> {
    let mut init = ResponseInit::new();
    init.status(status);
    init.headers(&JsValue::from(headers));
    Response::new_with_opt_str_and_init(Some(body), &init)
}

async fn render_paste(requested_id: &str) -> Result<Promise, RenderError> {
    // Retrieve paste asynchronously from KV
    let paste = Paste::get(requested_id);

    // Construct the rest of the response
    let headers = Headers::new()?;
    headers.append("content-type", "text/html")?;
    let mut resp_init = ResponseInit::new();
    resp_init.status(200);
    resp_init.headers(&JsValue::from(headers));

    // Block until paste has been retrieved and parsed before rendering
    // the HTML and finalizing the response object.
    let body = paste.await?.render_html();
    let resp = Response::new_with_opt_str_and_init(Some(&body), &resp_init)?;
    Ok(Promise::from(JsValue::from(resp)))
}

/// Creates a new paste with a randomly assigned ID and returns a Promise for
/// a response.
async fn create_paste() -> Result<Promise, RenderError> {
    todo!()
}

#[derive(Debug, Error)]
enum RenderError {
    #[error("url parse error: {0}")]
    UrlParseError(#[from] UrlParseError),

    #[error("js error: {0:?}")]
    JsError(JsValue),

    #[error("kv error: {0}")]
    KvError(#[from] KvError),

    #[error("route error: this worker should not have received this request")]
    RouteError,

    #[error("invalid method")]
    InvalidMethod,
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

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Paste {
    id: String,
    title: Option<String>,
    content: String,
    author: String,
    unlisted: bool,
}

impl Paste {
    async fn get(paste_id: &str) -> Result<Paste, KvError> {
        let promise = PasteNs::get(paste_id, "json");
        let retrieved = JsFuture::from(promise)
            .await
            .map_err(KvError::from)?;
        if !retrieved.is_null() {
            retrieved.into_serde::<Paste>().map_err(KvError::from)
        } else {
            Err(KvError::NotFound)
        }
    }

    fn render_html(self) -> String {
        templates::paste(self).into_string()
    }

    #[inline]
    fn get_path(&self) -> String {
        format!("/paste/{}", self.id)
    }
}

#[derive(Debug, Error)]
pub(crate) enum KvError {
    #[error("js error: {0:?}")]
    JsError(JsValue),

    /// Key was not found in namespace.
    ///
    /// Constructed when the KV namespace returns a null JsValue
    #[error("key not found")]
    NotFound,

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
