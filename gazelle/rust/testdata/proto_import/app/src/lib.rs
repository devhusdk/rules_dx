use api_proto_rs::User;
use prost::Message;

pub fn name(user: &User) -> &str {
    &user.name
}

pub fn encode(user: &User) -> Vec<u8> {
    user.encode_to_vec()
}
