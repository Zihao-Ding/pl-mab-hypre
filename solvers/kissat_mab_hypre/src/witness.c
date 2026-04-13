#include "witness.h"
#include "allocate.h"
#include "internal.h"

#include <stdio.h>
#include <string.h>

void kissat_print_witness(kissat *solver, int max_var, bool partial)
{
    // chars buffer;
    // INIT_STACK(buffer);
    solver->last_val = (int *)malloc(sizeof(int) * (max_var + 1));
    for (int eidx = 1; eidx <= max_var; eidx++) {
        int tmp = kissat_value(solver, eidx);
        if (!tmp && !partial)
            tmp = eidx;
        // if (tmp)
        //   print_int(solver, &buffer, tmp);
        solver->last_val[eidx] = tmp > 0 ? 1 : -1;
    }
    // print_int(solver, &buffer, 0);
    // assert(!EMPTY_STACK(buffer));
    // flush_buffer(&buffer);
    // RELEASE_STACK(buffer);
}
