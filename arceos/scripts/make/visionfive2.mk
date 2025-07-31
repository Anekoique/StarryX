visionfive2: build
	cp $(OUT_BIN) ./kernel-qemu.bin
	mkimage -f tools/visionfive2/visionfive2-arceos.its arceos-vf2.itb
	@echo '=============> Built the FIT-uImage VisionFive2 -- success!'