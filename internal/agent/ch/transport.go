package ch

import (
	"context"
	"net"
	"net/http"
	"time"
)

// unixSocketTransport is an HTTP transport that connects via Unix socket.
type unixSocketTransport struct {
	socketPath string
	baseTransport *http.Transport
}

func newUnixSocketTransport(socketPath string) *unixSocketTransport {
	return &unixSocketTransport{
		socketPath: socketPath,
		baseTransport: &http.Transport{
			DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
				d := net.Dialer{Timeout: 10 * time.Second}
				return d.DialContext(ctx, "unix", socketPath)
			},
		},
	}
}

func (t *unixSocketTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	// Clone the request and set host to localhost (required for HTTP/1.1)
	newReq := req.Clone(req.Context())
	newReq.URL.Scheme = "http"
	newReq.URL.Host = "localhost"
	
	return t.baseTransport.RoundTrip(newReq)
}
