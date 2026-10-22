package grades

/** Raised for input that is not a valid grade list. */
class GradeException(message: String, cause: Throwable? = null) : Exception(message, cause)
