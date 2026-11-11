namespace cpp consumer
namespace java consumer

struct Greeting {
  1: string name,
}

service Greeter {
  Greeting greet(1: string name),
}
