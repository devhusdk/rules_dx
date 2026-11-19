package base

const Name = "base"

type Box struct{ Value string }

func (b Box) Label() string {
	return Name + ":" + b.Value
}
