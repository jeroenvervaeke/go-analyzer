package fixtures

func Expressions() {
	// Literals
	_ = 42
	_ = 3.14
	_ = "hello"
	_ = `raw`
	_ = 'x'
	_ = 1i
	_ = true
	_ = false
	_ = nil

	// Composite literal
	type Point struct{ X, Y int }
	_ = Point{X: 1, Y: 2}

	// Function literal
	_ = func(x int) int { return x }

	// Binary ops
	_ = 1 + 2
	_ = 1 - 2
	_ = 1 * 2
	_ = 1 / 2
	_ = 1 % 2
	_ = true && false
	_ = true || false
	_ = 1 == 2
	_ = 1 != 2
	_ = 1 < 2
	_ = 1 <= 2
	_ = 1 > 2
	_ = 1 >= 2

	// Unary ops
	_ = -1
	_ = !true

	// Selector
	x := Point{X: 1, Y: 2}
	_ = x.X

	// Index
	s := []int{1, 2, 3}
	_ = s[0]

	// Slice
	_ = s[1:2]
	_ = s[1:2:3]

	// Type assertion
	var i interface{} = 42
	_ = i.(int)
}
