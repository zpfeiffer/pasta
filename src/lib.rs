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
use wasm_bindgen_futures::JsFuture;
use serde::{Serialize, Deserialize};
use web_sys::{FetchEvent, FormData, Headers, Request, Response, ResponseInit};

// TODO: Set http cache to expiration

pub(crate) const BASE_DOMAIN: &str = "pasta.zpfeiffer.com";
pub(crate) const BASE_URL: &str = "https://pasta.zpfeiffer.com";
pub(crate) const BASE_PASTE_URL: &str = "https://pasta.zpfeiffer.com/paste/";
pub(crate) const ALLOW_NEVER_EXPIRE: bool = false;

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
pub async fn main(req: Request) -> Promise {
    utils::set_panic_hook();
    match handle_request(req).await {
        Ok(promise) => promise,
        Err(ResponseError::ContentTypeError) => error_response::<400>(ResponseError::ContentTypeError),
        Err(ResponseError::InvalidMethod) => todo!(),
        Err(ResponseError::NonexistentResource) => not_found().ok_or_reject(),
        Err(ResponseError::InvalidExpiration) => error_response::<400>(ResponseError::InvalidExpiration),
        Err(ResponseError::MissingFormValue) => error_response::<400>(ResponseError::MissingFormValue),
        Err(e) => Promise::reject(&e.as_js_value())
    }
}

async fn handle_request(req: Request) -> Result<Promise, ResponseError> {
    let url = Url::parse(&req.url())?;

    let mut path_segments = url.path_segments()
        .ok_or(ResponseError::PathNotUnderstood)?;

    let first_segment = path_segments.next();
    if first_segment != Some("paste") {
        return Err(ResponseError::RouteError);
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
        return Err(ResponseError::NonexistentResource);
    }

    enum HttpMethod {
        Post,
        Get,
    }
    let method_str = req.method();
    let method = match method_str.as_ref() {
        "GET" => Ok(HttpMethod::Get),
        "POST" => Ok(HttpMethod::Post),
        _ => Err(ResponseError::InvalidMethod)
    }?;

    match (paste_id, method) {
        (None, HttpMethod::Get) => {
            // Redirect to index
            let resp = Response::redirect_with_status(BASE_URL, 301)?;
            Ok(Promise::from(JsValue::from(resp)))
        }
        (None, HttpMethod::Post) => {
            let content_type = req.headers().get("content-type")?;
            if content_type.as_deref() != Some("application/x-www-form-urlencoded") {
                return Err(ResponseError::ContentTypeError);
            }
            let form_data = FormData::from(JsFuture::from(req.form_data()?).await?);
            create_paste(form_data).await
        },
        (Some(requested_id), HttpMethod::Get) => {
            render_paste(requested_id).await
        },
        (_, _) => Err(ResponseError::InvalidMethod),
    }
}

fn generate_response(body: &str, status: u16, headers: &Headers) -> Result<Response, JsValue> {
    let mut init = ResponseInit::new();
    init.status(status);
    init.headers(&JsValue::from(headers));
    Response::new_with_opt_str_and_init(Some(body), &init)
}

/// Returns a `Promise` for a `Response` `JsValue` with the status set to
/// `STATUS`, the `content-type` header set to `text/html` and body set to
/// `error.to_string()`.
fn error_response<const STATUS: u16>(error: ResponseError) -> Promise {
    fn inner<const STATUS: u16>(error: ResponseError) -> Result<Promise, ResponseError> {
        let headers = Headers::new()?;
        headers.append("content-type", "text/html")?;

        let mut init = ResponseInit::new();
        init.status(STATUS);
        init.headers(&JsValue::from(headers));

        let body = error.to_string();

        let resp = Response::new_with_opt_str_and_init(Some(&body), &init)?;
        Ok(Promise::from(JsValue::from(resp)))
    }
    ok_or_reject(inner::<STATUS>(error))
}

fn ok_or_reject(result: Result<Promise, ResponseError>) -> Promise {
    match result {
        Ok(promise) => promise,
        Err(e) => Promise::reject(&e.as_js_value())
    }
}

async fn render_paste(requested_id_str: &str) -> Result<Promise, ResponseError> {
    // Retrieve paste asynchronously from KV
    let paste_result = StoredPaste::get_from_uuid_str(requested_id_str);

    // Construct the rest of the response
    let headers = Headers::new()?;
    headers.append("content-type", "text/html")?;
    let mut resp_init = ResponseInit::new();

    // Block until paste has been retrieved and parsed before rendering
    // the HTML and finalizing the response object.
    let resp = if let Some(paste) = paste_result.await? {
        if let Some(exp) = paste.exp {
            headers.append("Expires", &exp.to_rfc2822())?;
            headers.append("Cache-Control", "public")?;
            headers.append("Cache-Control", "immutable")?;
        } else {
            // Because these are Uuid v4s, if its not found its going to be
            // not found for the forseable future, right?
            // headers.append("Cache-Control", "immutable");
        }
        resp_init.headers(&JsValue::from(headers));
        resp_init.status(200);
        let body = paste.render_html();
        Response::new_with_opt_str_and_init(Some(&body), &resp_init)?
    } else {
        resp_init.headers(&JsValue::from(headers));
        resp_init.status(404);
        let body = include_str!("../public/404.html");
        Response::new_with_opt_str_and_init(Some(body), &resp_init)?
    };
    Ok(Promise::from(JsValue::from(resp)))
}

/// Creates a new paste with a randomly assigned ID and returns a Promise for
/// a response.
async fn create_paste(form: FormData) -> Result<Promise, ResponseError> {
    let new_paste = NewPaste::from_form_data(form)?;
    let put_future = new_paste.put();

    let url = put_future.await?;

    // Construct the  response
    let resp = Response::redirect_with_status(&url, 303)?;
    Ok(Promise::from(JsValue::from(resp)))
}

fn not_found() -> Result<Promise, ResponseError> {
    const HTML: &str = include_str!("../public/404.html");
    let headers = Headers::new()?;
    headers.append("content-type", "text/html")?;
    let mut init = ResponseInit::new();
    init.status(404);
    init.headers(&JsValue::from(headers));
    let resp = Response::new_with_opt_str_and_init(Some(HTML), &init)?;
    Ok(Promise::from(JsValue::from(resp)))
}

// TODO: Match render errors to HTTP status codes where appropriate:
// https://developer.mozilla.org/en-US/docs/Web/HTTP/Status

trait ExpectJs {
    fn ok_or_reject(self) -> Promise;
}

impl<E: ToString> ExpectJs for Result<Promise, E> {
    fn ok_or_reject(self) -> Promise {
        match self {
            Ok(promise) => promise,
            Err(e) => {
                let err_string = e.to_string();
                let js_val = JsValue::from_str(&err_string);
                Promise::reject(&js_val)
            }
        }
    }
}

#[derive(Debug, Error)]
enum ResponseError {
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

impl From<JsValue> for ResponseError {
    #[inline]
    fn from(value: JsValue) -> Self {
        Self::JsError(value)
    }
}

impl ResponseError {
    fn as_js_value(self) -> JsValue {
        let error_string = self.to_string();
        JsValue::from_str(error_string.as_str())
    }
}
