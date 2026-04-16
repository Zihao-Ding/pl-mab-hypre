MAKEFLAGS=-j $(if $(CORES),$(CORES),1)
CXX=g++

CCAnr: cca.h build.h basis.h cw.h preprocessor.h sat_solver.h sat_solver.cpp
	$(CXX) sat_solver.cpp -O3 -static  -o CCAnr
	rm -f *~

# Compile as static library and executable

SRC = sat_solver.cpp cca.h basis.h cw.h preprocessor.h sat_solver.h
# Actually only compile the cpp file, headers can't be compiled directly

OBJECTS = sat_solver.o

# Static library target
libpassat.a: sat_solver.o
	ar rcs $@ sat_solver.o
	ranlib $@



sat_solver.o: sat_solver.cpp cca.h basis.h cw.h preprocessor.h sat_solver.h build.h
	g++ -c sat_solver.cpp -O3 -std=c++11

clean:
	rm -f CCAnr libpassat.a *.o *~