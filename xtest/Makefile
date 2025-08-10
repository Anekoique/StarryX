DOCKER ?= docker.educg.net/cg/os-contest:20250714

all: sdcard

build-rv:
	make -f Makefile.sub clean
	mkdir -p sdcard/riscv/lib
	mkdir -p sdcard/riscv/usr
	mkdir -p sdcard/riscv/bin
	make -f Makefile.sub PREFIX=riscv64-linux-gnu- DESTDIR=/code/sdcard/riscv
	# rm -fr /code/sdcard/riscv/usr/libexec/git-core
	cp /usr/riscv64-linux-gnu/lib/libc.so.6 sdcard/riscv/lib/libc.so
	cp /usr/riscv64-linux-gnu/lib/libc.so.6 sdcard/riscv/lib/
	cp /usr/riscv64-linux-gnu/lib/libm.so.6 sdcard/riscv/lib/libm.so
	cp /usr/riscv64-linux-gnu/lib/libm.so.6 sdcard/riscv/lib/
	cp /opt/riscv64--musl--bleeding-edge-2020.08-1/riscv64-buildroot-linux-musl/sysroot/lib/libc.so sdcard/riscv/lib/ld-musl-riscv64.so.1
	cp /opt/riscv64--musl--bleeding-edge-2020.08-1/riscv64-buildroot-linux-musl/sysroot/lib/libc.so sdcard/riscv/lib/ld-musl-riscv64-sf.so.1
	cp /usr/riscv64-linux-gnu/lib/ld-linux-riscv64-lp64d.so.1 sdcard/riscv/lib/ld-linux-riscv64-lp64d.so.1

build-la:
	make -f Makefile.sub clean
	mkdir -p sdcard/loongarch/
	make -f Makefile.sub PREFIX=loongarch64-linux-gnu- DESTDIR=/code/sdcard/loongarch/glibc
	rm -fr /code/sdcard/loongarch/glibc/usr/libexec/git-core
	mkdir -p /code/sdcard/loongarch/musl
	cp -fr /code/sdcard/loongarch/glibc/* /code/sdcard/loongarch/musl
	mkdir -p /code/sdcard/loongarch/musl/lib
	mkdir -p /code/sdcard/loongarch/glibc/lib
	cp scripts/git_testcode.sh /code/sdcard/loongarch/glibc
	cp scripts/git_testcode.sh /code/sdcard/loongarch/musl

sdcard-rv: build-rv .PHONY
	dd if=/dev/zero of=sdcard-rv.img count=512 bs=1M
	mkfs.ext4 sdcard-rv.img
	mkdir -p mnt
	mount sdcard-rv.img mnt
	cp -rL sdcard/riscv/* mnt
	umount mnt

sdcard-la: build-la .PHONY
	dd if=/dev/zero of=sdcard-la.img count=512 bs=1M
	mkfs.ext4 sdcard-la.img
	mkdir -p mnt
	mount sdcard-la.img mnt
	cp -rL sdcard/loongarch/* mnt
	umount mnt

sdcard: sdcard-rv sdcard-la

clean:
	make -f Makefile.sub clean
	rm -rf sdcard/riscv/*
	rm -rf sdcard/loongarch/*
	rm -f sdcard-la.img.gz
	rm -f sdcard-rv.img.gz

docker:
	docker run --rm -it -v .:/code --entrypoint bash -w /code --privileged $(DOCKER)


.PHONY:
