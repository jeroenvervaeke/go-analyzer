package alpha

// Stringer is a type that can convert itself to a string.
type Stringer interface {
	String() string
}

// Starter is a type that can be started.
type Starter interface {
	Start() error
}
