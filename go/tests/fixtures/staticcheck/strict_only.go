// Package strictonly only has findings that need the strict profile.
package strictonly

// HttpServer is named so that only the strict profile reports it.
type HttpServer struct{}

// Name returns the server name.
func (HttpServer) Name() string {
	return "server"
}
