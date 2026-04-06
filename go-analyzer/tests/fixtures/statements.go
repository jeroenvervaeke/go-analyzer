package fixtures

func Statements() {
	// var and const
	var x int
	const c = 42

	// short var decl
	y := 10

	// assignment
	x = y
	x += 1

	// inc/dec
	x++
	x--

	// if
	if x > 0 {
		x = 0
	}

	// if-else
	if x > 0 {
		x = 0
	} else {
		x = 1
	}

	// for (condition)
	for x > 0 {
		x--
	}

	// for (c-style)
	for i := 0; i < 10; i++ {
	}

	// for range
	s := []int{1, 2, 3}
	for k, v := range s {
		_ = k
		_ = v
	}

	// switch
	switch x {
	case 1:
		break
	case 2, 3:
		fallthrough
	default:
	}

	// return
	return
}
