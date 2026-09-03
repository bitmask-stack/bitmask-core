#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::{
    bitcoin::{decrypt_wallet, upgrade_wallet},
    constants::switch_network,
    structs::SecretString,
    util::init_logging,
};
use log::{debug, info};

const ENCRYPTION_PASSWORD: &str = "asdfasdf";
const SEED_PASSWORD: &str = "";
const ENCRYPTED_DESCRIPTOR_04: &str = "d80b2c6af514802c5e7b7a91e8c84f93edbe705f0849cf57abf5bfd465a0c4b2792e8fb16a1b76d7f65b8d68a65c3c001565318ddcd9905536391ca0abb68789da28bb3ccbc923760b36474876070fff4c67e2c23c79f9d5272513fa17dfa4ea47101c7fb0487a678eb40b37ccda73805ab821b63feb9abcc7c60bfa7aac2ff076d906fd542400fb81d8a4bf8905932f3db10a252a1cd9661515f996724545c7e732db17899e6210e80af14be0610b0db90143513586bf8670deaa14e05f66b556936c7cc6f82a3363ee7e77f8081205cb6a1a5ad6d627d4dae6c174cd1f2158384daa276a37d8ef6b51b8ae7c8351fa07b606beb2f083e8a2f64f8148e834f326c256cc274adee7e5d05a946e23e9a76c165fbc25ff5618e2936b5222f394066fa071f954ff6ee3446d67375ec5d1caa5d01d7722576ecdf6f67aff833dccb7ade77a1988959cb250a8723a1dff2d3b6b39877f0291c5a34fb2ccc01ba0b6f9bdf6b1d30a3309870ee5d4f828e5d45edc7c5c08ff94ca8fd572e1038ff6975031a5a48733b968cf28eaf90a5895b393ef1230b1632c800f96b95cd0e77e8c971d20311a38da90aa49cf9570be8ef9e97534f4364d1b5840a5214fc534fbe63e76e3d8aeb159cd446fbaabf2502c0efce9a899650344df9fdd499862c92749fd26ea5070a51d3768515d0716f12312e29ca450c231ccf8714e43aa548f7a9a8b883e62b33b290f723bbf1469094ed589a7d0fdb12fc76e6267825924fe616eebb24202c387d0326a9d3dab7245cca9abe28003c7f574e9d3e63854732e5fdc6edda4180384a3424ad6ab1109c6a49acc8eb99af347f0ce72637723a9b3377124324dc9e4ec5a3f3c4673eebb1e7e4d5b7fd7d2568987edb71853fdb1bbd922eff16cf5cac008e43a90ff281ceec9f4213a0d6c2a3df4d579aa1ab20003a4792421cbc7a7822faec1430018861c39380878993f75b6051642af06857f57ba9ad067b9537f19dc35f69fa72935fd4690935168a812e20874f586d63af04b5a4955e1734d2a5d3e7d69b8f9f136a5bff94de0f5a932a89fe00d535145e971510ac16d3ddcd3053d7727a0164d5f560c372d98f13d98e67a108753b5df4abe6bbc5536ee551bbdf28cbf311afa41f2826d338cc8a3f87411e3fa1d178ec21da27b9382b9480cb974aa588f7c7ba09e08fd5428019ac017164e10a2a585ca063c518db1514fe3081f4f393fc06fb6d0d1719d33e85dec3a17a506fdf860ec07dde0bcf3d77d345bd893f0f79cda14d55577a1f6b768ef0bf1e2c69d8c201348f6fa2748cc63bc397aad7afe629122188b0b806546237e60be07063d6f1372d36da554d95e741f6bca9c7ffdc48cdf367e9d5b893fd710e74f24c3f0d8194aa389d0ff9f6f9a5f93cf07c1ce5b67589d99a77aa5fb122c1b88d38f5d6e0d18ed66c9637a73fb9085fc4c43e73e7e0124e383d91407c9ddfa285450ef09889b03e6b1550da033e1edbf0cfcf346c9d1929c07d6853999b7f5eea341dee10547d543463434dacde09926d2db1a5788d70711437e530d93c3bfaf58d5500e4bff89a680951976089832b3e1382cb943cbe7be40c5e0770363d30887d5634681cd903927003334bdd10364c39c0b9457fbc9b7a3f23808971d091e20363934555c449840a763eaa39d6407db0550693b2649517ee0696f10244b3814c37e0473f80d1acf26fbe2a129d35809b22cc6d047612ca9344c1e1c6cbeeab3907fb331b1ad90232b3f6984a90d8e1b3e47397f43ca0c9ab5b1273cdf0c368bc537e31a63b278aa76dc282cdf2550549e694afc45f32be88b436392d2b4637ea81b74447fc6892c9608722845cd8d0518459804fc1eefa624bd12a24b8bef12a5bb857906a213e2d4bcb31b6983b004946798c38ece5e2d7d82c04034deedee709f94210cabc0a8916c9b465fdd49fe708b7c2862474bb43368ae42fd4f8e0e45e28b1d1619069d25d3368e53244943952bfbc36004bf88b8e19e3d";
const ENCRYPTED_DESCRIPTOR_05: &str = "0b079114f43a0e3b16a9d68e37a316fc3ddd91ebbcb32466c5080d5ba4e29722565f757b97f376fcd995d3152b270d51ac6424622471f3d62cf42a453d2d3165072c35545294f63197fc8ad51457b4631d1966e45021c3f31dc556a9297a511cf1f16ebd02acc9dd94175a6e5fd3dc33325f1342c01774f6c6c2b4e75cd45b5e9b45295bde0214c2d4db1569bcb14784bf1fd1e975c347685b5e8d9df179722c94847d095a813245822b2d800814dd93466b6eb8892a4ebf42b6e1ea37e415a7a579c8f40c36092f348c54f24784fcf74ace9028e47645b09ae89f1a30190baec96e5be0c9a6399151684cb0a76dff72f7a71f1f28341f095a00106d209a47b4c52ec7f033fac235fb7b88b8668fb572c5a7bffa9ced677b92afcd2567170d4068f9b35a9aa47e90a6ee0ee8850d8bda0721b501308336676a0b85acd6a443510aa02ebe7b91a96d481a75edd5bb7dd9a91574b6e51a5d7dc383798ccbef7e71e64cd40a5721e756f3a8a3733db61ec897890261c9cc6942154ac37ebe66e3640f83ec4eb91a0cdf15fdcba70c96c66742af3d7342e2293a043bcc4943732325051f3cf4bfae8ac84e598742ad826f58330283a459f4207f7940f58ac64c10fd0ad5fc7f99199a9cc10862decfe9d16a0cf3071b120d3138ef5fa0f6b04028dc0fc6e95d5caf3428298e36c5a1a5872b2411bad08bbc0ee74427a247c641e89a54f8dcc1c68ad0a0e1ca6f0501847beb5b8f1ae689130e55279eefd34bde2b00fbc2260fc12bda257ff541e93fb55fb779fdab45805f5f5c96d9a643ad01786504cda7afb49896b0fe2ec843a36401d55fec20d517bca89c55008352587613b0aedd1f7f44abfdf09310a7f09d5f68a158d4e3f5c9bb10ea5bbcf0ecaa562b5fa4e7f320c9b601e22342e2c46c9b16de4c409ddef2f1386e23f78b6695075333230588344575478b2e587db4ee8bef0af8d185b66a0b300730b9ce8c69d3b88fd5b83d500776d5b6420276d34b411eb6f5539882a74e5f4d5910f5e01c84a1c69fb70f6607b0e8071b96285d530fefbee0c45c38f4a101ebc9bf45c3b2cca04131a767ecb2fe346b4617af3e4424774906ca249f33226f3156532a57a219a3de5809f2dd7e1dff0e1a1b67a42d717f800eff2c7579049a2eeda09eda55b6706a41ac287d4cd35fb80062a2c63b5ddd5370a1768ba986695188be379aa9ef5d471538d6a301e4d3acf9dc20b56defe5293c0918b78c00b6d76b05f0d5360f5877f296a9ba09a43ad591d2c31d192de0854938f1990e42b5930232e6fb055fcc2a7bd0143b8b70ce7d2eaa52866b52753e4b86c1e0c994c00c6f36dd9aa7702e833c4699f8855f0925a944e2a51a22c7a813195acde571e8275efa1ef253cf54ec2f5dcba04a1faaf79c52d79dd501da1e7df9951d8e297d3669ded78779462e61ab8d8a6100aac12ec8a444e319508e1533a80878e60aea1527b77abeb536d05754a1b47e69bba6133df33b219c6d959d7b97cdab2eb0fb736635c8d7ab57417188ac7733c263b1cb771d93bbb66c1dc3c6b6240a28374704542aae695732de726749c681867a61de12a69a47a7507891f103bbdd33be49c8c30a575aa890c8a1f38896e62512e56d7f8fcd51ec8633cea9d27fe668b7ba12f52b9814bb5bdfceada55de1ad9796b6699f0789b530faec43eb1a2c8f8788cf4cf1ed07a6ba9c73d8da1b43c75b10a82b17cb8be0ec4046d79f804058c20649b76f2a8a3fae1b77fff657916f2aae5e3d6523036b2d62a0cf6d264afc6a29f6d4e7863ee043d67d3df9c1040845ff3ee52a51dc6f475dbb6babc7d634ce0303bf27894ed6a94644040fbe5a65f4e0ef96588c876a3d5577210a56edaa3b1b5a90e70050a56dce7232bfff9eebfc54cbd0511ee5e4a8f4cbbc12a9ecb5ed6d2b1ac843985ca90008695b944e6d721398dce471e1e8f9d76b4cd27c545fd10638505086ff73f9e38a329f3e84cc9f32a67eb3e1dc71cd39366cfa0701a210c4ecab";

