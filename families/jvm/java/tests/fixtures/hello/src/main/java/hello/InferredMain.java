// Java binary whose main_class comes from the target name.
package hello;

public class InferredMain {
  public static void main(String[] args) {
    System.out.println(Hello.hello("world"));
  }
}
