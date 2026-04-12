all:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build"
kissat:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build" kissat
tissat:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build" tissat
clean:
	rm -f "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc"/makefile
	-$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build" clean
	rm -rf "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build"
coverage:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build" coverage
indent:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build" indent
test:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat-inc/build" test
.PHONY: all clean coverage indent kissat test tissat
