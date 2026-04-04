package fixtures

func Channels() {
	ch := make(chan int)
	ch <- 42
	x := <-ch
	_ = x

	var recv <-chan int
	_ = recv

	var send chan<- int
	_ = send
}
