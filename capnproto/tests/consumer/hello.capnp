@0xb2a7c1e4d5f60918;

struct Greeting {
  name @0 :Text;
}

interface Greeter {
  greet @0 (name :Text) -> (greeting :Greeting);
}
