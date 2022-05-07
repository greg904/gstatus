object_files := libs/parse_vdso.o \
	src/main.o \
	src/sys.o \
	src/util.o

base_flags := -std=gnu11 -ffreestanding -nostdlib -fno-stack-protector -static

CC := clang
CCFLAGS := -flto -fPIC -O2 -Wall -Wextra -Werror
CCFLAGS := ${CCFLAGS} ${base_flags}

LD := ${CC}
LDFLAGS := -flto -fPIC -O2 -fuse-ld=lld
LDFLAGS := ${LDFLAGS} ${base_flags}

.PHONY: all
all: gstatus

.PHONY: clean
clean:
	rm -f ${object_files} gstatus

.PHONY: format
format:
	clang-format -i src/*.c src/*.h

%.o: %.c
	${CC} $^ -c -o $@ ${CCFLAGS}

gstatus: ${object_files}
	${LD} $^ -o $@ ${LDFLAGS}
