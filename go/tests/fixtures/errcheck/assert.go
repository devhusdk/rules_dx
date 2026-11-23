package errcheck

// Name returns v as a string and panics when it is not one.
func Name(v any) string {
	return v.(string)
}
