//! Writes a Shape with the binary protocol and reads it back.

use shapes_rs::common::Point;
use shapes_rs::shape::Shape;
use thrift::protocol::{TBinaryInputProtocol, TBinaryOutputProtocol, TSerializable};
use thrift::transport::TBufferChannel;

#[test]
fn shape_round_trips_through_the_generated_crate() {
    let shape = Shape::new(
        "circle".to_string(),
        Point::new(Some(1.5.into()), Some((-2.0).into())),
    );
    let mut channel = TBufferChannel::with_capacity(0, 256);
    shape
        .write_to_out_protocol(&mut TBinaryOutputProtocol::new(&mut channel, true))
        .expect("write");
    let bytes = channel.write_bytes();
    let mut input = TBufferChannel::with_capacity(bytes.len(), 0);
    input.set_readable_bytes(&bytes);
    let read = Shape::read_from_in_protocol(&mut TBinaryInputProtocol::new(&mut input, true))
        .expect("read");
    assert_eq!(read.name.as_deref(), Some("circle"));
    let origin = read.origin.expect("origin");
    assert_eq!(origin.x.map(f64::from), Some(1.5));
    assert_eq!(origin.y.map(f64::from), Some(-2.0));
}
