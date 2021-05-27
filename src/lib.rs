mod utils;
mod templates;
mod kv;

use cfg_if::cfg_if;
use kv::{KvError, NewPaste, StoredPaste};
use thiserror::Error;
use uuid::Uuid;
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
    //fn put_paste_ttl(key: &str, val: &str, ttl: u64) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn delete(key: &str) -> Promise;
}

#[wasm_bindgen]
pub async fn main(req: Request) -> Promise {
    utils::set_panic_hook();
    match render_main(req).await {
        Ok(promise) => promise,
        Err(e) => Promise::reject(&e.as_js_value())
        // TODO: Redirect based on some error values
    }
}

async fn render_main(req: Request) -> Result<Promise, RenderError> {
    let url = Url::parse(&req.url())?;

    let mut path_segments = url.path_segments()
        .ok_or(RenderError::PathNotUnderstood)?;

    let first_segment = path_segments.next();
    if first_segment != Some("paste") {
        return Err(RenderError::RouteError);
    }

    // Get paste ID from next path segment. If the next value is `Some("")`,
    // the path may be of the form: domain.tld/paste/ (note the trailing /).
    // Such a trailing `/` is ignored here.
    let paste_id = match path_segments.next() {
        Some("") | None => None,
        Some(segment) => Some(segment)
    };

    // The last path segment should already have been encountered
    // (trailing `/`s are permitted). If not, this request must be
    // for a resource this worker can not serve.
    if !matches!(path_segments.next(), Some("") | None) {
        return Err(RenderError::NonexistentResource);
    }

    let method_str = req.method();
    enum HttpMethod {
        Post,
        Get,
    }
    let method = match method_str.as_ref() {
        "GET" => Ok(HttpMethod::Get),
        "POST" => Ok(HttpMethod::Post),
        _ => Err(RenderError::InvalidMethod)
    }?;

    match (paste_id, method) {
        (None, HttpMethod::Get) => {
            // TODO: Redirect to index
            todo!()
        }
        (None, HttpMethod::Post) => {
            // TODO: use multipart/form data
            // TODO: should I use jsfuture/promise .then here?
            let content_type = req.headers().get("content-type")?;
            if content_type.as_deref() != Some("application/x-www-form-urlencoded") {
                return Err(RenderError::ContentTypeError);
            }
            let form_data = FormData::from(JsFuture::from(req.form_data()?).await?);
            create_paste(form_data).await
        },
        (Some(requested_id), HttpMethod::Get) => {
            render_paste(requested_id).await
        },
        (_, _) => Err(RenderError::InvalidMethod),
    }
}

fn generate_response(body: &str, status: u16, headers: &Headers) -> Result<Response, JsValue> {
    let mut init = ResponseInit::new();
    init.status(status);
    init.headers(&JsValue::from(headers));
    Response::new_with_opt_str_and_init(Some(body), &init)
}

async fn render_paste(requested_id_str: &str) -> Result<Promise, RenderError> {
    // Retrieve paste asynchronously from KV
    let paste = StoredPaste::get_from_uuid_str(requested_id_str);

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
async fn create_paste(form: FormData) -> Result<Promise, RenderError> {
    // TODO: See if we can avoid copying into linear memory just to check string equality
    let title: Option<String>  = form.get("paste-title").as_string();
    let author: Option<String> = form.get("paste-author").as_string();
    let content = form.get("paste-content")
        .as_string()
        .ok_or(RenderError::MissingFormValue)?;
    let ttl = match form.get("expiration").as_string().as_deref() {
        Some("1 hour") => Ok(Some(3600u32)),
        Some("24 hours") => Ok(Some(86400)),
        Some("Never") => Ok(None),
        Some(_) => Err(RenderError::InvalidExpiration),
        None => Err(RenderError::MissingFormValue),
    }?;
    let unlisted = match form.get("privacy").as_string().as_deref() {
        Some("Public") => Ok(false),
        Some("Unlisted") => Ok(true),
        Some(_) => Err(RenderError::InvalidExpiration),
        None => Err(RenderError::MissingFormValue),
    }?;
    let new_paste = NewPaste::new(title, content, author, unlisted, ttl);
    //let (stored_paste, path) = new_paste.put().await?;
    let put_future = new_paste.put();

    // Construct the rest of the response
    let headers = Headers::new()?;
    headers.append("content-type", "text/html")?;
    let mut resp_init = ResponseInit::new();
    resp_init.status(200);
    resp_init.headers(&JsValue::from(headers));

    let (stored_paste, path) = put_future.await?;
    let html = templates::paste_created(stored_paste, &path).into_string();
    let resp = Response::new_with_opt_str_and_init(Some(&html), &resp_init)?;
    Ok(Promise::from(JsValue::from(resp)))
}

// TODO: Match render errors to HTTP status codes where appropriate:
// https://developer.mozilla.org/en-US/docs/Web/HTTP/Status

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

    /// The requested path was understood and correctly routed
    /// to this worker but does not correspond to any resource.
    ///
    /// HTTP 404 would be an appropriate response to this error.
    #[error("requested resource does not exist")]
    NonexistentResource,

    /// The requested path was not understood.
    ///
    /// This error should never occur, as URL validity should be a requirement
    /// for the request to be routed to this worker.
    #[error("url path not understood")]
    PathNotUnderstood,

    /// The request contained an expiration value that is not a valid option
    #[error("invalid expiration")]
    InvalidExpiration,

    #[error("submitted form missing values")]
    MissingFormValue,

    #[error("the request's content-type field was missing or invalid")]
    ContentTypeError,
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
