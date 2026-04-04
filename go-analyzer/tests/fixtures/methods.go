package fixtures

type Foo struct {
	Name string
}

func (f *Foo) PointerMethod() string {
	return f.Name
}

func (f Foo) ValueMethod() int {
	return 42
}
