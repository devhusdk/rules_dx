package com.example.app;

import com.example.api.v1.Greeting;
import com.google.protobuf.InvalidProtocolBufferException;

public final class Greeter {
  private Greeter() {}

  public static Greeting roundTrip(String name, int times) throws InvalidProtocolBufferException {
    Greeting greeting = Greeting.newBuilder().setName(name).setTimes(times).build();
    return Greeting.parseFrom(greeting.toByteArray());
  }
}
