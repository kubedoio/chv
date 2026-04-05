package ch

import (
	"bufio"
	"context"
	"io"
	"os"
	"time"
)

// ConsoleReader provides access to VM serial console output.
type ConsoleReader struct {
	filePath string
	file     *os.File
	reader   *bufio.Reader
}

// NewConsoleReader creates a new console reader for the given log file.
func NewConsoleReader(filePath string) (*ConsoleReader, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}

	// Seek to end for tailing
	if _, err := file.Seek(0, io.SeekEnd); err != nil {
		file.Close()
		return nil, err
	}

	return &ConsoleReader{
		filePath: filePath,
		file:     file,
		reader:   bufio.NewReader(file),
	}, nil
}

// ReadLine reads a line from the console output.
func (c *ConsoleReader) ReadLine() (string, error) {
	return c.reader.ReadString('\n')
}

// ReadBytes reads raw bytes from the console.
func (c *ConsoleReader) ReadBytes(buf []byte) (int, error) {
	return c.reader.Read(buf)
}

// Close closes the console reader.
func (c *ConsoleReader) Close() error {
	return c.file.Close()
}

// Tail tails the console file and sends output to the provided channel.
func (c *ConsoleReader) Tail(ctx context.Context, output chan<- string) error {
	defer close(output)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		line, err := c.ReadLine()
		if err != nil {
			if err == io.EOF {
				// Wait for more data
				time.Sleep(100 * time.Millisecond)
				continue
			}
			return err
		}

		select {
		case output <- line:
		case <-ctx.Done():
			return ctx.Err()
		}
	}
}

// ReadFullConsole reads the entire console file.
func ReadFullConsole(filePath string) (string, error) {
	data, err := os.ReadFile(filePath)
	if err != nil {
		return "", err
	}
	return string(data), nil
}
