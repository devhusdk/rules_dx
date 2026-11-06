package com.example.app;

import com.example.api.v1.Greeting;

public final class Main {
  private Main() {}

  public static void main(String[] args) throws Exception {
    Greeting greeting = Greeting.newBuilder().setName("java").setTimes(2).build();
    Greeting back = Greeting.parseFrom(greeting.toByteArray());
    System.out.println(back.getName() + " " + back.getTimes());
  }
}
