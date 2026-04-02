use wasm_bindgen::JsValue;

pub fn js_err(msg: impl ToString) -> JsValue {
    JsValue::from_str(&msg.to_string())
}
