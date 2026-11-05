use api_proto_rs::User;

pub fn name(user: &User) -> &str {
    &user.name
}
