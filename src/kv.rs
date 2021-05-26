use wasm_bindgen::{JsValue, prelude::*};
use serde::{Serialize, Deserialize};
use js_sys::{JSON, JsString, Object, Promise};
use wasm_bindgen_futures::JsFuture;

use crate::{KvError, Paste};

#[wasm_bindgen]
extern "C" {
    type PasteNs;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn get(key: &str, data_type: &str) -> Promise;

    // #[wasm_bindgen(static_method_of = PasteNs)]
    // fn put(key: &str, val: &str) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn put(key: &str, val: JsString, obj: Object) -> Promise;

    //#[wasm_bindgen(static_method_of = PasteNs)]
    //fn put(key: &str, val: &str, named: Option<&PasteNsPutConfig>) -> Promise;

    #[wasm_bindgen]
    fn put_paste_ttl(key: &str, val: &str, ttl: u64) -> Promise;

    #[wasm_bindgen(static_method_of = PasteNs)]
    fn delete(key: &str) -> Promise;

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
extern {
    fn alert(s: &str);
    fn test1(x: JsValue);
    fn test2(x: Object);
}

/*
#[wasm_bindgen]
#[derive(Debug, Serialize, Deserialize)]
struct PasteNsPutConfig {
    #[serde(rename = "expirationTtl")]
    pub ttl: u32,
}
*/

//#[wasm_bindgen]
pub(crate) async fn put_paste_with_ttl(key: &str, paste: Paste, ttl: u32) -> Result<(JsValue, Paste), KvError> {
    let paste_json = serde_json::to_string(&paste)?;
    let val = JsValue::from_serde(&paste)?;
    //let val = JsValue::from(paste);
    //let val_str = JSON::stringify(&val)?;
    let val_str = JsString::from(paste_json);
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"expirationTtl".into(), &ttl.into());
    let promise = PasteNs::put(key, val_str, obj);
    let future = JsFuture::from(promise);
    Ok((future.await?, paste))
}
