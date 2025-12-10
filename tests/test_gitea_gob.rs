use gob::{StreamSerializer, StreamDeserializer};
#[macro_use]
extern crate serde_derive;
extern crate serde_gob;
#[macro_use]
extern crate serde_gob_derive;
use std::io::Cursor;

#[derive(Serialize, GobSerialize, Deserialize, Debug)]
#[gob(interpret_as = "map[interface{}]interface{}", type_id=64)]
struct User {
    uid: i64,
    uname: String,
    email: String,
    #[serde(rename = "_old_uid")]
    old_uid: String,
    #[serde(rename = "userHasTwoFactorAuth")]
    has_2fa: bool,
}

#[test]
fn test_gitea_gob_deserialize() {
    let buffer = include_bytes!("normal-session-2.bin");
    let cursor = Cursor::new(buffer);
    let mut stream = StreamDeserializer::new(cursor);
    let user = stream.deserialize::<User>().unwrap().unwrap();
    println!("user: {:?}", user);
    assert_eq!(user.uid, 1);
    assert_eq!(user.uname, "dsotsen");
    assert_eq!(user.email, "dsotsen@qq.com");
    assert_eq!(user.old_uid, "1");
    assert_eq!(user.has_2fa, false);
}

#[test]
fn test_gitea_gob_serialize() {
    let user = User {
        uid: 1,
        uname: "test".to_string(),
        email: "test@test.com".to_string(),
        old_uid: "test".to_string(),
        has_2fa: false,
    };
    {
        // serialize to file
        let mut buffer = Vec::new();
        {
            let mut stream = StreamSerializer::new_with_write(&mut buffer);
            stream.serialize(&user).unwrap();
        }
        std::fs::write("tests/user.gob", buffer).unwrap();    
    }

    // deserialize check
    let buffer = std::fs::read("tests/user.gob").unwrap();
    let mut stream = StreamDeserializer::new(Cursor::new(buffer));
    let user = stream.deserialize::<User>().unwrap().unwrap();
    println!("user: {:?}", user);
    assert_eq!(user.uid, 1);
    assert_eq!(user.uname, "test");
    assert_eq!(user.email, "test@test.com");
    assert_eq!(user.old_uid, "test");
    assert_eq!(user.has_2fa, false);
}

// fn test_decode_user_info() {
//     let client = redis::Client::open("redis://cdn.mixstudio.tech:30002/0").unwrap();
//     let mut con = client.get_connection().unwrap();
//     //let _: () = con.set("test_key", "test_value").unwrap();
//     let buffer: Vec<u8> = con.get("45f8f1e6898dbfe0").unwrap();
//     std::fs::write("normal-session-2.bin", &buffer).unwrap();
//     //assert_eq!(value, "test_value");
//     // let filename = "normal-session.bin";
//     // let mut file = File::open(filename).expect("Failed to open normal-session.bin");
//     // let mut buffer = Vec::new();
//     // file.read_to_end(&mut buffer).expect("Failed to read file");
//     let cursor = std::io::Cursor::new(&buffer);
//     let mut decoder = Decoder::new(cursor);
//     //println!("Test: Decoding generic values from {}", filename);
//     let user_info: UserInfo = decoder.decode_into().expect("Failed to decode UserInfo");
//     println!("Decoded UserInfo: {:?}", user_info);
//     assert_eq!(user_info.uid, 1);
//     assert_eq!(user_info.uname, "dsotsen");
//     assert_eq!(user_info.old_uid, "1");
//     assert_eq!(user_info.two_factor_auth, false);
// }
