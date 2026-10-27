# Wildcard Import Resolution

Verifies a wildcard import stays qualified: a local `com.a.*` resolves to the library that holds
package `com.a`, and an external `org.x.*` uses the `# gazelle:resolve` mapping on `org.x`.
