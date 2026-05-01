all:
	$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build"
kissat:
	$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build" kissat
tissat:
	$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build" tissat
clean:
	rm -f "/app/mcp-satlution/solvers/SATLUTION_51"/makefile
	rm -f "/app/mcp-satlution/solvers/SATLUTION_51"/src/makefile
	-$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build" clean
	rm -rf "/app/mcp-satlution/solvers/SATLUTION_51/build"
coverage:
	$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build" coverage
format:
	$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build" format
test:
	$(MAKE) -C "/app/mcp-satlution/solvers/SATLUTION_51/build" test
.PHONY: all clean coverage format kissat test tissat
