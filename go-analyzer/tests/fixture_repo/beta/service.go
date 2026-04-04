package beta

import "fmt"

type Server struct {
	Addr string
}

func (s *Server) String() string {
	return fmt.Sprintf("Server(%s)", s.Addr)
}

func (s *Server) Start() error {
	return nil
}

type Client struct {
	URL string
}

func (c *Client) Connect() error {
	return nil
}

func RunServer(addr string) error {
	fmt.Printf("starting server at %s\n", addr)
	s := &Server{Addr: addr}
	return s.Start()
}
