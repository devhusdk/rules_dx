use greeter_proto::dx::tonic::v1::greeter_client::GreeterClient;
use greeter_proto::dx::tonic::v1::greeter_server::{Greeter, GreeterServer};
use greeter_proto::dx::tonic::v1::{HelloReply, HelloRequest};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

struct Hello;

#[tonic::async_trait]
impl Greeter for Hello {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        Ok(Response::new(HelloReply {
            message: format!("hello {}", request.into_inner().name),
        }))
    }
}

#[tokio::test]
async fn unary_call_over_loopback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(
        Server::builder()
            .add_service(GreeterServer::new(Hello))
            .serve_with_incoming(TcpListenerStream::new(listener)),
    );

    let mut client = GreeterClient::connect(format!("http://{addr}"))
        .await
        .unwrap();
    let reply = client
        .say_hello(HelloRequest { name: "dx".into() })
        .await
        .unwrap();
    assert_eq!(reply.into_inner().message, "hello dx");
}
