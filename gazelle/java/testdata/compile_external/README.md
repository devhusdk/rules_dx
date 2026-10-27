# Compile External

A real `javac` compiles each package with only its generated deps, and fails once any one of them
is gone. The jsr305 `javax.annotation.Nonnull` resolves through its mapping, not the JDK.
