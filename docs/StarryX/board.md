sudo ip addr add 192.168.137.1/24 dev enp55s0
sudo minicom -D /dev/ttyACM0 -b 115200
setenv ipaddr 192.168.137.223
setenv serverip 192.168.137.1
saveenv

tftpboot 0x40200000 StarryX_riscv64-visionfive2.bin
go 0x40200000

sudo minicom -D /dev/ttyUSB0 -b 115200
tftpboot 0x9000000092000000 StarryX_loongarch64-2k1000.bin
go 0x9000000092000000
