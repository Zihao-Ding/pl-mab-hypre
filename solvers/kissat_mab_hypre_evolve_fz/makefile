all: bliss dprtrim
	$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build"
kissat: bliss dprtrim
	$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build" kissat
tissat: bliss dprtrim
	$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build" tissat
bliss:
	@if [ -d "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/bliss" ]; then \
	  if [ ! -f "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/bliss/libbliss.a" ]; then \
	    $(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/bliss"; \
	  fi \
	fi
dprtrim:
	@if [ -d "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/dprtrim" ]; then \
	  if [ ! -f "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/dpr-trim" ]; then \
	    $(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/dprtrim"; \
	    cp "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/dprtrim/dpr-trim" "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/"; \
	  fi \
	fi
clean:
	rm -f "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz"/makefile
	rm -f "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz"/src/makefile
	rm -f "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz"/dpr-trim
	rm -f "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz"/bliss/libbliss.a "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz"/bliss/*.o "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz"/bliss/bliss
	-$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build" clean
	rm -rf "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build"
distclean: clean
	rm -rf "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/bliss" "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/dprtrim"
coverage:
	$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build" coverage
format:
	$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build" format
test: bliss dprtrim
	$(MAKE) -C "/mnt/d/wsl-code/sat_comp26_code/develop-mab-hypre-evolve-fz/pl-mab-hypre/solvers/kissat_mab_hypre_evolve_fz/build" test
.PHONY: all bliss dprtrim clean distclean coverage format kissat test tissat
