all:
	$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build"
kissat:
	$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build" kissat
tissat:
	$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build" tissat
clean:
	rm -f "/mnt/d/wsl-code/kissat_satcomp26"/makefile
	rm -f "/mnt/d/wsl-code/kissat_satcomp26"/src/makefile
	-$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build" clean
	rm -rf "/mnt/d/wsl-code/kissat_satcomp26/build"
coverage:
	$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build" coverage
format:
	$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build" format
test:
	$(MAKE) -C "/mnt/d/wsl-code/kissat_satcomp26/build" test
.PHONY: all clean coverage format kissat test tissat
