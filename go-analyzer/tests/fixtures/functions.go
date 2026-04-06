package fixtures

func Simple() {}

func WithParams(a int, b string) (int, error) {
	return 0, nil
}

func Variadic(args ...int) int {
	return 0
}

func Generic[T any](x T) T {
	return x
}
