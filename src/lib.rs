use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process_file(data: &[u8]) -> String {
    String::from_utf8_lossy(data).to_uppercase()
}
