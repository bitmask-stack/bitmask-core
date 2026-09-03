#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use carbonado::file::Header;

#[tokio::test]
async fn v2_header_len_on_hermetic_encode() -> Result<()> {
    let sk_hex = "76e9a09d5fa501c9048cb7ff48415786f7f6580726f33823010d130b19f61680";
    let sk = hex::decode(sk_hex)?;
    assert_eq!(sk.len(), 32, "Carbonado v2 master key is 32 bytes");

    let input = b"bitmask-core carbonado v2 header";
    let (body, _info) = carbonado::file::encode(&sk, input, 15, None)?;
    assert!(
        body.len() > Header::LEN,
        "encoded body must be longer than the v2 header"
    );
    let header = Header::try_from(&body[..Header::LEN])?;
    assert_eq!(header.try_to_vec()?.len(), Header::LEN);

    let (_decoded_header, decoded) = carbonado::file::decode(&sk, &body)?;
    assert_eq!(decoded, input);
    Ok(())
}