#[ignore = "No longer necessary due to password breaking change in bitmask-core 0.6"]
#[tokio::test]
async fn migration_v4() -> Result<()> {
    init_logging("migration=debug");

    switch_network("testnet").await?;

    info!("Import bitmask-core 0.4 encrypted descriptor");
    let wallet = decrypt_wallet(
        &SecretString(ENCRYPTION_PASSWORD.to_owned()),
        &SecretString(ENCRYPTED_DESCRIPTOR_04.to_owned()),
        &SecretString("".to_owned()),
    )
    .await;

    assert!(wallet.is_err(), "Importing an old descriptor should error");

    let upgraded_descriptor = upgrade_wallet(
        &SecretString(ENCRYPTION_PASSWORD.to_owned()),
        &SecretString(ENCRYPTED_DESCRIPTOR_04.to_owned()),
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await?;

    debug!(
        "Upgraded descriptor: {}",
        serde_json::to_string_pretty(&upgraded_descriptor)?
    );

    let wallet = decrypt_wallet(
        &SecretString(ENCRYPTION_PASSWORD.to_owned()),
        &upgraded_descriptor,
        &SecretString("".to_owned()),
    )
    .await?;

    assert_eq!(
        wallet.public.btc_descriptor_xpub,
        "tpubD6NzVbkrYhZ4Xxrh54Ew5kjkagEfUhS3aCNqRJmUuNfnTXhK4LGXyUzZ5kxgn8f2txjnFtypnoYfRQ9Y8P2nhSNXffxVKutJgxNPxgmwpUR",
        "Upgraded wallet should upgrade the descriptor"
    );

    Ok(())
}

#[ignore = "No longer necessary due to password breaking change in bitmask-core 0.6"]
#[tokio::test]
async fn migration_v5() -> Result<()> {
    init_logging("migration=debug");

    switch_network("testnet").await?;

    info!("Import bitmask-core 0.5 encrypted descriptor");
    let wallet = decrypt_wallet(
        &SecretString(ENCRYPTION_PASSWORD.to_owned()),
        &SecretString(ENCRYPTED_DESCRIPTOR_05.to_owned()),
        &SecretString("".to_owned()),
    )
    .await;

    assert!(wallet.is_err(), "Importing an old descriptor should error");

    let upgraded_descriptor = upgrade_wallet(
        &SecretString(ENCRYPTION_PASSWORD.to_owned()),
        &SecretString(ENCRYPTED_DESCRIPTOR_05.to_owned()),
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await?;

    println!(
        "Upgraded descriptor: {}",
        serde_json::to_string_pretty(&upgraded_descriptor)?
    );

    let wallet = decrypt_wallet(
        &SecretString(ENCRYPTION_PASSWORD.to_owned()),
        &upgraded_descriptor,
        &SecretString("".to_owned()),
    )
    .await?;

    assert_eq!(
        wallet.public.btc_descriptor_xpub,
        "tpubD6NzVbkrYhZ4XJmEMNjxuARFrP5kME8ndqpk9M2QeqtuTv2kTrm87a93Td47bHRRCrSSVvVEu3trvwthVswtPNwK2Kyc9PpudxC1MZrPuNL",
        "Upgraded wallet should upgrade the descriptor"
    );

    Ok(())
}
