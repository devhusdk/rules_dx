package govet

type Color int

const (
	Red Color = iota
	Green
	Blue
)

func Name(c Color) string {
	switch c {
	case Red:
		return "red"
	case Green:
		return "green"
	}
	return ""
}
