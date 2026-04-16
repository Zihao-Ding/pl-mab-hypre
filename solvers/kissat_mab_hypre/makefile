all:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build"
kissat:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build" kissat
tissat:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build" tissat
clean:
	rm -f "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre"/makefile
	rm -f "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre"/src/makefile
	-$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build" clean
	rm -rf "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build"
coverage:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build" coverage
format:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build" format
test:
	$(MAKE) -C "/mnt/d/wsl-code/my-painless/painless/solvers/kissat_mab_hypre/build" test
.PHONY: all clean coverage format kissat test tissat
