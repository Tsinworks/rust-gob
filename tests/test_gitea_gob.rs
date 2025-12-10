use gob::{StreamSerializer, StreamDeserializer};
#[macro_use]
extern crate serde_derive;
extern crate serde_gob;
#[macro_use]
extern crate serde_gob_derive;
use std::io::Cursor;

#[test]
fn test_gitea_gob() {
    #[derive(Serialize, GobSerialize, Deserialize, Debug)]
    #[gob(interpret_as = "map[interface{}]interface{}")]
    struct User {
        uid: i64,
        uname: String,
        email: String,
        #[serde(rename = "_old_uid")]
        old_uid: String,
        #[serde(rename = "userHasTwoFactorAuth")]
        has_2fa: bool,
    }

    let buffer = include_bytes!("normal-session-2.bin");
    let cursor = Cursor::new(buffer);
    let mut stream = StreamDeserializer::new(cursor);
    let user = stream.deserialize::<User>().unwrap().unwrap();
    println!("user: {:?}", user);
    let mut buffer = Vec::new();
    {
        let mut stream = StreamSerializer::new_with_write(&mut buffer);
        stream.serialize(&user).unwrap();
    }
    std::fs::write("user.gob", buffer).unwrap();
}