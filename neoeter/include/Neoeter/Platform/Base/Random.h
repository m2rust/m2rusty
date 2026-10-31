#pragma once

#if defined(_WIN32)
extern void				srandom(unsigned long seed);
extern unsigned long	random();
#else
#include <cstdlib>
#endif
extern float			frandom(float flLow, float flHigh);
extern long				random_range(long from, long to);
