# Make sure that the shell is the same everywhere.
SHELL = /bin/sh

src_c := src/main.c
objs := $(src_c:%.c=%.o)

CFLAGS = -std=gnu11 -ffreestanding -nostdlib -flto -fPIC -O2 -Wall -Wextra -Werror
LDLIBS = -lflibc
LDFLAGS = -static

.PHONY: all
all: gstatus

.PHONY: clean
clean:
	rm -f $(objs) gstatus

.PHONY: format
format:
	clang-format -i $(src_c) include/flibc/*.h

%.o: %.c
	$(CC) $< -c -MD -o $@ -Iflibc/include $(CPPFLAGS) $(CFLAGS)

flibc/libflibc.a:
	$(MAKE) -C flibc

gstatus: $(objs) flibc/libflibc.a
	$(CC) $(objs) -o $@ -Lflibc $(CFLAGS) $(LDLIBS) $(LDFLAGS)
