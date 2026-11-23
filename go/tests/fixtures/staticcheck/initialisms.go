// Package initialisms has one finding, which only ST1003 reports.
package initialisms

// HttpServer breaks the HTTP initialism.
type HttpServer struct{}

// Name returns the server name.
func (HttpServer) Name() string {
	return "server"
}
