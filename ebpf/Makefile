# CHV eBPF Policy Programs
# ==========================================================================
# This Makefile compiles eBPF TC classifier programs.
# Requirements:
#   - clang >= 12
#   - libbpf development headers
#   - Linux kernel headers (for vmlinux.h or bpf headers)
#
# Usage:
#   make          - compile all eBPF programs
#   make clean    - remove compiled objects
#   make vmlinux  - generate vmlinux.h from running kernel BTF
# ==========================================================================

CLANG       ?= clang
LLC         ?= llc
BPFTOOL     ?= bpftool
ARCH        := $(shell uname -m | sed 's/x86_64/x86/' | sed 's/aarch64/arm64/')
INCLUDES    := -I/usr/include -I/usr/include/$(shell uname -m)-linux-gnu
BPF_CFLAGS  := -g -O2 -target bpf -D__TARGET_ARCH_$(ARCH) $(INCLUDES)

SRCS := $(wildcard *.bpf.c)
OBJS := $(SRCS:.bpf.c=_tc.o)

.PHONY: all clean vmlinux

all: $(OBJS)
	@echo "eBPF programs compiled: $(OBJS)"

%_tc.o: %.bpf.c
	$(CLANG) $(BPF_CFLAGS) -c $< -o $@

vmlinux:
	$(BPFTOOL) btf dump file /sys/kernel/btf/vmlinux format c > vmlinux.h

clean:
	rm -f *.o vmlinux.h
