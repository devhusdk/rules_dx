package fixtures.associates

// An associate shares the module, so the internal member resolves without a false error.
fun reveal(): Int = secret()
