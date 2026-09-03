#![cfg(target_arch = "wasm32")]
// use bitmask_core::rgb::carbonado::FileMetadata;
use bitmask_core::carbonado::FileMetadata;
use bitmask_core::{
    info,
    web::{
        carbonado::{retrieve, retrieve_metadata, store},
        json_parse, resolve, set_panic_hook,
    },
};
use js_sys::Uint8Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn web_storage() {
    set_panic_hook();

    let sk = "76e9a09d5fa501c9048cb7ff48415786f7f6580726f33823010d130b19f61680".to_owned();
    let name = "test-my-file.c15".to_owned();
    let data = b"Hello world!".to_vec();

    info!("Testing web data store");
    resolve(store(sk.clone(), name.clone(), data.clone(), false, None)).await;

    info!("Testing web data retrieve");
    let result: JsValue = resolve(retrieve(sk.clone(), name.clone())).await;
    let array = Uint8Array::new(&result);
    let bytes: Vec<u8> = array.to_vec();
    assert_eq!(data, bytes, "Data stored and data retrieved match");

    let metadata: JsValue = resolve(retrieve_metadata(sk, name)).await;
    let metadata: FileMetadata = json_parse(&metadata);
    assert!(metadata.filename.ends_with(".c15"));
    assert_eq!(metadata.metadata, [0; 8]);
}
