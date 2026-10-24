package consumer;

import com.a.Outer.Inner;
import com.a.Util;

public class Consumer {
    public Inner inner() {
        return new Inner();
    }

    public Util util() {
        return new Util();
    }
}
