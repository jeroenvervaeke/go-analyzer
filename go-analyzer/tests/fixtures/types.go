package fixtures

type MyStruct struct {
	Name string
	Age  int
}

type MyInterface interface {
	String() string
}

type MyAlias = int

type MyNewtype int

type GenericType[T any] struct {
	Value T
}
