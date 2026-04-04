package alpha

import "fmt"

type User struct {
	Name  string
	Email string
}

func (u *User) String() string {
	return fmt.Sprintf("User(%s, %s)", u.Name, u.Email)
}

type Admin struct {
	User
	Role string
}

type Config struct {
	Host string
	Port int
}

func NewUser(name, email string) *User {
	fmt.Printf("creating user %s\n", name)
	return &User{Name: name, Email: email}
}

func NewConfig(host string, port int) *Config {
	return &Config{Host: host, Port: port}
}

func helperFunc() int {
	return 42
}
