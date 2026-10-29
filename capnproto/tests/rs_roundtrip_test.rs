//! Builds a Shape whose origin type comes from the base crate and reads it back.

use api_rs::kind_capnp::Kind;
use api_rs::shape_capnp::shape;

#[test]
fn shape_round_trips_through_the_generated_crates() {
    let mut message = capnp::message::Builder::new_default();
    {
        let mut builder = message.init_root::<shape::Builder>();
        builder.set_name("circle");
        let mut origin = builder.reborrow().init_origin();
        origin.set_x(1.5);
        origin.set_y(-2.0);
        builder.set_kind(Kind::Square);
    }
    let mut bytes = Vec::new();
    capnp::serialize::write_message(&mut bytes, &message).expect("write");

    let reader = capnp::serialize::read_message(&bytes[..], capnp::message::ReaderOptions::new())
        .expect("read");
    let shape = reader.get_root::<shape::Reader>().expect("root");
    assert_eq!(
        shape.get_name().expect("name").to_str().expect("utf8"),
        "circle"
    );
    // The origin is the base crate's type, not a copy inside api_rs.
    let origin: base_rs::common_capnp::point::Reader = shape.get_origin().expect("origin");
    assert_eq!(origin.get_x(), 1.5);
    assert_eq!(origin.get_y(), -2.0);
    assert_eq!(shape.get_kind().expect("kind"), Kind::Square);
}
