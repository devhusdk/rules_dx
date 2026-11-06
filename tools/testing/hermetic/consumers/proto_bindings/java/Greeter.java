package com.example.app;

import com.example.api.v1.Greeting;

public final class Greeter {
  private Greeter() {}

  public static Greeting roundTrip(String name, int times) throws Exception {
    Greeting greeting = Greeting.newBuilder().setName(name).setTimes(times).build();
    return Greeting.parseFrom(greeting.toByteArray());
  }
}
